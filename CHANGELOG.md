# Changelog

Todos los cambios notables de Gravital Sound se documentan aquí. El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y el proyecto usa [SemVer](https://semver.org/lang/es/).

## [0.2.0-alpha.1] — 2026-04-24

Fase 5 completa — Track A: codec Opus + audio hardware + CLI de producción.

### Added

**Crate `gravital-sound-codec`**
- Traits `Encoder` / `Decoder` (`Send`, frame-granular) con `CodecId` negociable.
- `PcmCodec` — passthrough i16-LE, zero-copy.
- `OpusCodec` — wrapper sobre libopus vía `audiopus`; `Application::Voip`, 64 kbps, FEC, PLC.
- `build_pair(id, sample_rate, channels, frame_ms)` — factory ergonómica.
- 9 tests unitarios: roundtrip PCM, roundtrip Opus, frame-size validation, rate/channel rejection.

**Crate `gravital-sound-io`**
- `AudioCapture::start(config, device_hint)` — captura desde micrófono vía cpal (ALSA/CoreAudio/WASAPI). Entrega `mpsc::Receiver<Vec<i16>>` con frames de tamaño fijo.
- `AudioPlayback::start(config, device_hint)` — playback a altavoz con pump thread desacoplado del callback de tiempo real.
- `list_input_devices()` / `list_output_devices()` — enumeración de devices con flag `is_default`.

**Crate `gravital-sound` (facade)**
- `CodecSession` — wrapper de alto nivel sobre `Session` + `Encoder`/`Decoder`:
  - `send_samples(&[i16])` — codifica y envía.
  - `recv_samples() -> Vec<i16>` — recibe y decodifica.
- Re-exports de `CodecId`, `CodecError`, `Encoder`, `Decoder`, `PcmCodec` (+ `OpusCodec` con feature `opus`).
- Feature `opus` (por defecto activada) propaga a `gravital-sound-codec/opus`.
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
- `gravital-sound-transport::session::handshake_server` rechaza paquetes de peers no esperados (hardening).
- `Cargo.toml` del workspace: `gravital-sound-codec` y `gravital-sound-io` añadidos como members y deps.

### Notes
- La negociación automática de codec en el handshake wire llega en Track B.
- El resampling automático por sample-rate mismatch llega en Track B (hoy emite `warn`).
- El protocolo sigue siendo `draft` hasta `0.1.0` final.

## [Unreleased]

### Roadmap
Trabajo planificado para próximas versiones (referencia cruzada con `seed.md`):

- **Fase 5 completa.** Integración del codec Opus (`gravital-sound-codec`) y audio I/O real vía `cpal` (`gravital-sound-io`) con backends ALSA, CoreAudio, WASAPI, AAudio.
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
- `gravital-sound-core` (`no_std` compatible): `PacketHeader` de 24 bytes, `MessageType`, `Packet<'a>` zero-copy, `SessionState` type-safe, CRC-16/CCITT-FALSE con aceleración SIMD opcional, fragmentación/reensamblado.
- `gravital-sound-metrics`: RTT con EWMA, jitter (RFC 3550), pérdida con bitmap window de 64 paquetes, estimador MOS-LQ, contadores atómicos lock-free.
- `gravital-sound-transport`: trait `Transport` async, `UdpTransport` con tuning de socket (`SO_REUSEADDR`, `SO_REUSEPORT`, buffers 4 MB, DSCP EF), `WebSocketTransport`, jitter buffer lock-free SPSC, orquestador de handshake 3-way.
- `gravital-sound-ffi`: exports `extern "C"` con prefijo `gs_`, generación automática del header C con `cbindgen`, handles opacos.
- `gravital-sound-cli`: binario `gs` con subcomandos `send`, `receive`, `bench`, `info`, `doctor`, `relay`.
- `gravital-sound` (facade): re-exporta la API ergonómica.

**SDKs**
- **Python** vía PyO3 + `maturin`: clases `Session`, `Config`, `Metrics`. Test de loopback con `pytest`.
- **Web/WASM** vía `wasm-bindgen`: `GravitalSoundSession` con transport WebSocket (delegado a JS). Demo de navegador en `sdks/web/examples/browser-demo`.

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

[Unreleased]: https://github.com/angelnereira/gravital-sound/compare/v0.1.0-alpha.1...HEAD
[0.1.0-alpha.1]: https://github.com/angelnereira/gravital-sound/releases/tag/v0.1.0-alpha.1
