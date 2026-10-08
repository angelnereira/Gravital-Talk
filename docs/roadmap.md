# Hoja de ruta Gravital Talk

Versiones objetivo con criterios de salida por hito. La serie `0.x` mantiene
compatibilidad del wire protocol v1 (los cambios de contrato se negocian, no
se rompen).

## Visión por versiones

| Versión | Tema | Qué entrega | Criterios de salida |
|---|---|---|---|
| 0.2.0-alpha.3 | Actual (hecho) | P₂P + STUN + Android + CLI | README/CHANGELOG 2026-05-10 |
| 0.3.0-alpha.1 | **Modo servidor real + Flutter + Contratos** | Sala enrutada por relay (session_id compartido), app multiplataforma, protos gRPC | Ver hitos abajo |
| 0.3.0 | Seguridad de producción | Noise Protocol, anti-replay, rate limiting, relay con TLS, auditoría interna | Tests de seguridad en CI |
| 0.3.1 | Plano de control gRPC | tonic + ServerControl, clientes Dart/Go | Interop REST↔gRPC en CI |
| 0.4.0 | Distribución | publicar crates.io / PyPI / npm, SDK Swift, SDK Node, landing | Publicaciones verificadas |
| 1.0.0 | Estable | Congelar wire protocol v1.x, SemVer, auditoría externa, fuzz permanente | - |

## Hito 0.3.0-alpha.1 — en curso

### Modo servidor (sala) ✅ hecho
- [x] `Session::set_preset_session_id` — handshake con `session_id` compartido.
- [x] Relay enruta el handshake (el servidor se registra con Heartbeat).
- [x] Test e2e `relay_routed_session` (handshake + audio cifrado vía relay).
- [x] CLI `gs ptt --relay --room [--listen]` resuelve y pre-fija la sala.
- [x] CLI headless: `--no-audio --headless --duration` (contenedores/CI) con
      resumen de frames y métricas (rtt/jitter/loss/MOS).
- [x] FFI `gs_session_set_session_id` (ABI C v1, compatible hacia atrás).

### App Flutter multiplataforma ✅ hecho (scaffold completo)
- [x] `app/` (android, ios, linux, macos, windows, web).
- [x] Screens: inicio, configuración servidor (crear/unirse sala + QR),
      configuración P2P (host/join), sesión en vivo (PTT + métricas),
      ajustes (settings_ui) y acerca de.
- [x] Motor FFI real (`dart:ffi` → `libgravital_talk_ffi.so`) con fallback
      demo; handshakes en isolate.
- [x] **Audio real**: `AudioPump` (record PCM16 → FFI → flutter_sound),
      recv con timeout, VU por RMS, fallback senoidal sin hardware.
- [x] UI moderna: flutter_animate, animate_do, toastification,
      percent_indicator, mobile_scanner (QR), introduction_screen.
- [ ] Empaquetado nativo por plataforma (cargo-ndk → jniLibs, podspec iOS).

### Contratos (gRPC candidato) ✅ hecho
- [x] `proto/gravital/v1/server_control.proto`, `pairing.proto`.
- [x] Evaluación `docs/grpc-evaluation.md` (control sí, media no).

### Docker ✅ hecho
- [x] `docker/Dockerfile` (relay + cli), `Dockerfile.dev` (tests/bench),
      `Dockerfile.fuzz` (cargo-fuzz nightly).
- [x] `compose.yaml` perfiles: e2e, test, bench, perf (netem), security, obs.
- [x] `make docker-*` targets + scripts de salas compartidas.

## Hito 0.3.0 — Seguridad de producción

- [ ] **Noise Protocol** (NK/XX) sustituyendo el handshake custom
      (mantener transcript binding y auth tags; no romper wire v1).
- [x] **Anti-replay**: ventana autenticada por sesión (`transport::replay`,
      RFC 6479-style) + contador `replayed_dropped` + test de reinyección.
- [x] **Rate limiting** en relay (fixed-window por IP, UDP+WS,
      `--rate-limit`/`GS_RATE_LIMIT`, métrica `dropped{rate_limited}`).
- [ ] **Auth en relay**: token de sala opcional, metadatos en `Room`.
- [ ] TLS/WSS para el bridge WebSocket; HTTPS para observabilidad.
- [x] Pruebas de seguridad base: `make docker-fuzz`, estrés, replay test.
      (pendiente: fuzz en CI y anti-replay Noise-native).

## Hito 0.3.1 — Plano de control gRPC

- [x] `tonic` en relay con feature `grpc` (`ServerControl` + `PairingService`).
- [x] `tonic-build` + `protoc-bin-vendored` en `build.rs` (sin protoc del sistema).
- [x] `WatchRoom` streaming (eventos de sala vía broadcast del `Router`).
- [x] Test de integración `grpc_control` con cliente tonic real.
- [x] Cliente Dart (`grpc-dart` 5.x) generado en `app/lib/generated` y
      `FallbackRoomApi` (gRPC → REST) con tests del fallback.
- [ ] CLI con feature `grpc` + `grpc-web`/connect para Flutter Web.

## Hito 0.4.0 — Distribución

- [x] Orden de publicación definido (`scripts/publish-order.sh`);
      `cargo publish --dry-run` OK para `gravital-talk-core` e
      `gravital-talk-io` (los únicos sin deps internas).
- [ ] Publicar `gravital-talk*` en crates.io (requiere `cargo login` y
      publicar en orden: core → io → metrics/codec/transport → facade → ffi/cli/relay).
- [ ] PyPI (`maturin`), npm (`wasm-pack`).
- [ ] SDK Swift (Kotlin Multiplatform o FFI iOS) y Node.js (N-API).
- [ ] Landing `gravitaltalk.dev` + docs hosted.

## Hito 1.0.0 — Estable

- [ ] Congelar wire protocol v1; `PROTOCOL_VERSION_MAX` bloqueado.
- [ ] Auditoría de seguridad externa.
- [ ] Fuzzing + benchmarks como gates de CI permanentes.
- [ ] Baseline de rendimiento publicado (criterion + docker bench).

## Deuda técnica conocida (fuera de hitos)

- `outputs/windows` y `outputs/macos` sin artefactos (CI pendiente).
- iOS sin SDK/App (roadmap 0.4).
- Docs mencionan `io-uring`/`tokio-uring` sin implementación (a futuro).
- SIMD CRC (`simd-crc`) existe pero no es el camino por default.
- Fuzzing: workflow `.github/workflows/fuzz.yml` (semanal/manual) listo;
  falta integrarlo como gate de PR.
- Benchmarks aún no comparan contra baseline en CI (solo reportan).