# AGENTS.md — Gravital Talk

Contexto para agentes de código que trabajan en este repositorio.

## Qué es el proyecto

`Gravital-Talk` es un protocolo de comunicación de audio **Push-To-Talk** en tiempo real escrito en Rust.
Transporte UDP con cifrado de extremo a extremo (X25519 ECDH → HKDF-SHA256 → ChaCha20-Poly1305), emparejamiento
P2P por QR sin servidor obligatorio (STUN + relay opcional), control de congestión, FEC, jitter buffer y métricas
de calidad (RTT, jitter, pérdida, MOS).

Restricción de diseño dominante: **portabilidad universal**. El 100 % del protocolo vive en Rust; todo otro lenguaje
consume la ABI C estable de `gravital-talk-ffi`. Estado: **alpha** (`0.3.0-alpha.1`).

## Layout del workspace

```
Cargo.toml            # workspace: 9 crates, resolver 2, release LTO+panic=abort+strip
rust-toolchain.toml   # canal fijado 1.99.0 (rustfmt, clippy, target wasm32)
Makefile              # targets de build reproducibles (ver abajo)
compose.yaml          # perfiles docker: relay, e2e, test, bench, perf, security, observability

crates/
  gravital-talk-core/       # no_std: header 24B, packet/message, FSM, crypto, fragment, CRC-16, floor
  gravital-talk-metrics/    # RTT, jitter RFC 3550, loss/reorder, MOS (atomics, lock-free)
  gravital-talk-transport/  # tokio: UDP, STUN, jitter buffer, FEC, anti-replay, congestion, Noise
  gravital-talk-codec/      # Encoder/Decoder traits, PcmCodec, Opus (feature `opus`)
  gravital-talk-io/         # cpal: capture/playback (ALSA/CoreAudio/WASAPI), resampler rubato
  gravital-talk/            # facade: re-exports core+metrics+transport+codec, `CodecSession`
  gravital-talk-ffi/        # ABI C `gs_*` + JNI bridge (feature `android`) + cbindgen header
  gravital-talk-cli/        # binario `gs` (send/receive/ptt/relay/room/discover/bench/info/doctor/devices)
  gravital-talk-relay/      # lib + binario `gs-relay`: UDP+WS, /metrics, /healthz, features `grpc`/`tls`/`web`
  gravital-talk-audit/      # harness de auditoría `gs-audit` (verifica audio real, nunca se publica)

app/      # App Flutter multiplataforma (dev.gravitaltalk.gravital_talk_app)
android/  # App Android Kotlin/JNI independiente (com.gravitaltalk) — NO es app/android
sdks/     # python (PyO3/maturin) y web (wasm-bindgen) — EXCLUIDOS del workspace Cargo
proto/gravital/v1/  # contratos gRPC del plano de control (server_control, pairing)
fuzz/     # cargo-fuzz: fuzz_packet_parse, fuzz_floor_payload, fuzz_message_type
docs/     # spec del protocolo + ADRs (ver "Documentación")
infra/    # terraform (aws/hetzner/digitalocean), helm, grafana, cloud-init
scripts/  # build-android, flutter-android-libs, flutter-ios-lib, publish-order, verify_wav
docker/   # Dockerfile, Dockerfile.dev, Dockerfile.fuzz, scripts/{room-host,room-join,netem-join,fuzz}.sh
```

## Requisitos del entorno

- Rust **1.99** (MSRV declarado en `Cargo.toml`, fijado en `rust-toolchain.toml`).
- Deps del sistema para el workspace completo: `libopus-dev libasound2-dev pkg-config` (Debian/Ubuntu),
  `brew install opus` (macOS).
- Sin esas deps, `gravital-talk-codec` (feature `opus`) y `gravital-talk-io` (cpal/ALSA) **no compilan**.
  Para iterar rápido sin audio: `cargo check/test -p gravital-talk-core -p gravital-talk-metrics -p gravital-talk-transport`.
- Java 17+ + Android SDK/NDK 26.3.11579264 + `cargo-ndk` solo para APK.
- `maturin` + `pytest` para el SDK Python; `wasm-pack` para el SDK web; `cross` para cross-compile a Linux.

