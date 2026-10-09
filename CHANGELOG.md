# Changelog

Todos los cambios notables de Gravital Talk se documentan aquí. El formato sigue [Keep a Changelog](https://keepachangelog.com/es-ES/1.1.0/) y el proyecto usa [SemVer](https://semver.org/lang/es/).

## [Unreleased]

### Added

**Contrato de emparejamiento con código TOTP (`gravital_talk_transport::pairing`)**
- `PairingOffer`: secreto de sala, caducidad opcional y usos máximos opcionales
  (0 = ilimitado, para salas con varios invitados).
- `PairingReject` con cuatro motivos distintos, cada uno con mensaje accionable:
  expirado, código incorrecto, agotado y código caducado. La UI puede explicar
  qué pasó en lugar de "no se pudo conectar".
- Código de 6 dígitos derivado del secreto de la sala, rotando cada 30 s, con
  ventana de deriva de ±1 paso.
- No es un segundo factor por sí solo: deriva del mismo secreto que autentica la
  sesión. Sí lo es en modo sala, donde el relay puede verificarlo contra Band-All.
- El truncado dinámico (RFC 4226) se verifica contra los vectores canónicos del
  RFC 6238, reproducidos con una implementación independiente antes de escribirlos.

### Fixed

**La app Flutter no compilaba para Android, iOS ni escritorio**
- `main.dart` importaba `grpc_room_api.dart` incondicionalamente, y ese fichero importaba
  `grpc_web.dart`, que tira de `dart:js_interop` (sólo existe en web). El fallo es al
  COMPILAR, con un error que apunta a la cadena de imports y no a la causa:
  `Dart library 'dart:js_interop' is not available on this platform`.
- Se separa en `grpc_web_io.dart` (canal gRPC-Web real, sólo web) y
  `grpc_web_stub.dart` (error descriptivo en el resto), unidos por import condicional.
- Consecuencia: la app llevaba tiempo siendo sólo construible para navegador. Cualquier
  build para móvil o escritorio fallaba, y el workflow `flutter-android.yml` de CI no
  podía pasar.

**`scripts/flutter-android-libs.sh` fallaba con cualquier perfil**
- Pasaba `--$PROFILE` a cargo, y cargo no acepta `--debug` (es el perfil por defecto):
  fallaba con `unexpected argument '--debug' found`. Ahora sólo se pasa `--release`.

### Added

**`scripts/build-apk.sh` y la regla del APK en `outputs/`**
- Compila las libs nativas para las 3 ABIs, construye el APK y lo deja en
  `outputs/android/<perfil>/` con su `LATEST.txt`. Es la regla de desarrollo: el repo
  mantiene siempre la última versión instalable, para probar cada cambio sin compilar.
- Detecta el NDK por versión en vez de asumir un nombre fijo.
- APK release sin firmar (se firma al publicar, no en desarrollo): 73 MB con AOT y
  tree-shaking de iconos, frente a los 184 MB del debug.
- Regla documentada en `AGENTS.md`, incluido el error de `dart:js_interop` para no tener
  que volver a diagnosticarlo.

### Changed

- `app/android/.gitignore`: los `.so` generados ya no son candidatos a commit; viajan
  dentro del APK.


**Harness de auditoría end-to-end (`gs-audit`)**
- Nuevo crate `crates/gravital-talk-audit` (binario `gs-audit`, nunca publicado) que verifica que el audio **realmente** cruza el protocolo, no sólo que llegan datagramas.
- **Planes de turno** (`--turn-plan`, `--split-turns`): alternan quién habla con ventanas disjuntas, así la bidireccionalidad y el floor control se vuelven verificables. `gs ptt --headless` mantiene el PTT activo toda la ejecución, con lo que ambos lados hablan a la vez y el arbitraje nunca se ejercita.
- **Análisis del contenido del audio** (`analyze.rs`): frecuencia por conteo de cruces de cero, RMS y saturación sobre el PCM recibido. Los bytes corruptos no conservan ninguna de las tres.
- **Criterios dentro del proceso**: `--expect-rx-frames`, `--expect-tx-frames`, `--expect-peer-ptt`, `--verify-audio`. Devuelve `exit != 0` si fallan, en vez de dejar que un shell haga `sed` sobre el stdout.
- **N sesiones simultáneas** (`--sessions N`): cada sesión usa una frecuencia distinta (base + 60 Hz por sesión), de modo que un cruce de tráfico se detecta porque un par mide la frecuencia de otro.
- **Informe JSON** sin dependencias de serialización (`--json`): permite a CI distinguir "pasó" de "no se ejecutó".
- `--dump-rx-wav` escribe el PCM recibido para un segundo juicio con `scripts/verify_wav.py`.
- Sin `libopus` ni ALSA: el códec por defecto es PCM y la fuente es una senoidal interna, así que la auditoría corre en cualquier máquina y en el job `test-no-default-features` de CI. Con `--features opus` valida también Opus.

**Verificado en ambos códecs**
- PCM (frame de 10 ms): host y join miden 439.7 Hz con RMS 0.345 y MOS 4.41 en las dos direcciones.
- Opus (frame de 20 ms): host y join miden 441.1 / 440.9 Hz sobre audio comprimido, con el tamaño de frame que documenta el README.
- 3 sesiones P2P simultáneas con frecuencias distintas: cada par midió su propio tono, sin cruce de tráfico.

**Integración**
- `crates/gravital-talk-audit/tests/p2p_bidirectional.rs`: 4 tests de integración que prueban audio en ambas direcciones, corrección del tono recibido, ausencia de cross-talk entre sesiones paralelas y respeto del plan de turnos.
- `crates/gravital-talk-audit/tests/opus_bidirectional.rs`: lo mismo con Opus y frame de 20 ms (feature `opus`).
- Perfil `docker compose --profile audit` + `docker/scripts/audit.sh`: escenarios `p2p`, `parallel` y `room`, con informe por escenario.
  - `audit` (p2p + parallel, sin relay), `audit-room-host` (ancla que crea la sala) y `audit-room` (audit como JOIN de esa sala). El audit entra como JOIN porque el host de la sala es el ancla; el códec lo negocia el primer cliente, así que `ROOM_CODEC` debe coincidir con el del ancla.
- `make docker-audit` cubre `p2p` + `parallel`; `docker compose --profile audit run --rm audit-room` añade la sala vía relay.

### Fixed

**Handshake en modo sala: el host esperaba a cualquier origen**
- El harness usaba `handshake_open()` para el host también en modo sala, pero ahí todo el tráfico llega reenviado por el relay. Se añadió `PeerOptions::expect_known_peer`, que usa `handshake(Server, relay_addr)` en modo sala, igual que `gs ptt --listen --relay`.
- Sin esto el escenario de sala se quedaba colgado esperando un handshake que el relay nunca presentaba como procedente del cliente.
- `docker/Dockerfile`: `gs-audit` en la imagen `cli`.
- Jobs `audit` (PCM) y `audit-opus` en `.github/workflows/ci.yml`, cada uno subiendo su informe JSON.
- `make audit`, `make audit-parallel`, `make docker-audit`.

**Carencias de la verificación anterior, que el harness corrige**
- `docker/scripts/room-host.sh` hacía `exec gs ptt` sin parsear su salida, así que la dirección *join -> host* nunca se verificaba. Ahora se exige audio recibido en ambos sentidos.
- `gs ptt --headless` mantiene el PTT activo durante toda la ejecución, con lo que ambos lados hablan a la vez y el floor control nunca se ejercita. Los planes de turno del harness cubren esa carencia.
- Ningún test comprobaba el contenido del audio: un conteo de frames no distingue audio válido de bytes corruptos. `analyze.rs` mide frecuencia, RMS y saturación.

### Fixed

**Handshake de cliente roto sin la feature `noise`**
- `Session::run_client_handshake` en modo `Auto` llamaba siempre a `handshake_noise_client`. Sin la feature `noise` ese stub devuelve `Handshake("compiled without noise feature")`, no `Timeout`, y el fallback a v1 legacy sólo cubría `Timeout`: el error se propagaba y **ningún** build sin `noise` podía completar un handshake de cliente.
- El servidor ya era correcto en ambos caminos (`allow_noise = cfg!(feature = "noise")`); el cliente ahora es simétrico: sin `noise` va directo a v1 legacy. Con token de sala sigue fallando cerrado (el token exige Noise y no puede degradarse).
- Detectado por el job `test-no-default` de CI, que fallaba en `replay_session::replayed_datagram_is_dropped`.

**Limitación documentada: un frame PCM de 20 ms no cabe en el protocolo**
- El core impone `MAX_PAYLOAD_SIZE = DEFAULT_MTU - HEADER_SIZE = 1176` bytes de payload y `PacketBuilder::encode` rechaza con `PayloadTooLarge` cualquier cosa mayor.
- Un frame PCM16 mono de 20 ms a 48 kHz son **1920 bytes** de payload, más 4 de `audio_seq` y 16 de tag AEAD. Por lo tanto **no se puede enviar**: el error real es `protocol error: payload exceeds maximum size`, o `output buffer too small: need 1920, have 1500` si se sube el MTU sin tocar el tope.
- El máximo que sí cabe es **12 ms** (1156 bytes útiles). `CodecSession` con PCM debe configurarse a 10 ms o menos; el harness de auditoría usa 10 ms por defecto (también es un tamaño de frame válido para Opus).
- **No es un bug introducido aquí**: `send_audio` nunca llama a `FragmentReassembler`, que ya existe y está probado en `gravital-talk-core` (`MAX_FRAGMENTS = 16`). Conectar la fragmentación en el camino de envío permitiría PCM de 20 ms dentro del MTU por defecto, que es el arreglo natural. Queda como trabajo pendiente.
- El ejemplo del README que hace `send_audio(&vec![0u8; 1920])` falla por esta razón: documenta un tamaño de frame que el protocolo no admite.
- Fijado con el test `pcm_twenty_ms_frame_does_not_fit_the_wire_limit`, que además calcula el máximo válido. Si alguien conecta la fragmentación, ese test empieza a fallar y avisa de que hay que actualizar el README y `docs/packet-format.md`.

**Imports muertos con `--no-default-features`**
- `NoiseHello1`/`NoiseHello2` en `crates/gravital-talk-transport/src/session.rs` ahora se importan con `#[cfg(feature = "noise")]`. `cargo clippy --no-default-features` fallaba con `-D warnings`; CI no lo detectaba porque sólo aplica `--no-default-features` al job de `test`, no al de `clippy`.

## [0.3.0-alpha.1] — 2026-10-08

Modo servidor (sala) real + Rust 1.99 + app Flutter + contratos + Docker.

### Added

**Anti-replay (seguridad de paquetes)**
- `crates/gravital-talk-transport/src/replay.rs`: ventana deslizante RFC 6479-style (4096 secuencias, wrap-around u32, primer paquete arbitrario).
- `Session::dispatch_packet` descarta duplicados y paquetes fuera de ventana una vez establecidas las claves AEAD; contador `replayed_dropped` en métricas.
- Tests: 7 unitarios de la ventana + integración `replay_session` (reinyección byte a byte del datagrama → descartada sin error AEAD).

**Rate limiting del relay (anti-DoS)**
- `crates/gravital-talk-relay/src/rate_limit.rs`: limitador fixed-window (1 s) por IP, con poda por umbral.
- Aplicado en los loops UDP y WebSocket; métrica `dropped{reason="rate_limited"}`.
- Config: `rate_limit_per_sec` (TOML) y `--rate-limit`/`GS_RATE_LIMIT` (CLI/env). `0` = ilimitado (default).

**Plano de control gRPC (feature `grpc`)**
- `crates/gravital-talk-relay/src/grpc.rs`: `ServerControl` (Info, Health, CreateRoom, GetRoom, ListRooms, DeleteRoom, WatchRoom por stream) + `PairingService`.
- Eventos de sala en `Router` (PeerJoined/Left, FloorGranted/Released) vía broadcast; `WatchRoom` filtrado por sesión.
- `build.rs` con `tonic-build` + `protoc-bin-vendored` (sin protoc del sistema); server en `--grpc-bind`/50051.
- Test `grpc_control`: cliente tonic real contra el servidor en proceso.
- Docker: `--build-arg CARGO_FEATURES="--features grpc"`.

**FFI**
- `gs_session_recv_audio_timeout()`: recepción con timeout (no bloqueante con 0 ms) para loops de audio parables.
- `gs_session_set_room_token()` (PSK de Noise) y `gs_session_set_handshake_mode()`.

**Noise, auth de sala y TLS**
- Handshake **Noise** (`NN`/`NNpsk0` con `snow`) como modo por defecto (`Auto`) con fallback a legacy v1; los tests de sesión cubren token correcto, token incorrecto sin downgrade y degradación a legacy.
- Token de sala: PSK de Noise + exigido por el relay al resolver el código (REST `?token=`, gRPC `token`); solo se almacena SHA-256 en el relay.
- **TLS/WSS** (feature `tls`): `--tls-cert/--tls-key` → WSS en el puerto WS y gRPC sobre TLS (verified con openssl TLSv1.3).
- **gRPC-Web** (feature `web`): `tonic-web` + CORS en el mismo puerto gRPC; Flutter Web usa `GrpcWebClientChannel` con fallback REST.
- Empaquetado nativo Flutter: scripts cargo-ndk (Android) / staticlib (iOS), task Gradle automática con NDK y workflow `flutter-android.yml`.

**App Flutter: audio real y UI moderna**
- `AudioPump`: captura PCM16 con `record`, playback con `flutter_sound` (PCM16 stream), recv con timeout en isolate, VU meter por RMS real, fallback senoidal sin hardware.
- Componentes modernos (shortlist FlutterGems/FlutterLibrary): `flutter_animate`, `animate_do` (pulso PTT), `toastification`, `settings_ui` (pantalla de ajustes), `percent_indicator` (level meter), `mobile_scanner` (QR de sala), `introduction_screen` (onboarding primer arranque), `url_launcher` (enlaces).
- Cliente gRPC del plano de control: código generado en `app/lib/generated` (protoc_plugin 25 + protobuf 6) y `FallbackRoomApi` (gRPC → REST) con tests.

**CI y distribución**
- Workflow `fuzz.yml`: smoke de cargo-fuzz en contenedor (manual + semanal) con subida de artefactos de crash.
- `scripts/publish-order.sh`: orden de publicación crates.io; `cargo publish --dry-run` verificado para `gravital-talk-core` y `gravital-talk-io`.

**Toolchain**
- Versión del workspace alineada a `0.3.0-alpha.1` (Cargo.toml, SDKs, Helm); MSRV `1.99` + `rust-toolchain.toml`; Dockerfile `rust:1.99-slim`.

**Modo servidor/sala — handshake enrutado por relay**
- `Session::set_preset_session_id(id)`: permite fijar el `session_id` antes del handshake. Sin esto, `ClientHello` viaja con id 0 y el relay lo descartaba, haciendo imposible el modo sala (terminal central).
- El servidor se registra ante el relay con un `Heartbeat` al empezar el handshake; el relay enruta los 4 mensajes del handshake sin descifrar nada.
- El cliente valida que el `ServerHello` respeta el id pre-fijado.
- FFI: `gs_session_set_session_id(handle, session_id)` (ABI C v1, aditiva).
- CLI: `gs ptt --relay H --room C` resuelve la sala y pre-fija el id; `--listen` actúa como servidor de la sala.
- Test e2e nuevo `crates/gravital-talk-relay/tests/relay_routed_session.rs`: handshake 4-way + frame de audio cifrado a través del relay.
- CLI headless para CI/contenedores: `gs ptt --no-audio --headless --duration N` (fuente senoidal interna, resume frames y RTT/jitter/loss/MOS).

**Toolchain Rust 1.99**
- MSRV `rust-version = "1.99"` en todo el workspace y SDKs; `rust-toolchain.toml` pin a 1.99.0; Dockerfile del relay actualizado (rust:1.99-slim).
- `async-trait` 0.1.89 → 0.1.92 (elimina lints redundantes del macro).

**App Flutter multiplataforma (`app/`)**
- Proyecto Flutter 3.47 (android, ios, linux, macos, windows, web).
- Pantallas: Home (hero + modo servidor/P2P + diagnóstico + eventos), configuración de servidor (crear sala con QR / unirse), configuración P2P (host/join), sesión en vivo (PTT, VU meter, métricas de red), ajustes (audio/códec/tema/diagnóstico) y Acerca de.
- Motor doble: `dart:ffi` contra `libgravital_talk_ffi` (handshakes bloqueantes en isolate) con fallback demo para desarrollo de UI sin librería nativa.
- Servicios: `RoomApi` (REST del relay), `SettingsStore` (shared_preferences), `EventLog`, controlador de sesión con ticker de métricas.
- Ajustes persistidos: sample rate, canales, frame, jitter buffer, MTU, bitrate, codec, tema.

**Contratos del plano de control (proto)**
- `proto/gravital/v1/server_control.proto`: ServerInfo, Health, CreateRoom, GetRoom, ListRooms, DeleteRoom, WatchRoom (stream).
- `proto/gravital/v1/pairing.proto`: ofertas de emparejamiento P2P.
- `docs/grpc-evaluation.md`: análisis (control sí, media no) y plan de adopción con tonic.

**Docker**
- `docker/Dockerfile` multi-target (relay/cli), `Dockerfile.dev` (tests/bench), `Dockerfile.fuzz` (nightly + cargo-fuzz).
- `compose.yaml` con perfiles: `e2e` (host+join headless con verificación de frames), `test`, `bench`, `perf` (tc netem: delay/jitter/pérdida), `security` (fuzz acotado), `observability` (Prometheus+Grafana).
- `make docker-e2e/test/bench/perf/fuzz/obs` + scripts de sala compartida por volumen.
- `.dockerignore` para contextos de build mínimos.

### Changed

- `README.md`: tabla de estado (modo sala, Flutter, contratos, Docker), quickstart Docker, hoja de ruta 0.3.0-alpha.1.
- `Makefile`: nuevos targets `docker-*`.
- Calidad: clippy/fmt limpios en todo el workspace (incluido gate `perf`+`nursery`); 25 suites de tests en verde.

### Fixed

- `gravital-talk-core`: `FragmentReassembler` reexportado solo con la feature `alloc` (compila `--no-default-features` y `make cross-wasm`).
- Tests de `floor.rs` y `prop_decode.rs` sin dependencias de `alloc` forzadas.
- `session.rs`: mutex no retenidos a través de `await` en PTT/close/FEC.
- Benches `opus_encode`: overflow de `i16` en debug.

## [0.2.0-alpha.3] — 2026-05-10

Emparejamiento P2P por QR + STUN + Android completo + CI auto-build de binarios.

### Added

**STUN — Descubrimiento de IP pública sin infraestructura propia**
- `crates/gravital-talk-transport/src/stun.rs` (nuevo): cliente STUN RFC 5389 puro, sin dependencias externas.
- `discover_public_addr(local_port: u16) -> Result<SocketAddr, StunError>`: crea socket UDP efímero, envía Binding Request a `stun.l.google.com:19302`, fallback a `stun1.l.google.com:19302`, timeout 5 s por servidor.
- Parsea atributo `XOR-MAPPED-ADDRESS` (type `0x0020`) con XOR cookie `0x2112A442`; fallback a `MAPPED-ADDRESS` para servidores legacy.
- 3 tests unitarios: parse roundtrip, tx_id incorrecto rechazado, paquete corto rechazado.
- Exportado desde `gravital-talk-transport` y re-exportado en `gravital-talk`.

**Handshake abierto — Servidor sin IP del cliente**
- `Session::handshake_open()`: modo servidor que acepta el primer `ClientHello` válido de cualquier dirección IP.
- Problema resuelto: el `handshake_server(peer)` original filtraba con `if from != peer`, haciendo imposible el pairing cuando no se conoce la IP del cliente de antemano.
- Tras recibir el primer ClientHello válido: fija `self.peer = Some(from)` y procede con el handshake normal.
- `Session::local_addr()`: expone el puerto UDP local para incluirlo en el QR.
- Compatibilidad: `handshake_server()` existente no se modifica.

**FFI C API — Nuevas funciones de pairing**
- `gs_session_local_port(handle, out_port)`: retorna el puerto UDP local de la sesión.
- `gs_discover_public_addr(local_port, out_buf, buf_len)`: descubre IP pública vía STUN, escribe `"ip:port"` como C-string.
- `gs_session_accept_any(handle)`: ejecuta `handshake_open()` bloqueante.

**JNI Bridge Android — Pairing nativo**
- `nativeGetLocalPort(handle: Long): Int`
- `nativeDiscoverPublicAddr(bindPort: Int): String?`
- `nativeAcceptAny(handle: Long): Int`

**Android PairingActivity — UI completa sin relay obligatorio**
- `PairingActivity.kt` (nueva): Activity con ViewFlipper de 3 pantallas (Home, Host QR, Join).
- `PairingViewModel.kt` (nuevo): lógica de estado con `StateFlow<PairingUiState>`.
- **Flujo HOST**: Crear sesión → obtener IP LAN (ConnectivityManager) + IP pública (STUN paralelo) → generar QR con ZXing → mostrar código texto `GRVT-XXXX` → `nativeAcceptAny` bloquea hasta cliente → lanzar PTT screen.
- **Flujo CLIENT (QR)**: Escanear QR con CameraX + ML Kit → parsear URI → intentar LAN (2 s) → Internet P2P (5 s) → relay fallback (10 s).
- **Flujo CLIENT (manual)**: Ingresar `host:port` del relay manualmente.
- **URI canónica**: `gravital-talk://pair?v=1&lan=<ip>:<port>&pub=<ip>:<port>&relay=<host:port>`
- Deep link `gravital-talk://pair` registrado en AndroidManifest.
- `activity_pairing.xml`: layouts completos con ImageView para QR, PreviewView para cámara, TabLayout (Escanear | Ingresar).
- Permiso `CAMERA` y feature `android.hardware.camera` (no obligatoria) en manifest.

**Android PttViewModel — Integración con handle existente**
- `attachExistingHandle(handle: Long)`: toma ownership de un handle creado por PairingViewModel sin crear nueva sesión.

**Android MainActivity — Colgar y navegar**
- `btnHangUp`: botón visible durante llamada; al pulsar → `disconnect()` → vuelve a PairingActivity.
- Si campos de relay vacíos al inicio → redirige a PairingActivity automáticamente.
- `onNewIntent()`: recibe handle nativo de PairingActivity via `EXTRA_NATIVE_HANDLE`.

**Dependencias Android añadidas**
- `com.google.zxing:core:3.5.3` — generación de QR.
- `androidx.camera:camera-camera2/lifecycle/view:1.4.1` — preview de cámara.
- `com.google.mlkit:barcode-scanning:17.3.0` — decodificación de QR.

**CI auto-build de binarios — `build-outputs.yml`**
- Nuevo workflow que construye en cada push que toca código compilable.
- 4 jobs paralelos: `android-apk` (Ubuntu), `linux-cli` (Ubuntu), `macos-cli` (macOS, matrix x86_64+aarch64), `windows-cli` (Windows).
- Cada job versiona el artefacto como `<nombre>-v<semver>-<short_sha>-<plataforma>`, lo copia a `outputs/<plataforma>/`, actualiza `LATEST.txt` y hace commit+push con `[skip ci]`.
- Retry/rebase automático para evitar conflictos cuando varios jobs pushean simultáneamente.
- El commit de CI incluye `[skip ci]` → previene loops infinitos; todos los jobs comprueban que el mensaje del commit anterior no contenga `[skip ci]`.
- También sube cada artefacto como GitHub Actions artifact (fallback 30 días).

**Directorio `outputs/` — Binarios pre-compilados en repo**
- `outputs/android/debug/` — APKs de debug versionados.
- `outputs/linux/x86_64/` — binario `gs` para Linux 64-bit.
- `outputs/macos/x86_64/` y `outputs/macos/aarch64/` — binario `gs` para Mac Intel y Apple Silicon.
- `outputs/windows/x86_64/` — ejecutable `gs.exe` para Windows.
- `outputs/README.md` — documentación de estructura, instalación y scripting con LATEST.txt.

### Changed

- `android.yml`: añadido `if: "!contains(github.event.head_commit.message, '[skip ci]')"` en ambos jobs para no duplicar trabajo con `build-outputs.yml`.
- `docs/overview.md`: roadmap actualizado; STUN ya implementado (no es trabajo futuro).
- `docs/session-model.md`: sección 9 sobre `handshake_open()`.

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
- **Fase 8.** Publicación a crates.io, PyPI, npm, Maven Central, SPM; landing page en `gravitaltalk.dev`.
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

[0.3.0-alpha.1]: https://github.com/angelnereira/gravital-talk/releases/tag/v0.3.0-alpha.1
[0.1.0-alpha.1]: https://github.com/angelnereira/gravital-talk/releases/tag/v0.1.0-alpha.1
