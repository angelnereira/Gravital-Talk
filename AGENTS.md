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

## App de escritorio Linux y audio nativo

```bash
make app-linux       # FFI con `--features audio` + flutter build linux --release
app/build/linux/x64/release/bundle/gravital_talk_app [--host] [--auto-talk=N] [gravital-talk://pair?…]
```

- En escritorio el audio lo lleva la librería nativa (`gs_audio_*`, cpal). En
  Android/iOS la feature `audio` va apagada, `gs_audio_start` devuelve
  `GS_ERR_UNSUPPORTED` y el audio lo ponen los plugins de Flutter (`AudioPump`).
- El bundle copia `target/release/libgravital_talk_ffi.so` **tal como esté**.
  `make ffi-smoke`, `make build` y el build de CI la recompilan **sin** `audio`:
  si después empaquetas la app sin volver a `make app-linux`, sale sin micrófono
  ni altavoz. `gs_audio_supported()` lo dice.
- `--host` abre directamente una sala y `--auto-talk=N` habla/escucha en turnos
  de N s. Existen porque en GNOME/Wayland no se pueden inyectar clics (xdotool
  no llega a la ventana, ni bajo XWayland); las capturas sí funcionan con
  `GDK_BACKEND=x11` + `import -window`, y `zbarimg` lee el QR de la captura.
- `GS_AUDIO_TEST_TONE=1000` sustituye el micrófono por un tono; con
  `gs peer --host IP --port P --token T` se mide lo que llega.
- `cargo run -p gravital-talk-io --example voice_probe` comprueba que el
  micrófono y el altavoz abren, sin protocolo de por medio.

## Códec de voz

- Las apps usan `gravital_talk_codec::voice` (feature `voice`): Opus y RNNoise
  en Rust puro, integrados en la FFI (`crates/gravital-talk-ffi/src/voice.rs`).
  El embebedor sigue viendo PCM en `gs_session_send_audio`/`recv_audio`.
- `OpusCodec` (feature `opus`, libopus en C vía `audiopus`) sigue existiendo
  para el CLI y `CodecSession`, pero **no compila para Android** con cargo-ndk
  ("Failed to autogen Opus"). No intentes meterlo en la FFI.
- Un `recv` puede devolver hasta 4 frames de golpe cuando reconstruye audio
  perdido: el buffer de quien llama tiene que dar para eso (7680 B a 48 kHz,
  20 ms).
- Los tests del códec se corren aparte, porque la feature no está en los
  defaults: `cargo test -p gravital-talk-codec --no-default-features --features voice`.
- Con tonos puros hay que apagar la reducción de ruido (`GS_AUDIO_TEST_TONE` y
  los tests lo hacen): RNNoise trata un tono como ruido y lo elimina.
- La voz se trata en Rust (`voice_dsp.rs`: pasa-altos, nivelador, limitador, y realce
  de presencia al recibir) y se elige por **perfil** (`gs_session_set_voice_profile`,
  `GS_VOICE_*`). La app ya no pide reducción de ruido ni ganancia automática al sistema:
  sería tratar la voz dos veces. Para tonos y medidas, perfil `GS_VOICE_RAW`: el
  nivelador también cambia el volumen de un tono.
- Para juzgar un cambio en la voz hay que **oírlo**: `voice_lab` (ejemplo del códec) deja
  un WAV por perfil a partir de una grabación. Las cifras de datos y lo que se probó y
  descartó están en `docs/codecs.md`.
- Sin llamar a `gs_session_set_voice_profile` el códec se comporta como antes de los
  perfiles (`VoiceTuning::LEGACY`): es lo que ven el CLI y cualquier embebedor antiguo.

## Conexión entre redes (perforación de NAT)

```bash
make nat-sim         # NAT real del kernel con namespaces; pide sudo. Incluye control negativo
cargo test -p gravital-talk-transport --test nat_punch   # red simulada, sin privilegios
```

- El servidor de encuentro es el propio `gs-relay` (puerto UDP). No lleva audio:
  sólo presenta direcciones. Protocolo y límites en `docs/pairing.md`.