## Comandos

### Gate completo (equivalente a `CONTRIBUTING.md`)

```bash
make fmt-check     # cargo fmt --all -- --check
make clippy        # cargo clippy --workspace --all-targets -- -D warnings -W clippy::perf -W clippy::nursery
make test          # cargo test --workspace --all-targets
make cross-wasm    # cargo check --target wasm32-unknown-unknown -p gravital-talk-core --no-default-features
```

CI (`.github/workflows/ci.yml`) corre además, y es la referencia real:

```bash
cargo test -p gravital-talk-core -p gravital-talk-metrics -p gravital-talk-transport --no-default-features
cargo check --target wasm32-unknown-unknown -p gravital-talk-core
cargo check --target aarch64-unknown-linux-gnu --no-default-features -p gravital-talk-core -p gravital-talk-transport
cargo build -p gravital-talk-ffi --release && cc -I crates/gravital-talk-ffi/include -c crates/gravital-talk-ffi/tests/c_smoke.c
```

`RUSTFLAGS=-Dwarnings` está activo en CI: cualquier warning rompe el build.

### Otros targets útiles

```bash
make build / dev / bench / doc / clean / help
make ffi-smoke         # regenera header + compila y ejecuta el smoke test en C
make python-sdk / python-test / web-sdk
make loopback          # cargo run --release --example loopback
make audit / audit-parallel / docker-audit
make docker-e2e / docker-test / docker-bench / docker-perf / docker-fuzz / docker-obs / docker-down
```

### Fuera de cargo

```bash
cd android && make apk-debug        # cargo-ndk 3 ABIs + ./gradlew assembleDebug
./scripts/build-android.sh [--all-abis|--release]
cd app && flutter test && flutter analyze && flutter build apk --debug
cd sdks/python && maturin develop --release && pytest -v
cd sdks/web && npm run build        # wasm-pack; `npm run tsc` sólo typecheck
```

## Convenciones (obligatorias)

1. **`gravital-talk-core` es `no_std`.** Sólo `core` + `alloc` (bajo feature `alloc`). Cualquier dependencia de
   `std` va a otro crate. `cargo check --no-default-features -p gravital-talk-core` debe seguir pasando.
2. **Cero allocs en el hot path.** Nada de `Box::new`/`Vec::push`/`String::from` dentro de encode/decode o
   send/recv; usa buffers pre-asignados, `smallvec`, `bytes`.
3. **Transiciones de estado explícitas.** La FSM de sesión usa tipos marcador; un estado nuevo implica una
   transición type-safe, no un `if` en runtime.
4. **La FFI es API pública estable.** La firma de una función `gs_*` es un breaking change. Todo export es
   `extern "C"` con prefijo `gs_`, tipos `Gs*`, constantes `GS_*`, handles opacos y `gs_error_last()`.
   Regenera el header con cbindgen (build.rs → `crates/gravital-talk-ffi/include/gravital_talk.h`, versionado:
   si cambias la ABI, commitea el header regenerado).
5. **`unsafe` exige comentario `// SAFETY:`** justificando las invariantes. `unsafe_op_in_unsafe_fn = "deny"`
   ya está en varios `Cargo.toml` vía `[lints.rust]`.
6. **Naming:** crates `gravital-talk-*`, CLI binario `gs`, relay `gs-relay`, paquete Python `gravital-talk`,
   npm `@gravital/talk-web`, Android `com.gravitaltalk` (Kotlin) / `dev.gravitaltalk.gravital_talk_app` (Flutter).
7. **Idioma:** identificadores, API, flags de CLI y commits en inglés; **comentarios y documentación en español**
   (es lo que usa el código existente). Sin emojis.
8. **Commits:** Conventional Commits `tipo(scope): descripción`, descripción en español.
   Ej.: `feat(transport): añadir backoff exponencial a handshake`.
9. **Todo cambio visible al usuario va en `CHANGELOG.md`** (Keep a Changelog) bajo `[Unreleased]`.

## Auditoría de audio real (`gs-audit`)

