"""Gravital Sound — Python SDK.

API de alto nivel que envuelve el módulo nativo `_gravital_sound`.

Ejemplo:

    import gravital_sound as gs

    session = gs.Session(config=gs.Config(sample_rate=48000, channels=1))
    session.connect("127.0.0.1", 9000)
    session.send_audio(b"\\x00" * 1920)
    metrics = session.metrics()
    print(metrics.estimated_mos)
    session.close()
"""

from ._gravital_sound import (  # noqa: F401
    Config,
    Metrics,
    Session,
    __version__,
    PROTOCOL_VERSION,
)

__all__ = [
    "Config",
    "Metrics",
    "Session",
    "__version__",
    "PROTOCOL_VERSION",
]
