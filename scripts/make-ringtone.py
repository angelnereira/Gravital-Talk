#!/usr/bin/env python3
"""Genera el tono de llamada de la app (`gt_ring.wav`).

No es un timbre de teléfono: son cuatro notas suaves de campana sintética
(un arpegio ascendente, Re mayor con novena) que se apagan solas, y un
silencio para que al repetirse en bucle respire.

    scripts/make-ringtone.py app/android/app/src/main/res/raw/gt_ring.wav
"""
import math
import struct
import sys
import wave

RATE = 44_100
LOOP_S = 2.6
# (segundo de entrada, frecuencia en Hz, volumen)
NOTES = [
    (0.00, 587.33, 0.55),   # Re5
    (0.16, 739.99, 0.50),   # Fa#5
    (0.32, 880.00, 0.50),   # La5
    (0.52, 1318.51, 0.42),  # Mi6
]


def bell(t, freq):
    """Campana blanda: fundamental, dos parciales y un ataque corto."""
    if t < 0:
        return 0.0
    attack = min(1.0, t / 0.008)
    body = math.exp(-t * 3.2)
    shimmer = math.exp(-t * 9.0)
    w = 2 * math.pi * freq * t
    return attack * (
        body * math.sin(w)
        + 0.35 * body * math.sin(2 * w)
        + 0.18 * shimmer * math.sin(3.01 * w)
    )


def main(path):
    n = int(RATE * LOOP_S)
    samples = []
    for i in range(n):
        t = i / RATE
        v = sum(vol * bell(t - start, freq) for start, freq, vol in NOTES)
        # El final a cero exacto, para que el bucle no haga clic.
        fade = min(1.0, (LOOP_S - t) / 0.05)
        samples.append(max(-1.0, min(1.0, v * 0.5 * fade)))
    with wave.open(path, "wb") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(RATE)
        w.writeframes(b"".join(struct.pack("<h", int(s * 32767)) for s in samples))


if __name__ == "__main__":
    main(sys.argv[1])
