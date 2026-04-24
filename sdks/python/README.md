# gravital-sound — Python SDK

Bindings Python del protocolo Gravital Sound, compilados con `maturin` sobre
`PyO3`. Requiere Python ≥ 3.9.

## Instalación (desarrollo)

```bash
cd sdks/python
python -m venv .venv
source .venv/bin/activate
pip install maturin pytest
maturin develop --release
pytest
```

## Uso

```python
import gravital_sound as gs

cfg = gs.Config(sample_rate=48000, channels=1, frame_duration_ms=20)
session = gs.Session(config=cfg, bind_addr="0.0.0.0", bind_port=0)
session.connect("192.0.2.10", 9000)

# Enviar un frame PCM16 mono de 20 ms (960 bytes).
session.send_audio(b"\x00\x10" * 480)

m = session.metrics()
print(f"MOS={m.estimated_mos:.2f} RTT={m.rtt_ms:.1f}ms loss={m.loss_percent:.1f}%")

session.close()
```

## Publicación

Pendiente (fase 8). Cuando llegue el momento:

```bash
maturin build --release --strip
twine upload dist/*
```
