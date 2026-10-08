# Docker — Gravital Talk

Empaquetado del proyecto en contenedores: el servicio (relay), el cliente
(CLI), y el **laboratorio de pruebas**: simulación de comunicaciones entre
contenedores, benchmarks, rendimiento bajo red degradada y fuzzing de
seguridad.

## Quickstart

```bash
# 1) Levantar el relay (UDP 9000, WS 9090, HTTP 9100)
docker compose up -d relay

# 2) E2E multi-contenedor: relay + host + join headless con verificación
make docker-e2e

# 3) Suite de tests completa
make docker-test

# 4) Benchmarks criterion (resultados en target/criterion)
make docker-bench

# 5) Rendimiento bajo red simulada (tc netem: delay/jitter/pérdida)
make docker-perf

# 6) Fuzzing acotado (cargo-fuzz, nightly)
FUZZ_SECONDS=90 make docker-fuzz

# 7) Observabilidad (Prometheus :9091, Grafana :3001 admin/gravital)
make docker-obs
```

## Imágenes

| Imagen | Contenido |
|---|---|
| `docker/Dockerfile` (target `relay`) | `gs-relay` en runtime mínima (debian slim, usuario no-root) |
| `docker/Dockerfile` (target `cli`) | binario `gs` (con libopus/libasound runtime) |
| `docker/Dockerfile.dev` | toolchain Rust 1.99 + deps de sistema para tests/bench |
| `docker/Dockerfile.fuzz` | nightly + cargo-fuzz para los targets de `fuzz/` |

Builds con `--mount=type=cache` (BuildKit): las dependencias y el `target`
se cachean entre builds.

## Perfiles de compose

| Perfil | Servicios | Qué prueba |
|---|---|---|
| *(default)* | `relay` | Daemon con healthcheck (`/healthz`) |
| `e2e` | `room-host`, `room-join` | Comunicación inter-contenedor: sala real, audio cifrado ida y vuelta; el join falla si no recibe frames |
| `test` | `tester` | `cargo test --workspace --all-targets` (código montado, cachés en volúmenes) |
| `bench` | `bench` | `cargo bench --workspace` (criterion → `target/criterion`) |
| `perf` | `netem-join` | Mismo e2e pero con `tc netem` (delay 30 ms ±5, pérdida 1 %) para medir RTT/jitter/loss/MOS reportados por `gs ptt --headless` |
| `security` | `fuzz` | Smoke de fuzzing por target (presupuesto `FUZZ_SECONDS`) |
| `observability` | `prometheus`, `grafana` | Métricas del relay + dashboard del repo (`infra/grafana/dashboards`) |

## E2E en detalle

`room-host.sh` crea la sala en el relay (`POST /api/rooms`), publica el
room code en el volumen compartido `room-code:/shared` y transmite audio
senoidal headless. `room-join.sh` espera el código, se une y verifica que
recibe ≥ 1 frame; si no, sale con error (el `--exit-code-from room-join`
propaga el fallo al shell).

Los clientes usan `gs ptt --no-audio --headless --duration N`:
- `--no-audio`: fuente senoidal interna, sin hardware.
- `--headless`: sin UI de terminal; transmite automáticamente.
- Resumen final: `frames enviados/recibidos`, `rtt`, `jitter`, `loss`, `MOS`.

## Parámetros

Variables de entorno por servicio (ver `compose.yaml`):

| Variable | Default | Descripción |
|---|---|---|
| `RELAY_HOST` | `relay` | Hostname del servicio relay |
| `UDP_PORT` | `9000` | Puerto UDP de audio |
| `OBS_PORT` | `9100` | Puerto HTTP de salas |
| `SESSION_ID` | `305419896` | `session_id` de la sala de prueba |
| `DURATION` | 25/10 | Segundos de transmisión headless |
| `DELAY_MS`, `JITTER_MS`, `LOSS_PERCENT` | 30 / 5 / 1 | Simulación netem (perfil `perf`) |
| `FUZZ_SECONDS` | `60` | Presupuesto por fuzz target |
| `GS_RATE_LIMIT` | `0` | Paquetes/seg por IP (0 = ilimitado, anti-DoS) |
| `CARGO_FEATURES` | — | Features de build (p. ej. `--features grpc`) |

## gRPC (plano de control)

```bash
# Compilar el relay con el plano de control gRPC (tonic) y arrancarlo
CARGO_FEATURES="--features grpc" docker compose build relay
docker compose up -d relay
# El relay expone ServerControl + PairingService en :50051
```

El contrato está en `proto/gravital/v1/` y la evaluación en
`docs/grpc-evaluation.md`. Sin la feature, `--grpc-bind` no existe y el
puerto 50051 queda cerrado.

## Consejos

- Si Docker usa BuildKit antiguo sin soporte de cache mounts, elimina los
  `--mount=type=cache` de `docker/Dockerfile` (builds más lentos, misma imagen).
- El perfil `test`/`bench` monta el código fuente (`.:/src`): edita y
  re-ejecuta sin rebuild de imagen.
- `make docker-down` elimina los volúmenes (room codes, caché de cargo).