# Gravital Talk

Voz **pulsa y habla** entre dispositivos, directa y cifrada de extremo a extremo, sin
cuentas y sin un servidor central. Un protocolo de audio en tiempo real escrito en Rust
y una app (Android y escritorio Linux) que lo usa.

**Estado: alpha (`0.3.0-alpha.1`).** Funciona y se usa a diario para probarlo, pero el
formato de red y la ABI pueden cambiar entre versiones, y hay límites conocidos que se
listan sin adornos en [Qué funciona y qué no](#qué-funciona-y-qué-no).

Licencia: MIT o Apache-2.0, a elegir.

---

## Índice

- [Qué es](#qué-es)
- [Instalar](#instalar)
- [Usar la app](#usar-la-app)
- [Cómo se conectan dos dispositivos](#cómo-se-conectan-dos-dispositivos)
- [Qué funciona y qué no](#qué-funciona-y-qué-no)
- [Arquitectura y stack](#arquitectura-y-stack)
- [Compilar desde el código](#compilar-desde-el-código)
- [Pruebas y medidas](#pruebas-y-medidas)
- [Línea de órdenes, relay y FFI](#línea-de-órdenes-relay-y-ffi)
- [Publicar instalables](#publicar-instalables)
- [Documentación](#documentación)

---

## Qué es

- **Pulsa y habla.** Mientras mantienes el botón, te oyen; al soltar, escuchas. Un turno
  cada vez, como un walkie-talkie.
- **Directo.** El audio va de un dispositivo al otro por UDP. No pasa por ningún
  servidor salvo que tú configures uno de respaldo.
- **Cifrado de extremo a extremo.** Intercambio de claves Noise (`NNpsk0`, X25519) con
  el secreto de la invitación, y ChaCha20-Poly1305 por paquete.
- **Sin cuentas.** Un dispositivo se identifica por lo que comparte con otro al
  emparejarse, nada más.

## Instalar

No hace falta compilar nada. Los instalables se publican a mano en el release
[`builds`](https://github.com/angelnereira/Gravital-Talk/releases/tag/builds):

| Plataforma | Fichero | Cómo |
|---|---|---|
| Android (7.0+) | [`gravital-talk-latest.apk`](https://github.com/angelnereira/Gravital-Talk/releases/download/builds/gravital-talk-latest.apk) | Descargar e instalar; hay que permitir «instalar apps desconocidas». |
| Linux x64 | `gravital-talk-latest-linux-x64.tar.gz` | Descomprimir y ejecutar `gravital-talk/gravital_talk_app`. |
| Android, versión anterior | `gravital-talk-previous.apk` | Para volver atrás si la última da problemas. |

Notas:

- El APK va firmado con una clave de desarrollo, no con una de publicación. Sirve para
  instalar y actualizar entre builds de este repositorio; no está en ninguna tienda.
- **Los dos dispositivos deben llevar el mismo build.** El protocolo todavía cambia, y
  una versión vieja con una nueva puede conectar y no oírse.
- iOS, Windows y macOS: la app no se ha compilado ni probado ahí.

## Usar la app

### Empezar a hablar con alguien

1. En un dispositivo, **Invitar**. Aparece un código QR y un enlace.
2. En el otro, **Escanear** el código, o abrir el enlace.
3. Los dos quedan guardados como contactos y, si quien invitó sigue en esa pantalla, la
   conversación **arranca sola**.

El enlace tiene la forma `gravital-talk://i?…`. Formas de abrirlo sin teclear:

- **Android**: tocarlo donde el chat lo muestre como enlace; o mantener pulsado el
  mensaje → *Compartir* → *Gravital Talk*; o copiarlo y tocar **Abrir invitación
  copiada** en la pantalla de inicio.
- **Linux**: la app se registra como manejadora de `gravital-talk://`; si ya está
  abierta, el enlace se le entrega a esa ventana.
- WhatsApp y otros no convierten `gravital-talk://` en enlace pulsable. No hay página
  web intermedia que lo resuelva: para eso están el QR y las dos vías anteriores.

### Volver a hablar

Cada contacto tiene un botón de llamada. Al otro le aparece «X quiere hablar» y, al
aceptar, entra. Con la app en segundo plano llega como notificación si en *Ajustes →
Contactos* está activado **Recibir llamadas en segundo plano** (Android).

### Durante la conversación

- **Mantén pulsado** el botón para hablar.
- **Pausar**: dejas de oír y de poder hablar sin perder la conexión.
- **Invitar a alguien más** (quien abrió la conversación): más personas pueden sumarse.
- En Android la conversación sigue con la app oculta o la pantalla apagada; la
  notificación permite hablar, soltar y salir sin abrir la app.

### Ajustes

| Sección | Qué hay |
|---|---|
| Permisos | Micrófono, cámara (para escanear) y notificaciones, con su estado. |
| Audio | Calidad (128, 64, 32 o 16 kbps), reducción de ruido, tonos de turno. |
| Conexión | Comprobar la red propia; servidor de respaldo opcional. |
| Contactos | Tu nombre visible; recibir llamadas en segundo plano. |
| Acerca de | Versión, motor de audio y, si la app se cerró sola, el informe para copiar. |

## Cómo se conectan dos dispositivos

Tener internet permite **salir**; para hablar directo, al menos uno tiene que poder
**recibir** del otro. Lo que hace la app, por orden:

1. **Misma red** (misma wifi): conexión inmediata por la red local.
2. **Redes distintas**: cada uno averigua su dirección pública (STUN) y ambos envían a
   la vez para abrir sus routers («perforar el NAT»), con barrido de puertos para los
   NAT que cambian de puerto. Para esto cada uno necesita la dirección del otro: se la
   pasan solos, cifrada, por un **buzón público** (servidores de mensajería MQTT de
   terceros). Si ninguno responde, queda el camino manual: el invitado enseña un
   código de respuesta.
3. **Router que abre el puerto solo** (UPnP): si el router lo permite, el anfitrión
   queda alcanzable sin más pasos.
4. **Servidor de respaldo** (opcional, `gs-relay` desplegado por ti): presenta a los
   dos y, si no hay camino directo, reenvía el audio cifrado. Apagado por defecto.

Límites reales:

- Si los **dos** están tras un NAT simétrico (habitual entre dos móviles con datos), no
  existe camino directo. Sin servidor de respaldo no conectan.
- Una llamada a un contacto sólo llega si el otro tiene la app en marcha y es
  alcanzable. No hay notificaciones *push* de un tercero.
- El buzón público sólo sirve para encontrarse: no lleva audio, no puede leer lo que
  transporta y no es nuestro. Es gratuito y a ratos no responde; por eso se usan varios
  y, sin ninguno, la app sigue funcionando en la red local.

## Qué funciona y qué no

Lo marcado como **medido** se comprobó con dispositivos reales (un Pixel 8 Pro y un
portátil Linux); lo demás lo cubren pruebas automáticas o no se ha comprobado.

| Área | Estado |
|---|---|
| Audio bidireccional, misma wifi | Funciona. **Medido**: 0–5 % de huecos por turno, ~170–220 ms del aviso al sonido (portátil → teléfono). |
| Audio entre wifi de casa y datos móviles | Ha funcionado en pruebas puntuales con perforación y barrido de puertos; no hay estadística de fiabilidad. |
| Invitación de un solo enlace, misma red | Funciona. **Medido**: 4 s desde abrir el enlace hasta estar hablando. |
| Invitación y llamadas entre redes distintas | La señalización viaja por buzones públicos (MQTT, de terceros). **Medido** con la señalización local apagada: invitación completa entre dos apps y llamada de portátil a teléfono. Con el teléfono en datos móviles **no** se ha podido probar. Depende de que algún servidor público responda. |
| Contactos y llamadas entre ellos | Funciona en la misma red. Entre redes sólo si el otro es alcanzable. |
| Salas de más de dos | El núcleo lo soporta y hay pruebas automáticas; con tres dispositivos reales no se ha probado. |
| Segundo plano en Android | Funciona. **Medido** en Android 17: micrófono activo con la app oculta, notificación con hablar/soltar/salir. |
| Códec de voz | Opus con corrección de pérdidas, ocultación de huecos, tasa adaptativa y reducción de ruido (RNNoise). |
| Tonos de turno | Suenan en local al empezar y terminar un turno. |
| Audio de baja latencia en Android (AAudio) | Compilado y **apagado**: en la prueba dio menos retardo pero más cortes. |
| La app se cierra a los segundos en algunos dispositivos | **Sin resolver.** No se ha podido reproducir; la app guarda el motivo del cierre en *Ajustes → Acerca de* para diagnosticarlo. |
| App en iOS, Windows, macOS | Sin compilar ni probar. |
| SDK de Python y de web (WASM) | Existen en `sdks/`; no se han revisado en esta etapa. |
| Integración continua | Los flujos existen pero **no se ejecutan** (cuenta bloqueada). Todo se verifica y publica en local. |

## Arquitectura y stack

```
            ┌────────────────────────── app/ (Flutter) ──────────────────────────┐
            │ pantallas · SessionController · ContactsService · AudioPump         │
            └───────────────────────────────┬─────────────────────────────────────┘
                                            │ dart:ffi  (ABI C: gs_*)
┌───────────────────────────────────────────┴───────────────────────────────────────┐
│ gravital-talk-ffi   handles, runtime por sesión, códec de voz, audio nativo        │
├────────────────────────────────────────────────────────────────────────────────────┤
│ gravital-talk (fachada)                                                             │
├───────────────┬───────────────────────┬──────────────────┬─────────────────────────┤
│ core (no_std) │ transport (tokio)     │ codec            │ io (cpal)               │
│ cabecera 24 B │ UDP, STUN, UPnP, NAT  │ Opus + RNNoise   │ micrófono y altavoz     │
│ cifrado, FSM  │ jitter, FEC, salas    │ en Rust puro     │ (escritorio)            │
└───────────────┴───────────────────────┴──────────────────┴─────────────────────────┘
        metrics (RTT, jitter, pérdida, MOS)   ·   relay   ·   cli   ·   audit
```

- **Núcleo**: Rust 1.99. `gravital-talk-core` es `no_std`; todo el protocolo vive en
  Rust y cualquier otro lenguaje lo consume por la ABI C de `gravital-talk-ffi`.
- **Red**: UDP, tokio. Cabecera de 24 bytes que va autenticada; nonce derivado del
  número de secuencia y del identificador de sesión.
- **Cifrado**: Noise `NNpsk0` (X25519, ChaCha20-Poly1305) con el secreto de la sala;
  anti-repetición por ventana deslizante.
- **Audio**: Opus (`unsafe-libopus`) y reducción de ruido (`nnnoiseless`), ambos en
  Rust puro. 48 kHz mono, tramas de 20 ms, 16–128 kbps.
- **App**: Flutter (Dart). En escritorio el audio lo capta y reproduce la librería
  nativa (cpal); en Android, los plugins `record` y `flutter_sound`.
- **Android**: Kotlin para lo que Flutter no cubre: servicio en primer plano, enlaces,
  compartir, motivo del último cierre.

Carpetas:

```
crates/    los 10 crates del workspace (core, metrics, transport, codec, io,
           fachada, ffi, cli, relay, audit)
app/       la app Flutter (Android y Linux)
android/   app Android antigua en Kotlin/JNI; ya no es la que se publica
sdks/      Python (PyO3) y web (wasm-bindgen), fuera del workspace
docs/      especificación del protocolo, emparejamiento, seguridad, ADRs
scripts/   compilar el APK, publicar instalables, simulador de NAT
```

## Compilar desde el código

Requisitos:

- Rust 1.99 (lo fija `rust-toolchain.toml`).
- Linux: `libasound2-dev libopus-dev pkg-config`, más las dependencias de Flutter para
  escritorio (`libgtk-3-dev`, `clang`, `cmake`, `ninja-build`).
- Flutter estable, para la app.
- Android: JDK 17, SDK y NDK de Android, y `cargo install cargo-ndk --locked` con los
  targets `aarch64-linux-android armv7-linux-androideabi x86_64-linux-android`.

```bash
# Núcleo, sin audio ni dependencias del sistema
cargo test -p gravital-talk-core -p gravital-talk-metrics -p gravital-talk-transport

# App de escritorio Linux (incluye el motor de audio nativo)
make app-linux
app/build/linux/x64/release/bundle/gravital_talk_app

# APK instalable → outputs/android/release/
./scripts/build-apk.sh
adb install -r "outputs/android/release/$(cat outputs/android/release/LATEST.txt)"
```

## Pruebas y medidas

```bash
cargo fmt --all -- --check
cargo clippy --workspace --exclude gravital-talk-audit --all-targets -- \
  -D warnings -W clippy::perf -W clippy::nursery
cargo test --workspace --exclude gravital-talk-audit --all-targets
cargo test -p gravital-talk-codec --no-default-features --features voice
make ffi-smoke                      # compila y ejecuta una prueba en C contra la ABI
cd app && flutter analyze && flutter test
```

Además:

- `make audit` comprueba que el **audio** llega (frecuencia y nivel del PCM recibido),
  no sólo que llegan paquetes.
- `make nat-sim` reproduce NAT reales con espacios de nombres de red (pide `sudo`).
- **Cortes y latencia** se miden de forma acústica: el portátil emite un tono por la
  conversación y se graba cómo sale por el altavoz del teléfono. El procedimiento está
  en `AGENTS.md` («Medir audio entrecortado y latencia»).

`gravital-talk-audit` se excluye de `clippy` y de los tests del workspace por un fallo
propio que ya estaba en la rama principal.

## Línea de órdenes, relay y FFI

- **`gs`** (`gravital-talk-cli`): `send`, `receive`, `ptt`, `peer`, `relay`, `room`,
  `discover`, `bench`, `info`, `doctor`, `devices`. `gs peer` sirve para probar la
  conexión entre dos máquinas sin la app.
- **`gs-relay`** (`gravital-talk-relay`): servidor opcional. Presenta a dos dispositivos
  y, si no hay camino directo, reenvía el audio cifrado, que no puede leer. Expone
  `/metrics` y `/healthz`. Nadie tiene que desplegarlo para usar la app en una red
  local o entre redes que se dejen perforar.
- **ABI C** (`gravital-talk-ffi`): funciones `gs_*`, cabecera generada en
  `crates/gravital-talk-ffi/include/gravital_talk.h`. Los cambios son sólo aditivos.

## Publicar instalables

Los binarios no se versionan en git ni dependen de la integración continua:

```bash
scripts/publish-builds.sh --build   # compila Android y Linux y los sube al release `builds`
```

El script comprueba que el APK corresponde al commit actual y que el paquete de Linux
lleva audio nativo, pasa el APK anterior a `previous` y verifica lo publicado.

## Documentación

| Documento | Contenido |
|---|---|
| `AGENTS.md` | Guía de trabajo del repositorio: órdenes, convenciones y trampas conocidas. La más al día. |
| `CHANGELOG.md` | Qué cambió y por qué, versión a versión. |
| `docs/pairing.md` | Invitaciones, perforación de NAT, contactos y llamadas. |
| `docs/protocol-spec.md`, `docs/packet-format.md`, `docs/session-model.md` | El protocolo. |
| `docs/security.md` | Modelo de amenazas. |
| `docs/roadmap.md`, `docs/adr/` | Planes y decisiones de diseño. |

Parte de `docs/` describe etapas anteriores del proyecto y puede no coincidir con el
código actual; ante la duda mandan `AGENTS.md`, `CHANGELOG.md` y el propio código.