- En la app sólo lo configura el anfitrión (Ajustes); viaja en la invitación
  (`rv=`). Sin él, las salas funcionan sólo dentro de la misma red.
- STUN para la invitación se hace **desde el socket de la sesión**
  (`Session::discover_public_addr`) y **antes** del handshake: es el único
  lector del socket en ese momento. `discover_public_addr(0)` usa otro socket y
  su puerto no es el de la sala.
- Si cambias cómo se sondan o eligen los caminos, corre las dos pruebas: la red
  simulada no reproduce el estado de conntrack, y el NAT del kernel sí.
- Mientras un extremo espera una respuesta del handshake le siguen llegando sondas del
  otro. No pueden contar como intentos ni provocar reenvíos (`Session::recv_reply`): así
  se agotaban los reintentos en milisegundos y la conexión entre redes fallaba una de cada
  tres veces. El test es `las_sondas_ajenas_no_agotan_el_handshake_del_cliente`.
- La app ya no enseña el «código de respuesta»: las direcciones del invitado le llegan
  solas al anfitrión (red local y buzón, selladas con el token de la sala). Los enlaces
  `answer` antiguos se siguen aceptando, pero ninguna pantalla los pide.
- Sin servidor, lo que conecta dos redes distintas es el intercambio
  invitación ↔ respuesta (`PairingAnswer`, `Session::add_path_hints`) más el
  barrido de puertos. Para reproducirlo con el CLI: `gs peer --bind 0.0.0.0
  --hint IP:PUERTO` en un lado y `gs peer --host IP --port PUERTO` en el otro.
- Para clasificar un NAT: STUN desde el **mismo puerto local** a varios
  servidores de IP distinta. Mismo puerto público en todos = cono; distinto =
  simétrico. En el teléfono, con `toybox nc -u -p PUERTO` por adb.
- `recv_audio` es el dueño del socket y quien mantiene el camino (latidos,
  sondas de recuperación, plazo de pérdida). Lo que tenga que sobrevivir entre
  llamadas vive en `Session`, no en variables locales: quien lo llama suele
  usar un timeout corto y cancela el futuro.

## REGLA: cada cambio deja un APK instalable en `outputs/` y se publica a mano

Obligatorio después de tocar Dart o el FFI. El repo debe tener siempre la última versión
instalable, para poder probar cualquier cambio sin compilar nada.

```bash
./scripts/build-apk.sh           # release unsigned (por defecto)
./scripts/build-apk.sh debug     # con símbolos
```

El script compila las libs nativas para las 3 ABIs, construye el APK y lo deja en
`outputs/android/<perfil>/` con su `LATEST.txt`. Para probarlo en local:

```bash
LATEST=$(cat outputs/android/release/LATEST.txt)
adb install -r "outputs/android/release/$LATEST"
adb shell am start -n dev.gravitaltalk.gravital_talk_app/.MainActivity
```

**Las builds no se versionan en git** (`outputs/` está en el `.gitignore`), así que el APK
de un commit concreto sólo existe en la máquina que lo construyó. Para que se puedan
descargar, los instalables se publican **a mano** en el release `builds`:

```bash
scripts/publish-builds.sh --build    # compila Android y Linux y los sube
scripts/publish-builds.sh            # sube lo último ya compilado
make publish-builds                  # lo mismo que --build
```

Es el flujo de publicación del proyecto, decidido así: no depende del CI. El script
comprueba que el APK es del commit actual y que el bundle de Linux lleva audio nativo,
pasa el APK publicado a `previous`, sube los nuevos y verifica el hash de lo publicado.

| Fichero | Para qué |
|---|---|
| `gravital-talk-latest.apk` | Android, el último build. Este es el que hay que instalar. |
| `gravital-talk-previous.apk` | Android, el anterior, para volver atrás si algo va mal. |
| `gravital-talk-latest-linux-x64.tar.gz` | Linux x64: descomprimir y ejecutar `gravital-talk/gravital_talk_app`. |

```
https://github.com/angelnereira/Gravital-Talk/releases/download/builds/gravital-talk-latest.apk
```