La suite verifica que el protocolo funciona; `gs-audit` verifica que el **audio** funciona.
Un test que confirma "llegaron N frames" no distingue audio válido de bytes corruptos.

```bash
make audit           # par P2P 1:1, turnos partidos, ambas direcciones + contenido del audio
make audit-parallel  # N sesiones P2P simultáneas, frecuencias distintas (detecta cross-talk)
make docker-audit    # contenedor: añade escenarios de sala vía relay y codec Opus
```

Criterios que evalúa (dentro del proceso, `exit != 0` si fallan): frames rx/tx mínimos,
eventos de PTT remoto observados, y **frecuencia + RMS + saturación del PCM recibido**.
Requiere `--frame-ms 10` con PCM: el tope duro `MAX_PAYLOAD_SIZE` (1176 B) no admite un
frame PCM de 20 ms (1920 B). Ver la nota de `[Unreleased]` en `CHANGELOG.md`.

## Testing por capa

- **core** → property tests con `proptest` (roundtrip encode/decode) en `tests/prop_decode.rs`;
  el resto en `#[cfg(test)]` inline (~77 tests).
- **transport** → tests de integración con dos sockets en localhost reales
  (`tests/{noise_session,prop_fec,replay_session}.rs`).
- **facade/relay** → tests de integración end-to-end sobre UDP loopback
  (`crates/gravital-talk/tests/{handshake_flow,net_sim,opus_roundtrip,ptt_floor_control,session_lifecycle,stress}.rs`,
  `crates/gravital-talk-relay/tests/{grpc_control,relay_routed_session}.rs`).
- **FFI** → extender `crates/gravital-talk-ffi/tests/c_smoke.c` (lo valida `make ffi-smoke`).
- **cambios en hot path** → `make bench` (criterion) y comparar contra la baseline de `docs/benchmarks.md`;
  regresión ≥ 10 % requiere justificación escrita.
- **fuzzing** → `make docker-fuzz` o `FUZZ_SECONDS=90 make docker-fuzz`.

## Documentación

- `docs/protocol-spec.md`, `docs/packet-format.md`, `docs/session-model.md` — el formato de header es **24 bytes**;
  el header es AAD del AEAD y el nonce de 96 bits deriva de `sequence` + `session_id`. Cualquier cambio wire
  requiere actualizar spec + packet-format + CHANGELOG.
- `docs/security.md` (modelo de amenazas), `docs/pairing.md` (QR/STUN), `docs/grpc-evaluation.md` (plano de
  control: el audio **nunca** pasa por gRPC), `docs/benchmarks.md`, `docs/roadmap.md`, `docs/adr/`.
- Contratos gRPC en `proto/gravital/v1/`; el cliente Dart generado vive en `app/lib/generated/` (**generado, no
  editar**; instrucciones en `proto/README.md`).

## Gotchas conocidos

- `make pgo-build` invoca `scripts/build-pgo.sh`, que **no existe** en el repo.
- `make doc` usa `--open` (abre navegador; no sirve en headless/CI).
- `sdks/python` y `sdks/web` son workspaces Cargo independientes con su propio `Cargo.lock`; `cargo` en la raíz
  no los toca. También tienen versions propias (PyO3 0.23 en python vs 0.22 pineado en el workspace).
- El facade `gravital-talk` **no** re-exporta `gravital-talk-io`; úsalo como dependencia directa.
- `gravital-talk-ffi` habilita `std` del facade pero **no** `opus`; el build de release del FFI es el que genera
  el header cbindgen.
- `app/lib/services/native_bridge_io.dart` busca el `.so/.dylib/.dll` en rutas de dev (`../target/{release,debug}`).
  Si no lo encuentra, la app cae al `DemoSessionEngine` (seno) en lugar de fallar — no lo confundas con un bug.
- `docker compose --profile observability` expone Prometheus :9091 y Grafana :3001; el relay expone 9000/udp,
  9090/ws, 9100/http.
- El relay con gRPC requiere `--features grpc` (el `Dockerfile` lo acepta vía `--build-arg CARGO_FEATURES`).