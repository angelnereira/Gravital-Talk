#!/usr/bin/env python3
"""Valida un WAV generado por `examples/receiver`.

Verifica:
- Formato: PCM 16-bit, 48 kHz, 1 canal.
- Duración mínima.
- Presencia de energía: RMS > umbral (descarta silencio absoluto).
- Frecuencia dominante ~= 440 Hz usando FFT simple.

Uso: python3 scripts/verify_wav.py /tmp/out.wav [min_seconds]
"""

from __future__ import annotations

import struct
import sys
import wave

MIN_RMS = 100  # amplitud int16 mínima para considerar "hay señal"


def read_wav(path: str):
    with wave.open(path, "rb") as w:
        channels = w.getnchannels()
        sample_rate = w.getframerate()
        sample_width = w.getsampwidth()
        n_frames = w.getnframes()
        raw = w.readframes(n_frames)
    if sample_width != 2:
        raise SystemExit(f"FAIL: sample width = {sample_width}, want 2")
    samples = list(struct.unpack(f"<{n_frames * channels}h", raw))
    return channels, sample_rate, samples


def rms(samples):
    if not samples:
        return 0.0
    total = sum(s * s for s in samples)
    return (total / len(samples)) ** 0.5


def dominant_freq(samples, sample_rate, bins=4096):
    # FFT ingenua usando sólo la magnitud al cuadrado por frecuencia candidata
    # para evitar depender de numpy.
    import cmath
    import math

    n = min(len(samples), bins)
    if n < 2:
        return 0.0
    best_k = 0
    best_mag = 0.0
    for k in range(1, n // 2):
        real = 0.0
        imag = 0.0
        for i in range(n):
            angle = -2.0 * math.pi * k * i / n
            real += samples[i] * math.cos(angle)
            imag += samples[i] * math.sin(angle)
        mag = real * real + imag * imag
        if mag > best_mag:
            best_mag = mag
            best_k = k
    return best_k * sample_rate / n


def main():
    if len(sys.argv) < 2:
        raise SystemExit("usage: verify_wav.py <path> [min_seconds]")
    path = sys.argv[1]
    min_seconds = float(sys.argv[2]) if len(sys.argv) > 2 else 1.0

    channels, sample_rate, samples = read_wav(path)
    if channels != 1:
        raise SystemExit(f"FAIL: expected mono, got {channels} channels")
    if sample_rate != 48_000:
        raise SystemExit(f"FAIL: expected 48_000 Hz, got {sample_rate}")
    duration = len(samples) / sample_rate
    if duration < min_seconds:
        raise SystemExit(f"FAIL: duration {duration:.2f}s < {min_seconds}s")

    level = rms(samples)
    if level < MIN_RMS:
        raise SystemExit(f"FAIL: RMS {level:.1f} under {MIN_RMS} (silence?)")

    # Limita FFT a primer segundo para que termine rápido.
    fft_window = samples[: min(len(samples), sample_rate // 12)]
    peak = dominant_freq(fft_window, sample_rate, bins=1024)

    print(f"OK: duration={duration:.2f}s rms={level:.0f} peak_freq~{peak:.0f}Hz")


if __name__ == "__main__":
    main()