Después de publicar, instala el APK en el dispositivo de pruebas: un teléfono con un build
viejo y otro extremo con el nuevo pueden no entenderse (cambio de códec), y el síntoma es
"conecta pero no se oye".

`build-apk.sh` puede fallar en el paso de Flutter **después** de compilar las libs nativas.
Comprueba siempre que `outputs/android/release/LATEST.txt` nombra el commit actual; mirar
sólo el final de la salida no basta.

Requisitos que la máquina de desarrollo necesita una sola vez:

```bash
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
cargo install cargo-ndk --locked
```

Detalles que ahorran tiempo:

- El APK de **release** (~75 MB, AOT) es el útil para instalar. El **debug** pesa
  ~184 MB porque incluye `kernel_blob.bin` sin comprimir; sólo para depurar.
- El APK release queda **sin firmar**: se firma al publicar, no en desarrollo.
- `ANDROID_NDK_HOME` se detecta solo, pero sus nombres no son estables: el script coge el
  más reciente por versión.
- Un cambio sólo de Dart no necesita recompilar las libs nativas, pero el script lo hace
  igual para que el APK siempre sea coherente.
- Los `.so` generados (`app/android/app/src/main/jniLibs/`) **no** se commitean: son salida
  de build y ya viajan dentro del APK. El `.gitignore` de `app/android` los excluye.

### Error que ya ha mordido: `dart:js_interop`

`main.dart` no puede importar nada que tire de `grpc_web.dart`: ese paquete usa
`dart:js_interop`, que no existe fuera del navegador. El fallo es al **compilar** para
Android/iOS/escritorio y el error apunta a la cadena de imports, no a la causa. Los imports
de gRPC-Web viven en `grpc_web_io.dart` y llegan por import condicional desde
`grpc_web_stub.dart`. Si ves `dart:js_interop is not available on this platform`, es esto.

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

## Gravital Experience UI — Regla estricta de interfaces Flutter

`docs/gravital-experience-ui.md` es la **regla de diseño obligatoria** para construir y diseñar todas las interfaces Flutter del proyecto (presentes y futuras). Toda pantalla, diálogo, notificación in-app o flujo debe seguir sus tokens, componentes y patrones. Ningún widget usa colores, radios, espacios o duraciones literales; todo sale del sistema en `app/lib/ui/`. El checklist de revisión de cada pantalla está en la sección 13.2 de ese documento.

## Documentación

- `docs/protocol-spec.md`, `docs/packet-format.md`, `docs/session-model.md` — el formato de header es **24 bytes**;
  el header es AAD del AEAD y el nonce de 96 bits deriva de `sequence` + `session_id`. Cualquier cambio wire
  requiere actualizar spec + packet-format + CHANGELOG.
- `docs/security.md` (modelo de amenazas), `docs/pairing.md` (QR/STUN), `docs/grpc-evaluation.md` (plano de
  control: el audio **nunca** pasa por gRPC), `docs/benchmarks.md`, `docs/roadmap.md`, `docs/adr/`.
- Contratos gRPC en `proto/gravital/v1/`; el cliente Dart generado vive en `app/lib/generated/` (**generado, no
  editar**; instrucciones en `proto/README.md`).

## Cierres inesperados en dispositivos ajenos

La librería nativa se compila con `panic = "abort"`: un pánico cierra la app sin más.
Para no depender de `adb`, la app guarda la causa y la enseña en **Ajustes → Acerca de**
en el siguiente arranque (`app/lib/services/crash_report.dart`):

- pánico de Rust → `gs_crash_log_set_path` lo anota (lugar y mensaje) en `crash-native.txt`;
- excepción de la parte Android → `MainActivity` la anota en `crash-java.txt`;
- motivo del sistema (Android 11+) → `getHistoricalProcessExitReasons`.

Ante un "se cierra sola", pedir ese informe antes de suponer la causa. Descartado ya
con pruebas: el códec en ARM de 32 bits (pasa bajo `qemu-arm`), paquetes que no son
Opus y el consumo de pila del códec.

## Audio nativo en Android (experimental, apagado)

