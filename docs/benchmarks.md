# Benchmarks y targets de rendimiento

Baseline capturado con `cargo bench --workspace` en hardware x86_64 moderno,
perfil `release` con LTO fat, codegen-units=1. Los números son la **mediana**
del rango [lower bound, estimate, upper bound] que reporta criterion.

## Primitivas de framing — `benches/encode_decode.rs`

| Bench                 | Tiempo (ns) | Throughput      | Target |
| --------------------- | ----------- | --------------- | ------ |
| `packet/encode_960B`  | 2 906       | 315 MiB/s       | ≤ 5 µs |
| `packet/decode_960B`  | 2 789       | 328 MiB/s       | ≤ 5 µs |

Incluye header 24 B, payload 960 B (20 ms PCM16 mono @ 48 kHz), trailer 4 B.
El CRC-16 domina el tiempo: ≈ 2.9 µs de 3 µs totales.

## CRC-16/CCITT-FALSE — `benches/checksum.rs`

| Payload | Tiempo (ns) | Throughput      |
| ------- | ----------- | --------------- |
| 64 B    | 188         | 325 MiB/s       |
| 512 B   | 1 500       | 325 MiB/s       |
| 1 024 B | 3 020       | 323 MiB/s       |
| 2 048 B | 6 092       | 321 MiB/s       |

Throughput constante ~320 MiB/s = ~4 ns/byte. Implementación con lookup table
de 256 entradas y sin SIMD. La feature `simd-crc` (Fase 5+) bajaría esto
2-3× con PCLMULQDQ.

## Loopback encode→decode — `examples/loopback.rs`

Ejecución con 1 000 000 iteraciones, payload 960 B, warmup 10 000:

```
min   : 5 256 ns
p50   : 5 331 ns
p95   : 5 455 ns
p99   : 9 911 ns
p99.9 : 17 471 ns
max   : 530 431 ns
```

El p50 de 5.3 µs incluye dos llamadas a `Instant::now()` (~30 ns cada una)
y la iteración completa. La medición real de encode+decode sola es
≈ 2.9 + 2.8 = 5.7 µs (de criterion), consistente con el `p50` observado.
Target p99 < 1 ms cumplido con gran margen.

## Handshake 3-way localhost — `tests/session_lifecycle.rs`

Tiempo wall-clock medio: ≈ 1 ms (tres roundtrips UDP localhost).

## Quality gates para CI

Un regressor excede el target si los valores crecen más de **25 %** vs
esta baseline; entre 10 % y 25 % es warning. Para actualizar esta tabla,
re-ejecuta `cargo bench --workspace` y copia los `time:` centrales.
