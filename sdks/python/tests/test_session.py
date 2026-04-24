"""Smoke tests del SDK Python.

Estos tests requieren el módulo nativo construido con `maturin develop`.
Si el módulo no está disponible, los tests se skippean.
"""

from __future__ import annotations

import threading
import time

import pytest

gs = pytest.importorskip("gravital_sound")


def test_version_exposed():
    assert gs.__version__
    assert gs.PROTOCOL_VERSION == 1


def test_config_defaults():
    cfg = gs.Config()
    assert cfg.sample_rate == 48000
    assert cfg.channels == 1


def test_config_overrides():
    cfg = gs.Config(sample_rate=16000, channels=2, frame_duration_ms=10)
    assert cfg.sample_rate == 16000
    assert cfg.channels == 2
    assert cfg.frame_duration_ms == 10


def test_session_create_and_close():
    session = gs.Session(bind_addr="127.0.0.1", bind_port=0)
    assert session.session_id == 0
    session.close()


def test_loopback_handshake_and_send():
    server = gs.Session(config=gs.Config(), bind_addr="127.0.0.1", bind_port=0)
    client = gs.Session(config=gs.Config(), bind_addr="127.0.0.1", bind_port=0)

    # Hand off real ports via Python-level side channel.
    server_ready = threading.Event()
    server_error: list[BaseException] = []

    def server_main():
        try:
            # El peer "real" lo descubre el handshake al recibir el primer paquete.
            server.accept("127.0.0.1", client_port_holder[0])
            server_ready.set()
        except BaseException as e:  # pragma: no cover - debug
            server_error.append(e)
            server_ready.set()

    # Conocemos el bind port del cliente via metrics no está disponible.
    # En su lugar hacemos handshake en dos threads intercambiando puertos.
    client_port_holder = [0]
    # Extraer el bind port del cliente a partir de un campo interno — el SDK
    # Python no lo expone todavía, así que usamos un socket dummy para picks
    # un puerto libre antes de arrancar el cliente real.
    import socket

    probe = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    probe.bind(("127.0.0.1", 0))
    client_port_holder[0] = probe.getsockname()[1]
    probe.close()

    # Reinicia el cliente en ese puerto específico para que el server sepa a dónde responder.
    client = gs.Session(config=gs.Config(), bind_addr="127.0.0.1", bind_port=client_port_holder[0])

    # Mismo truco para el servidor.
    probe = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    probe.bind(("127.0.0.1", 0))
    server_port = probe.getsockname()[1]
    probe.close()
    server = gs.Session(config=gs.Config(), bind_addr="127.0.0.1", bind_port=server_port)

    t = threading.Thread(target=server_main, daemon=True)
    t.start()

    time.sleep(0.1)
    client.connect("127.0.0.1", server_port)
    server_ready.wait(timeout=5.0)
    if server_error:
        raise server_error[0]

    assert client.session_id != 0
    assert client.session_id == server.session_id

    payload = b"\x00\x10" * 480  # 960 bytes = 480 samples PCM16 mono
    client.send_audio(payload)

    client.close()
    server.close()