- Las libs de Android se compilan con `--features android-audio` (cpal sobre Oboe). El
  runtime de C++ va enlazado estático desde `build.rs`: sin eso `dlopen` falla con
  `cannot locate symbol "__cxa_pure_virtual"` y la app cae al motor demo **sin avisar**.
  Tras tocar dependencias nativas, comprueba en el teléfono que Ajustes → Acerca de no
  dice «Demo».
- `MainActivity` llama a `nativeInitAudio(context)`; sin esa llamada `gs_audio_supported`
  devuelve 0 en Android (cpal abortaría al preguntar por los dispositivos).
- La app sólo lo usa con `--dart-define=GT_EXPERIMENTAL=true` y el interruptor de Ajustes.
  Pendiente: validar que suena y medir la latencia frente a `AudioPump` antes de activarlo.

## Medir audio entrecortado y latencia

Método acústico, sin instrumentos: el portátil aloja con
`GS_AUDIO_TEST_TONE=700 … --host --auto-talk=4`, el teléfono se une, y se graba con
`arecord` lo que sale por su altavoz. El aviso de turno del portátil marca el inicio; el
tono de 700 Hz que sale del teléfono, la llegada. Con eso se cuentan huecos por turno y el
retardo. Referencia (2026-10-11, misma wifi, Pixel 8 Pro, camino `AudioPump`): 0–5 % de
huecos y 170–220 ms del aviso al sonido. El audio nativo de Android dio menos retardo
(100–170 ms) pero más cortes: por eso sigue apagado.

Dos retardos distintos en la misma grabación: del aviso de inicio al tono (primer frame,
incluye el arranque del turno) y del aviso de fin a que el tono calla (con la cola ya
formada: es el que se nota al conversar). Referencia (2026-10-12, portátil en casa →
Pixel 8 Pro por **LTE**, 20 turnos): inicio p50 265–285 ms, final p50 150–155 ms, 4 % de
huecos, y el teléfono tira un 5–7 % de los frames (los que llegan en ráfaga).

Compilando con `--dart-define=GT_AUDIO_STATS=true`, `AudioPump` escribe cada segundo en
`adb logcat -s flutter` cuántos frames llegaron, cuánto tardó el reproductor en aceptarlos
y cuántos tiró. `adb shell dumpsys media.audio_flinger` dice qué pista abrió Android: la
de `flutter_sound` tiene 60 ms de búfer, no va por la ruta rápida y declara ~175 ms de
latencia con el búfer lleno. Ese es el suelo de esta ruta.

Probado y descartado: guardar hasta 60 ms en Dart en vez de tirar las ráfagas (con y sin
vaciado progresivo). Pierde menos frames (2–4 %) pero el retardo al final del turno sube a
210–240 ms. Bajar de 150 ms pide la ruta nativa, no más ajustes en `AudioPump`.

Si el audio se corta, mira primero el tráfico (`tcpdump` por el puerto de la sesión): una
sala sana son ~50 paquetes por segundo y sentido mientras alguien habla, y unos pocos en
silencio.

## Iconos en blanco en Linux

`flutter build linux` recorta la fuente de iconos y en compilaciones incrementales puede
quedarse con el recorte anterior: los iconos nuevos salen como un rombo o en blanco.
`make app-linux` compila con `--no-tree-shake-icons`; si lanzas `flutter build linux` a
mano, pásalo también. Otra causa de «icono que no se ve»: mismo color que su fondo (pasó
en el onboarding con el primario sobre un vidrio con resplandor primario).

## Enlaces de invitación: dos formas

- La propia, `gravital-talk://…`, es la que llevan los QR y la que entiende el resto del
  código. La que se **comparte o copia** es `http://localhost:47801/gravital-talk/…`
  (`LaunchLinks.shareable`): las apps de mensajería no hacen pulsable un esquema propio.
- Todo lo que entra (intent, argumento, portapapeles, texto pegado o compartido) pasa por
  `LaunchLinks.extract`, que devuelve siempre la forma propia. No compares prefijos a mano.
- Dos caminos para que el enlace web llegue a la app, ninguno con una web de por medio:
  el filtro `http`/`localhost:47801` del manifiesto (abre la app aunque esté cerrada; la
  primera vez Android pregunta) y `LaunchLinks.serveLocal`, que contesta en ese puerto
  mientras la app vive (en Android con un 302 al esquema propio, que es lo que la trae al
  frente; en escritorio atiende el enlace él mismo).
