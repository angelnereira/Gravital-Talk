# Changelog

Todos los cambios notables de Gravital Talk se documentan aquí. El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y el proyecto usa [SemVer](https://semver.org/lang/es/).

## [0.2.0-alpha.2] — 2026-04-25

Track A.1 (follow-ups Fase 5) + Track C (relay productivo) + Track D parcial (release infrastructure) + Track E (Infraestructura as Code con Terraform/Helm).

### Added

**Track A.1 — Negociación de codec en handshake (`transport`, `facade`)**
- `Config.supported_codecs: Vec<u8>` lista los codecs aceptables del lado server.
- Server elige `codec_preferred` del client si está en su lista; si no, hace fallback al primer codec local.
- Client valida que el codec aceptado esté en su propia lista; aborta con `Handshake("server selected unsupported codec")` si no.
- `Session::negotiated_codec()` y `Session::config()` expuestos para capas superiores.
- `CodecSession::handshake()` valida coincidencia con su `codec_id` y retorna `CodecMismatch { requested, negotiated }` si difiere.
- `CodecSession::new()` ahora sincroniza automáticamente `config.codec_preferred` con el codec elegido.
- 3 tests integración: fallback ok, rechazo cliente, mismatch CodecSession.

**Track A.1 — Resampler (`gravital-talk-io`)**
- `Resampler::new(in_rate, out_rate, channels, out_frames_per_channel)` basado en `rubato::FftFixedOut`.
- Conversión `i16 → f32 → resample → i16` con buffers reutilizables (zero-alloc en hot path).
- Soporta channels arbitrarios (deinterleave/interleave automático).
- 3 tests: 44.1→48 kHz preserva energía, 48→48 kHz produce salida, rechaza 0 channels.

**Track C — Crate `gravital-talk-relay`** (relay productivo stand-alone)
- Servidor que acepta tráfico UDP **y** WebSocket en el mismo proceso.
- `Router` con `DashMap<u32, RouteEntry>` y políticas: 2 peers/sesión, drop de tercer peer interviniendo, GC de sesiones idle por TTL.
- Loop UDP usando `PacketView` para extraer `session_id` sin parsear más.
- Bridge WebSocket con `tokio-tungstenite`; cada conexión es endpoint válido al mismo router (cross-transport relay browser↔native).
- HTTP `/metrics` (Prometheus text format) y `/healthz` en `hyper 1.x`.
- Métricas: `gs_relay_packets_in/out_total`, `bytes_in/out_total`, `active_sessions`, `ws_connections`, `dropped_total{reason}`.
- Config TOML con override por flags CLI (`--udp-bind`, `--ws-bind`, `--observability-bind`, `--config`).
- Defaults sensatos: UDP 9000, WS 9090, observabilidad 9100, TTL 5min, max 10k sessions.
- Binario `gs-relay` con manejo de Ctrl-C y GC thread (cada 30s).
- 9 tests unitarios.

**Track C — Containerización del relay**
- `Dockerfile` multi-stage (rust:1.78-slim builder → debian:bookworm-slim runtime con usuario `gravital` uid 10001).
- `docker-compose.yml` con relay + Prometheus para stack local de testing.
- `prometheus.yml` con scrape config preconfigurado.
- `relay.example.toml` con todos los campos comentados.

**Track D — Workflows CI/CD**
- `.github/workflows/release.yml`: disparado por tags `v*`, construye binarios `gs` para `linux-x86_64`, `linux-aarch64`, `macos-x86_64`, `macos-aarch64`, `windows-x86_64`. Empaqueta tar.gz/zip con README+LICENSE+CHANGELOG. Construye wheels Python (manylinux + macOS + Windows) vía maturin. Construye bundle WASM vía wasm-pack. Crea draft release y lo publica solo si todos los jobs pasan.
- `.github/workflows/docs.yml`: en cada push a main y tag `v*`, publica `cargo doc --workspace --all-features` a GitHub Pages con redirect de raíz a `gravital_talk/`. Usa `RUSTDOCFLAGS=-D warnings` para garantizar que docs no se rompan en silencio.
- `.github/workflows/terraform.yml`: en cambios bajo `infra/terraform/**`, ejecuta `terraform fmt -check -recursive`, `terraform validate` por cada módulo (matrix), `tflint --recursive` y `checkov` security scan (soft fail).
- `.github/workflows/ci.yml`: triggers extendidos a `feat/**` y `verify/**` + `workflow_dispatch` para relanzar desde la UI.

**Track E.1 — Módulos Terraform multi-cloud (`infra/terraform/modules/`)**
- `relay-aws`: EC2 t4g.small (ARM64, ~$12/mes), Security Group con UDP/WS abiertos, Route53 record A opcional, Debian 12, IMDSv2 obligatorio, EBS gp3 cifrado. Outputs estandarizados (`relay_endpoint`, `udp_port`, `ws_url`).
- `relay-hetzner`: CX22 (~€4/mes), Cloud Firewall, IPv4+IPv6, datacenters EU+US. La opción más barata para self-host.
- `relay-digitalocean`: Droplet con DO Firewall, monitoring activado, 13 regiones globales (~$6/mes el más chico).

**Track E.3 — Edge nodes**
- `infra/terraform/modules/edge-node`: produce `user_data` cloud-init agnóstico que cualquier provider de compute puede consumir. Configura systemd unit con el daemon `gs send` capturando del mic local, `Nice=-5` para latencia.
- `infra/cloud-init/raspberry-pi.yml`: cloud-config descargable directo a SD card (`/boot/firmware/user-data`) para Raspberry Pi 4/5 con Pi OS Lite ARM64. Instala libopus, libasound, configura UFW y systemd unit que arranca tras editar `/etc/default/gravital-talk`.
- `infra/terraform/examples/single-region-aws` y `self-hosted-hetzner` con `terraform apply` listo.

**Track E.2 — Helm chart `gravital-talk-relay`**
- Chart.yaml v0.1.0 con appVersion = 0.2.0-alpha.1.
- `Deployment` con `securityContext` estricto (`runAsNonRoot`, `readOnlyRootFilesystem`, drop ALL capabilities), liveness/readiness en `/healthz`.
- `Service` `LoadBalancer` con `externalTrafficPolicy: Local` (preserva IP del cliente para rate limiting).
- `Service` separado `ClusterIP` solo para `/metrics` (no expone al exterior).
- `ServiceMonitor` opcional compatible con prometheus-operator.
- `HorizontalPodAutoscaler` por CPU (autoscaling.enabled=false por defecto).
- `ConfigMap` que monta `config.toml` derivado de `values.yaml`.

**Track E.4 — Dashboards Grafana**
- `infra/grafana/dashboards/gravital-fleet-overview.json`: stat panels (sesiones, conexiones WS, paquetes/s, Mbit/s), timeseries (throughput in vs out, drop reasons, sesiones activas histórico), filtro por instance.
- Compatible con la convención `grafana_dashboard=1` de kube-prometheus-stack.

**Documentación**
- `infra/README.md` con tabla comparativa de proveedores, runbook de operaciones (health, métricas, update, troubleshooting).
- `infra/terraform/modules/relay-aws/README.md` con todas las variables y outputs.
- `infra/helm/gravital-talk-relay/README.md` con 3 modos de instalación.
- `infra/grafana/README.md` con guía de import.
- README principal completamente reescrito reflejando el alcance actual.

### Changed
- README principal: nuevo árbol de directorios, tabla de estado expandida, quickstarts para relay y Terraform, sección de CI/CD.

### Notes
- El relay actual no tiene cifrado ni rate limiting (planificado para 0.3 con Noise Protocol y tower-rate-limit).
- El HPA del Helm chart está basado en CPU; autoscaling por sesiones activas requiere prometheus-adapter (documentado).
- Routing del relay es per-pod (en memoria); para escalar horizontalmente con peers en pods distintos se necesita backend compartido (Redis/etcd) — pendiente.
- Terraform: ningún módulo se valida automáticamente sin proveedor configurado en el sandbox local; CI lo valida con `terraform validate` + `tflint` + `checkov`.

## [0.2.0-alpha.1] — 2026-04-24

Fase 5 completa — Track A: codec Opus + audio hardware + CLI de producción.

### Added

**Crate `gravital-talk-codec`**
- Traits `Encoder` / `Decoder` (`Send`, frame-granular) con `CodecId` negociable.
- `PcmCodec` — passthrough i16-LE, zero-copy.
- `OpusCodec` — wrapper sobre libopus vía `audiopus`; `Application::Voip`, 64 kbps, FEC, PLC.
- `build_pair(id, sample_rate, channels, frame_ms)` — factory ergonómica.
- 9 tests unitarios: roundtrip PCM, roundtrip Opus, frame-size validation, rate/channel rejection.

**Crate `gravital-talk-io`**
- `AudioCapture::start(config, device_hint)` — captura desde micrófono vía cpal (ALSA/CoreAudio/WASAPI). Entrega `mpsc::Receiver<Vec<i16>>` con frames de tamaño fijo.
- `AudioPlayback::start(config, device_hint)` — playback a altavoz con pump thread desacoplado del callback de tiempo real.
- `list_input_devices()` / `list_output_devices()` — enumeración de devices con flag `is_default`.

**Crate `gravital-talk` (facade)**
- `CodecSession` — wrapper de alto nivel sobre `Session` + `Encoder`/`Decoder`:
  - `send_samples(&[i16])` — codifica y envía.
  - `recv_samples() -> Vec<i16>` — recibe y decodifica.
- Re-exports de `CodecId`, `CodecError`, `Encoder`, `Decoder`, `PcmCodec` (+ `OpusCodec` con feature `opus`).
- Feature `opus` (por defecto activada) propaga a `gravital-talk-codec/opus`.
- Ejemplos `mic_to_speaker` (latencia e2e con hdrhistogram) y `voip_peer` (full-duplex bidireccional).
- Integration test `opus_roundtrip`: PCM SNR > 60 dB, Opus energía > 10 % original.
- Benchmark `opus_encode`: PCM y Opus encode/decode criterion.

**CLI `gs`**
- `gs send --device <name> --codec <pcm|opus>` — captura desde micrófono o genera sinusoidal/WAV.
- `gs receive --device <name> --codec <pcm|opus>` — escribe WAV + reproduce por altavoz en paralelo.
- `gs devices` — lista input/output devices del sistema.

**CI**
- Instala `libopus-dev libasound2-dev pkg-config` en jobs Ubuntu.
- Instala `opus` vía Homebrew en el job macOS.
- Job `test-no-default-features` valida que `core`, `metrics` y `transport` compilan sin features extra.
- Cross-check aarch64 usa `--no-default-features` para evitar dependencias de libopus.

**Docs**
- `docs/codecs.md` — arquitectura de codecs, rangos de bitrate, negociación, extensión.
- `docs/audio-io.md` — diseño de captura/playback, backpressure, sample-rate mismatch, CI headless.
- `docs/adr/006-opus-codec.md` — decisión de usar Opus + alternativas descartadas.
- `docs/adr/007-cpal-audio-io.md` — decisión de usar cpal + diseño del adaptador RT.

### Changed
- `gravital-talk-transport::session::handshake_server` rechaza paquetes de peers no esperados (hardening).
- `Cargo.toml` del workspace: `gravital-talk-codec` y `gravital-talk-io` añadidos como members y deps.

### Notes
- La negociación automática de codec en el handshake wire llega en Track B.
- El resampling automático por sample-rate mismatch llega en Track B (hoy emite `warn`).
- El protocolo sigue siendo `draft` hasta `0.1.0` final.

## [Unreleased]

### Roadmap
Trabajo planificado para próximas versiones (referencia cruzada con `seed.md`):

- **Fase 5 completa.** Integración del codec Opus (`gravital-talk-codec`) y audio I/O real vía `cpal` (`gravital-talk-io`) con backends ALSA, CoreAudio, WASAPI, AAudio.
- **Fase 6 ampliada.** SDKs adicionales: Swift (XCFramework + SPM), Kotlin (AAR + JNI), Node.js (napi-rs).
- **Fase 7.** Relay server productivo con Docker, NAT traversal, balanceo por `session_id`.
- **Fase 8.** Publicación a crates.io, PyPI, npm, Maven Central, SPM; landing page en `gravitalsound.dev`.
- Fuzz targets con `cargo-fuzz` y fuzzing continuo.
- Transport WebTransport sobre QUIC cuando los navegadores lo estabilicen.
- Paquetes `.deb` / `.rpm` para CLI y daemon.
- Tests de verificación formal con `kani` integrados en CI semanal.
- Backend DPDK/AF_XDP para kernel bypass (opcional, Linux servers).

## [0.1.0-alpha.1] — 2026-04-19

Release inicial alpha. Establece la base arquitectónica del protocolo y una implementación funcional end-to-end en Rust nativo + SDKs Python y Web/WASM.

### Added

**Protocolo y documentación**
- Especificación formal del protocolo en `docs/protocol-spec.md`.
- Formato binario del paquete documentado con diagramas de bits en `docs/packet-format.md`.
- Modelo de sesión con máquina de estados en `docs/session-model.md`.
- Justificación del transporte (UDP-first) en `docs/transport.md`.
- Modelo de amenazas inicial en `docs/security.md`.
- Estrategia de portabilidad en `docs/portability.md`.
- ADRs 001-005 con las decisiones arquitectónicas fundacionales.

**Crates Rust**
- `gravital-talk-core` (`no_std` compatible): `PacketHeader` de 24 bytes, `MessageType`, `Packet<'a>` zero-copy, `SessionState` type-safe, CRC-16/CCITT-FALSE con aceleración SIMD opcional, fragmentación/reensamblado.
- `gravital-talk-metrics`: RTT con EWMA, jitter (RFC 3550), pérdida con bitmap window de 64 paquetes, estimador MOS-LQ, contadores atómicos lock-free.
- `gravital-talk-transport`: trait `Transport` async, `UdpTransport` con tuning de socket (`SO_REUSEADDR`, `SO_REUSEPORT`, buffers 4 MB, DSCP EF), `WebSocketTransport`, jitter buffer lock-free SPSC, orquestador de handshake 3-way.
- `gravital-talk-ffi`: exports `extern "C"` con prefijo `gs_`, generación automática del header C con `cbindgen`, handles opacos.
- `gravital-talk-cli`: binario `gs` con subcomandos `send`, `receive`, `bench`, `info`, `doctor`, `relay`.
- `gravital-talk` (facade): re-exporta la API ergonómica.

**SDKs**
- **Python** vía PyO3 + `maturin`: clases `Session`, `Config`, `Metrics`. Test de loopback con `pytest`.
- **Web/WASM** vía `wasm-bindgen`: `GravitalTalkSession` con transport WebSocket (delegado a JS). Demo de navegador en `sdks/web/examples/browser-demo`.

**Tooling**
- Workspace Cargo con `resolver = "2"` y perfil release agresivo (`lto = "fat"`, `codegen-units = 1`, `panic = "abort"`, `mimalloc`).
- `Cross.toml` para cross-compilation a aarch64, armv7, musl y wasm32.
- `Makefile` con targets para `build`, `test`, `clippy`, `bench`, `cross-*`, `ffi-smoke`, `python-test`, `web-sdk`, `pgo-build`.
- CI de GitHub Actions (`.github/workflows/ci.yml`): fmt, clippy estricto, test, cross-check para aarch64 y wasm32, smoke test de FFI, quality gate de regresión de benchmarks.

**Quality**
- Property testing con `proptest` sobre encode/decode de paquetes.
- Benchmarks con `criterion` + `iai` para header, checksum, jitter buffer, throughput.
- Test con `dhat` verificando zero-allocs en el hot path de send/recv.
- Histogramas HDR (`hdrhistogram`) para p50/p95/p99/p99.9 en el ejemplo `loopback`.
- `#![forbid(unsafe_code)]` en los crates donde es viable; el `unsafe` restante (FFI, SIMD) está marcado con `// SAFETY:` y justificado.

### Notes
- El codec inicial es PCM crudo. Opus queda para la siguiente fase.
- El audio I/O de hardware (mic/speaker) no se incluye; se suministran señales de prueba (seno) y lectura/escritura de WAV con `hound`.
- El protocolo es `draft` — pueden introducirse cambios incompatibles hasta `0.1.0` final.

[Unreleased]: https://github.com/angelnereira/gravital-talk/compare/v0.1.0-alpha.1...HEAD
[0.1.0-alpha.1]: https://github.com/angelnereira/gravital-talk/releases/tag/v0.1.0-alpha.1
