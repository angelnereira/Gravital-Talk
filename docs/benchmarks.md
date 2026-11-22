# Benchmarks y targets de rendimiento

Baseline de referencia capturado el **2026-11-22** sobre el código en el commit
`012f9f4`. Los números son la **mediana** del rango [lower bound, estimate,
upper bound] que reporta criterion; el throughput es la mediana de ese rango.

## Entorno y reproducibilidad

| Dato | Valor |
| --- | --- |
| Fecha | 2026-11-22 |
| Commit medido | `012f9f4` |
| Sistema | Linux 7.0.0-44-generic x86_64 |
| Rust | `rustc 1.99.0 (b940084d7 2026-09-28)` |
| Perfil | `release` del workspace (LTO fat, `codegen-units=1`, `panic=abort`, `strip`) |
| Comando | `cargo bench -p gravital-talk --bench checksum --bench encode_decode -- --save-baseline docs44` |

Los datos crudos de criterion quedan en `target/criterion/` (no versionado), bajo
el nombre de baseline `docs44`. Para repetir la captura, usa el mismo comando y
cambia el nombre del baseline; luego actualiza esta tabla y el commit medido.

## Primitivas de framing — `benches/encode_decode.rs`

| Bench | Tiempo (ns) | Throughput | Target |
| --- | --- | --- | --- |
| `packet/encode_960B` | 1 928 | 474,9 MiB/s | <= 5 µs |
| `packet/decode_960B` | 1 920 | 476,8 MiB/s | <= 5 µs |

Incluye header 24 B, payload 960 B (20 ms PCM16 mono @ 48 kHz), trailer 4 B.
El CRC-16 sigue dominando el coste: ~2,1 µs para 1024 B frente a ~1,9 µs de
encode o decode.

## CRC-16/CCITT-FALSE — `benches/checksum.rs`

| Payload | Tiempo (ns) | Throughput |
| --- | --- | --- |
| 64 B | 134 | 456,8 MiB/s |
| 512 B | 1 106 | 441,4 MiB/s |
| 1 024 B | 2 122 | 460,2 MiB/s |
| 2 048 B | 4 088 | 477,8 MiB/s |

Throughput ~440–480 MiB/s ≈ 2,1–2,3 ns/byte. Implementación con lookup table
de 256 entradas y sin SIMD. La feature `simd-crc` (Fase 5+) podría bajar esto
2–3× con PCLMULQDQ (idea futura, no implementada).

## Mediciones históricas (no recapturadas en este baseline)

### Loopback encode→decode — `examples/loopback.rs`

Ejecución con 1 000 000 iteraciones, payload 960 B, warmup 10 000:

```
min   : 5 256 ns
p50   : 5 331 ns
p95   : 5 455 ns
p99   : 9 911 ns
p99.9 : 17 471 ns
max   : 530 431 ns
```

El p50 de 5,3 µs incluye dos llamadas a `Instant::now()` (~30 ns cada una)
y la iteración completa. La medición real de encode+decode sola es
≈ 1,9 + 1,9 = 3,8 µs en el baseline actual, consistente con el `p50`
observado en aquella máquina. Target p99 < 1 ms cumplido con gran margen.

### Handshake 3-way localhost — `tests/session_lifecycle.rs`

Tiempo wall-clock medio: ≈ 1 ms (tres roundtrips UDP localhost).

## Quality gates para CI

- Regla del repo (`AGENTS.md`): una regresión **>= 10 %** requiere justificación
  escrita en el descriptor del cambio.
- Umbral propuesto para el gate de CI (issue #43): **> 25 %** falla el job;
  entre 10 % y 25 % es aviso con evidencia.
- Para actualizar esta tabla, re-ejecuta el comando del entorno, copia las
  medianas de criterion y cambia el commit medido. Nunca se actualiza
  automáticamente desde un PR: exige revisión humana.