- Probado en un Pixel con Android 17. La documentación de Android dice que desde la 12 un
  enlace web sin dominio verificado va al navegador; con `localhost` no ocurrió, pero no
  está comprobado en otros fabricantes. Para verlo: `cmd package query-activities` con el
  enlace, y `toybox nc 127.0.0.1 47801` contra la app abierta.
- iOS no tiene equivalente (exige un dominio verificado): allí se comparte el esquema.

## Buzón de señalización

`app/lib/services/mailbox.dart` lleva los datagramas sellados de `knock` (saludo, llamada,
respuesta) por MQTT, además de por la red local. Es lo que deja llamar a un contacto en
otra red sin códigos. Activo por defecto con `MqttMailbox.defaultServers`.

- Los servidores públicos son irregulares: el 2026-10-11 pasaron de no aceptar ninguno a
  entregar sólo hivemq. `cd app && dart run tool/mailbox_probe.dart` dice cuáles responden.
- `GT_NO_LAN=1` apaga la señalización por red local: es como se prueba que el buzón basta.
- La librería `mqtt_client` habla MQTT 3.1 salvo que se llame a `setProtocolV311()`; con
  3.1 los servidores no contestan y el error no lo dice.
- Lo que se publica antes de conectar se guarda 20 s (abrir la app desde un enlace y
  saludar en el acto era justo ese caso).
- El buzón sólo presenta: el audio sigue necesitando camino directo (o el relay).

## Tonos de turno

Se generan en Rust (`gravital_talk_transport::tone::turn_start_tone` / `turn_end_tone`) y
se reproducen **en local**: en escritorio mezclados en la cola del altavoz
(`PlayoutMixer`, que suma en vez de encolar), en móvil por `AudioPump.playTone`. No los
mandes por la sesión: el códec y RNNoise se los comen.

## Contactos y llamadas entre ellos

- El secreto de un contacto es el token de la sala en la que se conocieron. De él salen
  (en Rust, `gravital_talk_transport::knock`) el id público y la clave con la que se sellan
  las llamadas; Dart sólo guarda la lista (`app/lib/services/contacts.dart`) y mueve los
  datagramas por UDP 47800.
- No hay servidor ni push de terceros, a propósito: una llamada sólo llega si el otro tiene
  la app viva y es alcanzable. No prometas más que eso.
- Para probarlo sin clics: dos instancias con `XDG_DATA_HOME` distinto (cada una su
  identidad); una con `--host` y la otra con el enlace para emparejar, y después
  `--auto-accept` en una y `--call-first` en la otra.
- Los botones del tema ocupan todo el ancho (`Size.fromHeight`): dentro de un `Row` hay que
  darles `minimumSize: Size(0, …)` o aplastan al resto.

## Llamadas con la app oculta, timbre y vibración

- Una llamada sólo llega si el proceso está vivo: lo mantiene `SessionService` en modo
  espera («Recibir llamadas en segundo plano», activo por defecto). Sigue sin haber push de
  terceros: con la app cerrada desde recientes, o con el teléfono en reposo profundo
  (Doze), no llega nada. No prometas otra cosa.
- El tono y la vibración de llamada los pone `Ringer.kt`, no el canal de notificación: así
  son los mismos con la app a la vista y se paran al contestar. Respeta el modo del
  teléfono (silencio / sólo vibración) y se calla solo a los 30 s.
- El tono es `res/raw/gt_ring.wav`, generado por `scripts/make-ringtone.py`. Para cambiarlo
  se edita el script y se regenera; no se edita el WAV a mano.
- Un canal de notificaciones no cambia de sonido una vez creado en el teléfono: si hay que
  cambiar su configuración, se le da otro id (`calls.silent` sustituyó a `calls`).
- La notificación de llamada trae «Contestar» y «Ahora no»; llegan a Dart por
  `BackgroundSession.onCallAction` y cierran el mismo diálogo que se ve dentro de la app.
