"""Smoke tests del SDK Python.

Requiere el módulo nativo construido con `maturin develop`. Si no está
disponible, los tests se skippean con importorskip.
"""

from __future__ import annotations

import queue
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
    assert session.local_port > 0
    session.close()


def test_local_addr_exposed():
    session = gs.Session(bind_addr="127.0.0.1", bind_port=0)
    assert session.local_addr.startswith("127.0.0.1:")
    port = session.local_port
    assert 1024 <= port <= 65535
    session.close()


def test_loopback_handshake_and_send():
    """Two sessions en localhost hacen handshake 3-way y envían un frame."""
    server = gs.Session(config=gs.Config(), bind_addr="127.0.0.1", bind_port=0)
    client = gs.Session(config=gs.Config(), bind_addr="127.0.0.1", bind_port=0)

    server_port = server.local_port
    client_port = client.local_port

    result: queue.Queue = queue.Queue()

    def server_thread():
        try:
            server.accept("127.0.0.1", client_port)
            result.put(("ok", None))
        except BaseException as e:
            result.put(("err", e))

    t = threading.Thread(target=server_thread, daemon=True)
    t.start()

    # El servidor ya está listen-blocking; ahora el cliente envía INIT.
    # Un pequeño delay para ceder el CPU al thread del servidor.
    time.sleep(0.05)
    client.connect("127.0.0.1", server_port)

    status, err = result.get(timeout=5.0)
    if status == "err":
        raise err

    assert client.session_id != 0
    assert client.session_id == server.session_id

    payload = b"\x00\x10" * 480  # 960 bytes = 480 samples PCM16 mono @ 20 ms
    client.send_audio(payload)

    client.close()
    server.close()


def test_metrics_snapshot_shape():
    session = gs.Session(bind_addr="127.0.0.1", bind_port=0)
    m = session.metrics()
    assert m.rtt_ms >= 0.0
    assert m.estimated_mos >= 1.0
    assert m.estimated_mos <= 5.0
    assert m.packets_sent == 0
    session.close()