- Para probarlo sin tocar el teléfono: app abierta y `adb shell input keyevent KEYCODE_HOME`,
  una instancia de escritorio que ya lo tenga de contacto con `--call-first`, y mirar
  `dumpsys notification --noredact`, `dumpsys vibrator_manager` y `dumpsys audio`
  (un `MediaPlayer` con `USAGE_NOTIFICATION_RINGTONE`).
- Las vibraciones cortas (botón de hablar, código leído, avisos) son `Haptics`
  (`app/lib/services/haptics.dart`); no necesitan permiso.

## Burbuja para hablar (Android)

`PttBubble.kt` dibuja una ventana `TYPE_APPLICATION_OVERLAY` cuando la actividad está
parada (`onStop`) y Dart dice que hay sala (`session`) o llamada entrante (`ringing`), por
`BackgroundSession` (método `bubble`). Mantener pulsado 150 ms es hablar (`pttDown`/`pttUp`
hacia Dart); moverse antes es arrastrar. Tocarla con una llamada abre la actividad con
`EXTRA_ANSWER` + `EXTRA_FROM_BUBBLE`, y Dart (`app.dart`, `_returnWhenLive`) manda la app al
fondo en cuanto la sala conecta.

- Sin `SYSTEM_ALERT_WINDOW` concedido (`Settings.canDrawOverlays`) no aparece. Ese mismo
  permiso es el que deja abrir la actividad y llamar a `startForegroundService` desde
  segundo plano.
- Hablar desde la burbuja no debe pasar por `startForeground` otra vez: con la app oculta
  y fuera de las excepciones (tocar la notificación) Android puede negar el tipo micrófono
  (`enterForeground` cae entonces a sólo reproducción) y la sala seguiría, pero muda. Sin
  comprobar en un teléfono; se evita por diseño. Por eso los cambios de estado van por `SessionService.refresh`
  (`notify` con el mismo id), no rearrancando el servicio.
- Mantener 150 ms y subir el dedo 64 dp bloquea el turno (`locked`); con la sala en pausa,
  mantener 600 ms manda `resume`. La etiqueta con el nombre (`NameTag`) sale con
  `peerName`, que pone `ContactsService` (`talkingWith`): las salas del relay no lo tienen.
- El mismo estado de `bubble` alimenta `PttKeys` (botón del auricular por una
  `MediaSession` sólo mientras hay sala; volumen por `dispatchKeyEvent`, sólo con la app a
  la vista) y el tile `PttTileService`, que lee `PttBubble.mode/talking/paused/room`.
  En los botones físicos una pulsación corta conmuta y mantener es pulsar-para-hablar: el
  botón de cable sólo llega como clic (Android se queda la pulsación larga para el
  asistente).
- Probar los botones sin auricular: `adb shell input keyevent KEYCODE_HEADSETHOOK` (clic) o
  `adb shell cmd media_session dispatch play-pause`.
- Para probarla sin tocar el teléfono: `adb shell appops set dev.gravitaltalk.gravital_talk_app
  SYSTEM_ALERT_WINDOW allow`, entrar en una sala, `input keyevent KEYCODE_HOME` y
  `input swipe X Y X Y 2000` sobre la burbuja (un *swipe* sin moverse es mantener pulsado).

## Sala en segundo plano (Android)

`SessionService` es un servicio en primer plano (`microphone|mediaPlayback`) que arranca
al conectar y se para al salir; no mueve audio, sólo evita que Android corte el micrófono
y congele el proceso. El tipo `microphone` sólo se pide con el permiso concedido: pedirlo
sin él lanza `SecurityException`. Se gobierna desde Dart por
`app/lib/services/background_session.dart`.

## Gotchas conocidos

- `make clippy` falla en `main` por `crates/gravital-talk-audit/tests/spike_multipeer.rs`
  (`listener` sin usar). Para validar el resto: `--workspace --exclude gravital-talk-audit`.
- Un frame PCM tiene que caber en un paquete: 1156 B útiles. A 48 kHz mono eso
  son 10 ms; la app recorta sola (`effectiveFrameMs`), el CLI necesita `--frame-ms 10`.
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