# Gravital Sound — Plan de Proyecto y Arquitectura de Repositorio

**Versión:** 1.0  
**Autor:** Angel — Nereira Technology and Business Solutions  
**Fecha:** Abril 2026  
**Estado:** Planificación

---

## 1. Identidad del proyecto

**Nombre:** Gravital Sound  
**Organización GitHub:** `gravital` (o `nereira`)  
**Repositorio:** `gravital-sound`  
**Dominio:** `gravitalsound.dev` (documentación y landing)  
**Licencia:** Dual MIT / Apache-2.0 (estándar del ecosistema Rust, compatible con uso comercial y embebido)

Gravital Sound es una división técnica bajo el paraguas de Gravital, la marca comercial de Nereira Technology and Business Solutions. Se posiciona junto a Gravital Cloud, Gravital Security (Quimera), Gravital ID y las demás divisiones como un componente de infraestructura que puede operar de forma independiente o integrarse con el ecosistema Gravital.

La relación con el ecosistema Gravital es clara pero no obligatoria: Gravital Sound funciona como biblioteca standalone para cualquier desarrollador, pero ofrece integración nativa con Gravital ID para autenticación de sesión y con Gravital Cloud para persistencia de métricas y gestión de sesiones cuando el contexto lo requiere.

---

## 2. Principio arquitectónico: portabilidad universal

El requisito de que Gravital Sound se instale y ejecute en cualquier servidor, computadora, teléfono y navegador no es un feature posterior — es la restricción de diseño más importante del proyecto. Toda decisión arquitectónica se evalúa contra esta restricción.

### 2.1 Estrategia de portabilidad

La portabilidad se logra mediante una arquitectura de tres niveles:

```
┌─────────────────────────────────────────────────────────┐
│                    SDKs idiomáticos                      │
│  Swift · Kotlin/Java · Python · TypeScript/JS · C/C++   │
│  (wrappers que exponen API nativa de cada plataforma)    │
└────────────────────────┬────────────────────────────────┘
                         │ llaman funciones C via FFI
┌────────────────────────┴────────────────────────────────┐
│              gravital-sound-ffi (C ABI)                  │
│  Interfaz C estable, header generado con cbindgen        │
│  Compila a: .so (Linux), .dylib (macOS), .dll (Windows)  │
│             .a (static), .wasm (browser)                 │
└────────────────────────┬────────────────────────────────┘
                         │ Rust internals
┌────────────────────────┴────────────────────────────────┐
│              Rust Core (100% del protocolo)               │
│  gravital-sound-core · transport · codec · metrics        │
│  Sin dependencias de plataforma (no_std compatible        │
│  en el core, std en transport/codec)                      │
└─────────────────────────────────────────────────────────┘
```

**Nivel 1 — Rust Core.** Todo el protocolo vive en Rust. Los tipos, la serialización, la máquina de estados, el jitter buffer, el manejo de codec, las métricas. Este código se compila una vez para cada target architecture y produce una biblioteca nativa. El crate `gravital-sound-core` es `no_std` compatible (puede compilar sin la librería estándar de Rust) para soportar entornos embebidos y WASM. Los crates de transporte y codec requieren `std` pero no asumen un sistema operativo específico.

**Nivel 2 — FFI (Foreign Function Interface).** Un crate dedicado (`gravital-sound-ffi`) expone las funciones del core como funciones C con convención de llamada `extern "C"`. El header C se genera automáticamente con `cbindgen` en cada build. Esta es la interfaz estable que consume cualquier lenguaje que pueda llamar funciones C, que es prácticamente todos: Swift, Kotlin/JNI, Python/ctypes, Node.js/ffi-napi, C++, Go, C#, Ruby, Dart.

**Nivel 3 — SDKs idiomáticos.** Wrappers delgados en el lenguaje nativo de cada plataforma que envuelven las funciones C en APIs que se sienten naturales en cada ecosistema. El SDK de Swift usa tipos de Swift y manejo de memoria de Swift. El SDK de Kotlin usa coroutines. El SDK de TypeScript usa Promises. Ninguno reimplementa lógica de protocolo — todos llaman al mismo core compilado.

### 2.2 Matriz de plataformas objetivo

| Plataforma | Target Rust | Biblioteca | Audio I/O | Transporte | SDK |
|------------|-------------|------------|-----------|------------|-----|
| Linux (servidor/desktop) | `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu` | `.so` + `.a` | ALSA / PipeWire (via `cpal`) | UDP nativo | C/C++, Python, Go |
| macOS | `x86_64-apple-darwin`, `aarch64-apple-darwin` | `.dylib` + `.a` | CoreAudio (via `cpal`) | UDP nativo | Swift, C/C++, Python |
| Windows | `x86_64-pc-windows-msvc` | `.dll` + `.lib` | WASAPI (via `cpal`) | UDP nativo | C/C++, Python, C# |
| Android | `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android` | `.so` (JNI) | AAudio/Oboe (via `cpal` o binding directo) | UDP nativo | Kotlin/Java |
| iOS | `aarch64-apple-ios`, `aarch64-apple-ios-sim` | `.a` (static, App Store requiere static linking) | CoreAudio (via `cpal`) | UDP nativo | Swift |
| Browser (WASM) | `wasm32-unknown-unknown` | `.wasm` + JS glue | WebAudio API (desde JS) | WebSocket (UDP no disponible en browser), WebTransport (cuando esté estable) | TypeScript/JS |
| Embebido / IoT | `aarch64-unknown-linux-musl`, `armv7-unknown-linux-musleabihf` | `.a` (static, musl) | ALSA directo | UDP nativo | C |

**Notas críticas por plataforma:**

**iOS:** Apple no permite dynamic linking de bibliotecas de terceros en apps distribuidas via App Store. El core se compila como biblioteca estática (`.a`) y se enlaza directamente en el binario de la app. Se distribuye como XCFramework que contiene el `.a` para device (arm64) y simulator (arm64-sim).

**Android:** La biblioteca se compila como `.so` para cada ABI soportada (arm64-v8a, armeabi-v7a, x86_64) y se empaqueta como un AAR que incluye los bindings JNI. Kotlin/Java llama al core via JNI, no via proceso externo.

**Browser/WASM:** Esta es la plataforma con más restricciones. No hay UDP en el navegador — el transporte obligatorio es WebSocket (o WebTransport cuando los navegadores lo estabilicen). El jitter buffer y el codec corren en WASM, pero la captura/reproducción de audio usa la WebAudio API del navegador, orquestada desde JavaScript. El módulo WASM se distribuye como paquete npm.

**Servidores sin audio I/O:** En un servidor que actúa como relay, mezclador o grabador, no hay dispositivo de audio. El core opera sobre streams de datos sin tocar hardware de audio. La capa de audio I/O (`cpal`) es una dependencia opcional que solo se incluye cuando el target la necesita.

### 2.3 Distribución e instalación por plataforma

| Plataforma | Método de instalación |
|------------|----------------------|
| Rust (cualquier plataforma) | `cargo add gravital-sound` — uso directo como crate |
| C/C++ (cualquier plataforma) | Descargar release binario (`.so`/`.dylib`/`.dll` + header `gravital_sound.h`) o compilar desde source |
| Python | `pip install gravital-sound` — wheel con binario precompilado (via `maturin` / PyO3) |
| Swift (iOS/macOS) | Swift Package Manager apuntando al repo, o CocoaPods con XCFramework precompilado |
| Kotlin/Java (Android) | Dependencia Gradle: `implementation("dev.gravital:sound:0.1.0")` — AAR desde Maven Central o GitHub Packages |
| TypeScript/JS (Node.js) | `npm install @gravital/sound` — binding nativo via `napi-rs` |
| TypeScript/JS (Browser) | `npm install @gravital/sound-web` — módulo WASM + WebSocket transport |
| Go | `cgo` linking contra la biblioteca C, o wrapper Go puro si se justifica |
| Docker (servidor) | `docker pull gravital/sound-relay` — imagen mínima con el daemon de relay |
| Linux packages | `.deb` y `.rpm` para el CLI y el daemon (via `cargo-deb`, `cargo-generate-rpm`) |

---

## 3. Estructura del repositorio

```
gravital-sound/
│
├── Cargo.toml                              # Workspace root
├── Cross.toml                              # Configuración de cross-compilation
├── Makefile                                # Targets de build por plataforma
├── README.md
├── LICENSE-MIT
├── LICENSE-APACHE
├── CHANGELOG.md
├── CONTRIBUTING.md
├── SECURITY.md
│
├── docs/                                   # Documentación técnica del protocolo
│   ├── overview.md                         # Visión, contexto, posicionamiento
│   ├── protocol-spec.md                    # Especificación formal completa
│   ├── packet-format.md                    # Estructura binaria con diagramas
│   ├── session-model.md                    # Máquina de estados y ciclo de vida
│   ├── transport.md                        # Justificación de transporte y plan
│   ├── security.md                         # Modelo de amenazas
│   ├── portability.md                      # Guía de portabilidad y platform notes
│   ├── benchmarks.md                       # Resultados de rendimiento
│   └── adr/                                # Architecture Decision Records
│       ├── 001-rust-core-with-c-ffi.md
│       ├── 002-udp-first-transport.md
│       ├── 003-24-byte-header.md
│       ├── 004-opus-as-default-codec.md
│       ├── 005-no-std-core.md
│       ├── 006-cpal-for-audio-io.md
│       ├── 007-wasm-websocket-browser.md
│       └── 008-xcframework-ios.md
│
│
│   ════════════════════════════════════════
│   RUST CRATES (el core del proyecto)
│   ════════════════════════════════════════
│
├── crates/
│   │
│   ├── gravital-sound-core/                # Núcleo del protocolo (no_std compatible)
│   │   ├── Cargo.toml                      # [no default features, optional std]
│   │   └── src/
│   │       ├── lib.rs                      # Re-exports públicos
│   │       ├── packet.rs                   # Packet struct, encode/decode sobre &[u8]
│   │       ├── header.rs                   # Header parsing, campo por campo
│   │       ├── message.rs                  # MessageType enum, payload structs
│   │       ├── session.rs                  # SessionState enum, transiciones type-safe
│   │       ├── checksum.rs                 # CRC-16 implementation (no_std)
│   │       ├── fragment.rs                 # Lógica de fragmentación/reensamblado
│   │       ├── error.rs                    # Error types (no_std: no usa std::error::Error)
│   │       └── constants.rs               # Versión del protocolo, defaults, límites
│   │
│   ├── gravital-sound-transport/           # Capa de transporte (requiere std)
│   │   ├── Cargo.toml                      # Deps: tokio, socket2
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs                   # Transport trait: send(), recv(), close()
│   │       ├── udp.rs                      # UdpTransport (tokio)
│   │       ├── websocket.rs                # WebSocketTransport (tokio-tungstenite)
│   │       ├── jitter_buffer.rs            # Ring buffer con configurable depth
│   │       └── session_manager.rs          # Orquestación de handshake y lifecycle
│   │
│   ├── gravital-sound-codec/               # Codecs y manejo de frames de audio
│   │   ├── Cargo.toml                      # Deps: opus (optional), rubato (resampling)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs                   # Codec trait: encode_frame(), decode_frame()
│   │       ├── opus.rs                     # Opus via libopus bindings
│   │       ├── pcm.rs                      # PCM crudo (referencia/benchmark)
│   │       └── resampler.rs               # Sample rate conversion
│   │
│   ├── gravital-sound-metrics/             # Observabilidad y medición
│   │   ├── Cargo.toml                      # Deps: tracing (optional)
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── rtt.rs                      # RTT moving average
│   │       ├── jitter.rs                   # Jitter calculation (RFC 3550)
│   │       ├── loss.rs                     # Packet loss tracking (bitmap window)
│   │       └── quality.rs                  # MOS-LQ estimation
│   │
│   ├── gravital-sound-io/                  # Audio I/O (captura y playback de hardware)
│   │   ├── Cargo.toml                      # Deps: cpal
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── traits.rs                   # AudioInput/AudioOutput traits
│   │       ├── cpal_backend.rs             # Backend usando cpal (ALSA, CoreAudio, WASAPI, AAudio)
│   │       └── null_backend.rs             # Null sink/source para servidores y tests
│   │
│   ├── gravital-sound-ffi/                 # C ABI — la capa de portabilidad universal
│   │   ├── Cargo.toml                      # crate-type = ["cdylib", "staticlib"]
│   │   ├── cbindgen.toml                   # Config para generación del header C
│   │   ├── build.rs                        # Genera gravital_sound.h en cada build
│   │   └── src/
│   │       ├── lib.rs                      # Exports extern "C"
│   │       ├── session.rs                  # gs_session_create, gs_session_connect, etc.
│   │       ├── packet.rs                   # gs_packet_encode, gs_packet_decode
│   │       ├── transport.rs                # gs_transport_udp_new, gs_transport_send
│   │       ├── audio.rs                    # gs_audio_open_input, gs_audio_open_output
│   │       ├── metrics.rs                  # gs_metrics_get_rtt, gs_metrics_get_loss
│   │       ├── error.rs                    # gs_error_last, gs_error_message
│   │       └── types.rs                    # Opaque handle types (GsSession*, GsTransport*)
│   │
│   └── gravital-sound-cli/                 # Herramienta de línea de comandos
│       ├── Cargo.toml                      # Deps: clap, tokio
│       └── src/
│           └── main.rs                     # Subcommands: send, receive, bench, info, doctor
│
│
│   ════════════════════════════════════════
│   SDKs POR PLATAFORMA
│   ════════════════════════════════════════
│
├── sdks/
│   │
│   ├── swift/                              # SDK para iOS y macOS
│   │   ├── Package.swift                   # Swift Package Manager manifest
│   │   ├── Sources/
│   │   │   └── GravitalSound/
│   │   │       ├── GravitalSound.swift     # API pública de Swift
│   │   │       ├── Session.swift           # GravitalSoundSession class
│   │   │       ├── Transport.swift         # Wrapper de transporte
│   │   │       ├── AudioIO.swift           # Wrapper de CoreAudio
│   │   │       └── Metrics.swift           # Métricas observables
│   │   ├── Tests/
│   │   └── GravitalSoundFFI/              # XCFramework o bridging header
│   │       └── module.modulemap
│   │
│   ├── kotlin/                             # SDK para Android
│   │   ├── build.gradle.kts
│   │   ├── src/main/
│   │   │   ├── kotlin/dev/gravital/sound/
│   │   │   │   ├── GravitalSound.kt       # API pública de Kotlin
│   │   │   │   ├── Session.kt             # Session con coroutines
│   │   │   │   ├── Transport.kt
│   │   │   │   ├── AudioIO.kt             # Wrapper de AAudio/Oboe
│   │   │   │   └── Metrics.kt
│   │   │   └── jni/
│   │   │       └── NativeBridge.kt         # JNI declarations
│   │   ├── src/main/jniLibs/              # .so precompilados por ABI
│   │   │   ├── arm64-v8a/
│   │   │   ├── armeabi-v7a/
│   │   │   └── x86_64/
│   │   └── src/main/c/
│   │       └── jni_bridge.c               # Glue JNI → C ABI
│   │
│   ├── python/                             # SDK para Python (servidor/desktop/scripting)
│   │   ├── pyproject.toml                  # Build con maturin (PyO3)
│   │   ├── Cargo.toml                      # PyO3 crate
│   │   ├── src/lib.rs                      # PyO3 module bindings
│   │   ├── gravital_sound/
│   │   │   ├── __init__.py
│   │   │   ├── session.py                  # Pythonic API
│   │   │   ├── transport.py
│   │   │   └── metrics.py
│   │   └── tests/
│   │
│   ├── node/                               # SDK para Node.js (server-side JS/TS)
│   │   ├── package.json
│   │   ├── Cargo.toml                      # napi-rs crate
│   │   ├── src/lib.rs                      # napi bindings
│   │   ├── index.ts                        # TypeScript API
│   │   ├── index.d.ts                      # Type declarations
│   │   └── __tests__/
│   │
│   └── web/                                # SDK para navegadores (WASM)
│       ├── package.json                    # @gravital/sound-web
│       ├── Cargo.toml                      # wasm-bindgen crate
│       ├── src/
│       │   └── lib.rs                      # wasm-bindgen exports
│       ├── js/
│       │   ├── index.ts                    # API TypeScript
│       │   ├── audio-worklet.ts            # AudioWorklet para procesamiento de audio
│       │   ├── websocket-transport.ts      # WebSocket transport (JS-side)
│       │   └── webtransport-transport.ts   # WebTransport (experimental)
│       └── __tests__/
│
│
│   ════════════════════════════════════════
│   EJEMPLOS, TESTS, BENCHMARKS, CI
│   ════════════════════════════════════════
│
├── examples/
│   ├── rust/
│   │   ├── sender.rs                       # Envía audio sintético por UDP
│   │   ├── receiver.rs                     # Recibe y escribe a archivo
│   │   ├── loopback.rs                     # Benchmark de latencia loopback
│   │   ├── live_audio.rs                   # Mic → protocolo → speaker
│   │   └── relay_server.rs                 # Servidor relay básico
│   ├── python/
│   │   └── simple_session.py               # Sesión de audio con el SDK Python
│   ├── swift/
│   │   └── iOSExample/                     # App iOS mínima de ejemplo
│   ├── kotlin/
│   │   └── AndroidExample/                 # App Android mínima de ejemplo
│   └── web/
│       └── browser-demo/                   # Demo HTML + JS con WASM
│           ├── index.html
│           └── app.ts
│
├── fuzz/                                   # cargo-fuzz targets
│   └── fuzz_targets/
│       ├── decode_packet.rs
│       └── decode_handshake.rs
│
├── benches/                                # criterion benchmarks
│   ├── encode_decode.rs
│   ├── checksum.rs
│   ├── jitter_buffer.rs
│   └── throughput.rs
│
├── tests/                                  # Tests de integración cross-crate
│   ├── session_lifecycle.rs
│   ├── handshake_flow.rs
│   ├── loss_recovery.rs
│   └── cross_platform.rs                   # Tests que validan FFI roundtrip
│
├── scripts/
│   ├── build-ios.sh                        # Compila XCFramework para iOS
│   ├── build-android.sh                    # Compila .so para cada ABI Android
│   ├── build-wasm.sh                       # Compila WASM + genera JS glue
│   ├── build-python-wheel.sh               # Genera wheels con maturin
│   ├── build-all.sh                        # Build completo para todas las plataformas
│   ├── generate-header.sh                  # Regenera gravital_sound.h con cbindgen
│   └── run-cross-tests.sh                  # Ejecuta tests en targets cross-compiled
│
├── docker/
│   ├── Dockerfile.relay                    # Imagen para relay server
│   ├── Dockerfile.builder                  # Imagen con toolchains de cross-compilation
│   └── docker-compose.yml                  # Setup de test con dos nodos + relay
│
└── .github/
    └── workflows/
        ├── ci.yml                          # Tests, clippy, fmt, miri (en push/PR)
        ├── cross-compile.yml               # Build para todas las plataformas (en release)
        ├── bench.yml                       # Benchmarks de regresión (semanal)
        ├── fuzz.yml                        # Fuzzing continuo (semanal)
        ├── release.yml                     # Publicación de crates, SDKs, binarios
        └── docs.yml                        # Deploy de documentación a gravitalsound.dev
```

---

## 4. Dependencias del proyecto

### 4.1 Dependencias Rust por crate

| Crate | Dependencia | Versión | Propósito | Notas |
|-------|-------------|---------|-----------|-------|
| `core` | — | — | Sin dependencias externas | `no_std` puro, solo libcore/liballoc |
| `transport` | `tokio` | 1.x | Async runtime, UDP I/O | Feature flags: `rt-multi-thread`, `net`, `time` |
| `transport` | `socket2` | 0.5.x | Configuración avanzada de sockets (SO_REUSEADDR, buffer sizes) | |
| `transport` | `tokio-tungstenite` | 0.24.x | WebSocket transport | Feature flag opcional |
| `codec` | `opus` | 0.3.x | Bindings a libopus | Requiere `libopus-dev` en el sistema o vendored build |
| `codec` | `rubato` | 0.15.x | Resampling de audio (Rust puro, no deps C) | |
| `metrics` | `tracing` | 0.1.x | Logging estructurado | Feature flag opcional |
| `io` | `cpal` | 0.15.x | Audio I/O cross-platform | Soporta ALSA, CoreAudio, WASAPI, AAudio |
| `ffi` | `cbindgen` | 0.27.x | Generación de header C | Solo build-time (build.rs) |
| `cli` | `clap` | 4.x | Parsing de argumentos CLI | |
| `cli` | `hound` | 3.x | Lectura/escritura de WAV | Para grabar output a archivo |

### 4.2 Dependencias por SDK

| SDK | Dependencia | Propósito |
|-----|-------------|-----------|
| Python | `maturin` + `PyO3` | Build de wheels con bindings nativos |
| Node.js | `napi-rs` | Bindings Node.js nativos sin overhead de FFI genérico |
| Web/WASM | `wasm-bindgen` + `wasm-pack` | Compilación a WASM y generación de JS glue |
| Swift | Xcode toolchain + `swift-bridge` (opcional) | Bridging module o uso directo del header C |
| Kotlin | Android NDK + JNI | Cross-compilation para ABIs de Android |

### 4.3 Toolchain de cross-compilation

| Tool | Propósito |
|------|-----------|
| `cross` (cross-rs) | Cross-compilation de Rust en containers Docker con toolchains pre-configurados |
| `cargo-ndk` | Simplifica compilación de Rust para Android NDK targets |
| `cargo-lipo` / `xcframework` tooling | Genera universal binaries y XCFrameworks para iOS/macOS |
| `wasm-pack` | Pipeline de compilación WASM → npm package |
| `cargo-deb` | Genera paquetes `.deb` para distribución Linux |
| `cargo-generate-rpm` | Genera paquetes `.rpm` para distribución Linux |

---

## 5. Diseño de la capa FFI

La capa FFI es el componente más crítico para la portabilidad. Define la interfaz estable que consume todo lenguaje que no es Rust.

### 5.1 Convenciones de la API C

**Prefijo:** Todas las funciones exportadas usan el prefijo `gs_` (Gravital Sound). Todos los tipos usan el prefijo `Gs`.

**Manejo de memoria:** Los objetos creados por la biblioteca (sesiones, transportes, streams) se devuelven como handles opacos (`GsSession*`, `GsTransport*`). Cada `gs_*_create()` tiene un `gs_*_destroy()` correspondiente. La responsabilidad de liberar es siempre del llamador. La biblioteca nunca libera memoria que no haya creado ella.

**Manejo de errores:** Las funciones devuelven `GsResult` (un enum numérico). El último error detallado se consulta con `gs_error_last()` que devuelve un `const char*` con un mensaje legible. Este string es propiedad de la biblioteca y es válido hasta la siguiente llamada a cualquier función `gs_*`.

**Thread safety:** Las funciones de sesión no son thread-safe — se documenta explícitamente que cada `GsSession` debe usarse desde un solo hilo (o con sincronización externa). Las funciones de métricas sí son thread-safe (lecturas atómicas).

### 5.2 API C de referencia

```c
/* gravital_sound.h — generado por cbindgen, no editar manualmente */

#ifndef GRAVITAL_SOUND_H
#define GRAVITAL_SOUND_H

#include <stdint.h>
#include <stdbool.h>

/* ── Tipos opacos ────────────────────────────────────── */

typedef struct GsSession    GsSession;
typedef struct GsTransport  GsTransport;
typedef struct GsAudioIn    GsAudioIn;
typedef struct GsAudioOut   GsAudioOut;

/* ── Resultado y errores ─────────────────────────────── */

typedef enum {
    GS_OK                  = 0,
    GS_ERR_INVALID_ARG     = 1,
    GS_ERR_NETWORK         = 2,
    GS_ERR_TIMEOUT         = 3,
    GS_ERR_SESSION_CLOSED  = 4,
    GS_ERR_CODEC           = 5,
    GS_ERR_AUDIO_DEVICE    = 6,
    GS_ERR_DECODE          = 7,
    GS_ERR_CHECKSUM        = 8,
    GS_ERR_STATE            = 9,   /* transición de estado inválida */
    GS_ERR_INTERNAL        = 255,
} GsResult;

const char* gs_error_last(void);

/* ── Configuración ───────────────────────────────────── */

typedef struct {
    uint32_t sample_rate;       /* Hz (44100, 48000) */
    uint8_t  channels;          /* 1 = mono, 2 = stereo */
    uint8_t  frame_duration_ms; /* 5, 10, 20, 40, 60 */
    uint32_t max_bitrate;       /* bps, 0 = default */
    uint8_t  codec;             /* 0 = auto, 1 = opus, 2 = pcm */
    uint16_t jitter_buffer_ms;  /* tamaño del jitter buffer */
} GsConfig;

GsConfig gs_config_default(void);

/* ── Sesión ──────────────────────────────────────────── */

GsResult   gs_session_create(const GsConfig* config, GsSession** out);
GsResult   gs_session_connect(GsSession* session, const char* host, uint16_t port);
GsResult   gs_session_accept(GsSession* session, const char* bind_addr, uint16_t port);
GsResult   gs_session_pause(GsSession* session);
GsResult   gs_session_resume(GsSession* session);
GsResult   gs_session_close(GsSession* session);
void       gs_session_destroy(GsSession* session);

uint8_t    gs_session_state(const GsSession* session);  /* devuelve GsSessionState enum */
uint32_t   gs_session_id(const GsSession* session);

/* ── Audio I/O ───────────────────────────────────────── */

GsResult   gs_session_send_audio(GsSession* session, const int16_t* samples,
                                  uint32_t num_samples);
GsResult   gs_session_recv_audio(GsSession* session, int16_t* buffer,
                                  uint32_t buffer_size, uint32_t* samples_read);

/* ── Métricas (thread-safe) ──────────────────────────── */

typedef struct {
    float    rtt_ms;
    float    jitter_ms;
    float    loss_percent;
    float    reorder_percent;
    float    buffer_fill_percent;
    float    estimated_mos;
    uint64_t packets_sent;
    uint64_t packets_received;
    uint64_t bytes_sent;
    uint64_t bytes_received;
} GsMetrics;

GsResult   gs_session_metrics(const GsSession* session, GsMetrics* out);

/* ── Versión ─────────────────────────────────────────── */

const char* gs_version(void);
uint8_t     gs_protocol_version(void);

#endif /* GRAVITAL_SOUND_H */
```

### 5.3 Ejemplo de uso desde cada lenguaje

**Rust (directo, sin FFI):**
```rust
use gravital_sound::{Session, Config};

let config = Config::default().sample_rate(48000).channels(2);
let mut session = Session::new(config)?;
session.connect("192.168.1.50:9000").await?;

while let Some(frame) = capture_audio() {
    session.send_audio(&frame).await?;
}
```

**Swift (iOS/macOS):**
```swift
import GravitalSound

let session = try GravitalSoundSession(sampleRate: 48000, channels: 2)
try session.connect(host: "192.168.1.50", port: 9000)

session.onAudioReceived { samples in
    audioPlayer.play(samples)
}
```

**Kotlin (Android):**
```kotlin
val session = GravitalSound.createSession {
    sampleRate = 48000
    channels = 2
}
session.connect("192.168.1.50", 9000)

session.audioFlow.collect { samples ->
    audioTrack.write(samples, 0, samples.size)
}
```

**Python (servidor/scripting):**
```python
import gravital_sound as gs

session = gs.Session(sample_rate=48000, channels=2)
session.connect("192.168.1.50", 9000)

while True:
    audio = session.recv_audio(frame_size=960)
    process(audio)
```

**TypeScript/Browser (WASM):**
```typescript
import { GravitalSound } from '@gravital/sound-web';

const session = await GravitalSound.create({
  sampleRate: 48000,
  channels: 2,
  transport: 'websocket'
});
await session.connect('wss://relay.gravitalsound.dev/session/abc123');

session.onAudio((samples) => {
  audioContext.playBuffer(samples);
});
```

---

## 6. Plan de implementación por fases

### Fase 0 — Scaffolding y toolchain (Semana 1)

**Objetivo:** Tener el workspace de Rust compilando, CI configurado, y cross-compilation validada para al menos 3 targets.

| Entregable | Descripción | Criterio de aceptación |
|------------|-------------|----------------------|
| Workspace Cargo | `Cargo.toml` raíz con todos los crates definidos, compilando con `cargo build` | `cargo check --workspace` pasa sin errores |
| CI básico | GitHub Actions con test, clippy, fmt | PR bloqueados si CI falla |
| Cross-compilation | Validar compilación para `x86_64-linux`, `aarch64-linux`, `wasm32` | `cross build --target aarch64-unknown-linux-gnu` exitoso |
| `Cross.toml` | Configuración de targets de cross-compilation | Targets documentados |
| README.md | Qué es, cómo compilar, roadmap | Legible por un tercero |

### Fase 1 — Especificación formal (Semana 2-3)

**Objetivo:** Documentar completamente el protocolo antes de escribir lógica de negocio. La especificación debe ser implementable por un tercero sin ver el código fuente.

| Entregable | Descripción |
|------------|-------------|
| `docs/protocol-spec.md` | Especificación completa: mensajes, estados, reglas, flujos |
| `docs/packet-format.md` | Estructura binaria con diagramas de bits, endianness, rangos válidos, ejemplos hex |
| `docs/session-model.md` | Diagrama de estados, transiciones, timeouts, reglas de heartbeat |
| `docs/transport.md` | Justificación de UDP como transporte inicial, plan para WebSocket/QUIC |
| `docs/security.md` | Modelo de amenazas, plan de seguridad por fases |
| `docs/portability.md` | Estrategia de portabilidad, platform notes, limitaciones por plataforma |
| ADRs 001-005 | Decisiones fundacionales documentadas con contexto y alternativas |

### Fase 2 — Core del protocolo (Semana 4-6)

**Objetivo:** Implementar `gravital-sound-core` como crate `no_std` con serialización eficiente y máquina de estados correcta por construcción.

| Entregable | Criterio de aceptación |
|------------|----------------------|
| `Packet` struct con encode/decode sobre `&[u8]` | Zero-copy decode, benchmark < 100ns |
| `MessageType` enum con todos los tipos | Pattern matching exhaustivo (el compilador rechaza tipos no manejados) |
| `SessionState` enum con transiciones type-safe | Transición inválida es error de compilación, no runtime |
| CRC-16 implementation | Correctitud validada contra test vectors de referencia |
| Payload structs (Handshake, AudioFrame, Heartbeat, etc.) | Serialización roundtrip verificada |
| Tests unitarios | Cobertura > 90% de líneas en core |
| Fuzzing | `cargo-fuzz` corriendo sobre `decode()` sin panics después de 10M iteraciones |
| Benchmarks | `criterion` midiendo encode y decode, resultados guardados como baseline |

### Fase 3 — Capa FFI (Semana 7-8)

**Objetivo:** Exponer el core como biblioteca C y validar que los bindings funcionan desde al menos dos lenguajes.

| Entregable | Criterio de aceptación |
|------------|----------------------|
| `gravital-sound-ffi` crate con exports `extern "C"` | Compila como `cdylib` + `staticlib` |
| `gravital_sound.h` generado por `cbindgen` | Header válido, verificado con `gcc -fsyntax-only` |
| Build para Linux `.so`, macOS `.dylib`, Windows `.dll` | `cross build` exitoso para los 3 targets |
| Build WASM | `wasm-pack build` produce `.wasm` + JS glue funcional |
| Test de integración C | Un programa `.c` mínimo que crea sesión, codifica/decodifica paquete |
| Test de integración Python | Script Python usando `ctypes` que llama `gs_version()` y `gs_config_default()` |

### Fase 4 — Transporte UDP (Semana 9-11)

**Objetivo:** Enviar y recibir paquetes reales sobre UDP entre dos procesos en la misma máquina y en la misma LAN.

| Entregable | Criterio de aceptación |
|------------|----------------------|
| `Transport` trait | Definido con `send()`, `recv()`, `close()` |
| `UdpTransport` implementation | Funcional con `tokio::net::UdpSocket` |
| Handshake de 3 vías funcional | Dos procesos completan handshake y transicionan a `Active` |
| Heartbeat con RTT | RTT calculado y expuesto vía métricas |
| Detección de pérdida | Gap en sequence number detectado y contado |
| `examples/sender.rs` | Envía onda sinusoidal PCM |
| `examples/receiver.rs` | Recibe, decodifica, escribe a `.wav` |
| Integration tests | Sender ↔ Receiver en localhost, validación de datos recibidos |

### Fase 5 — Audio I/O y codec (Semana 12-15)

**Objetivo:** Conectar con hardware de audio real (micrófono, speakers) e integrar Opus.

| Entregable | Criterio de aceptación |
|------------|----------------------|
| `gravital-sound-io` con backend `cpal` | Funciona en Linux (ALSA), macOS (CoreAudio), Windows (WASAPI) |
| `null_backend` | Para servidores y CI (sin hardware de audio) |
| Opus encode/decode | Roundtrip de audio a 48kHz/stereo/64kbps sin artefactos audibles |
| Jitter buffer (ring buffer fijo) | Configurable 10-200ms, absorbe jitter simulado de ±20ms |
| `examples/live_audio.rs` | Mic → encode → UDP → decode → speaker, funcional en localhost |
| Fragmentación de frames | Frames > 1200 bytes se fragmentan y reensamblan correctamente |
| Benchmark de latencia end-to-end | Medición loopback con audio real, resultado documentado |

### Fase 6 — SDKs de plataforma (Semana 16-20)

**Objetivo:** Producir SDKs funcionales para las plataformas prioritarias.

| SDK | Prioridad | Entregable | Criterio |
|-----|-----------|------------|----------|
| Python | Alta | Wheel publicable, API Pythonic | `pip install` funciona, test de sesión loopback pasa |
| Swift/iOS | Alta | XCFramework + SPM package | App iOS de ejemplo compila y establece sesión |
| Kotlin/Android | Alta | AAR con JNI bindings | App Android de ejemplo compila y establece sesión |
| TypeScript/Browser | Alta | npm package con WASM | Demo en navegador conecta via WebSocket y reproduce audio |
| Node.js | Media | npm package con napi-rs | Script Node establece sesión UDP |

La prioridad "Alta" de los 4 primeros SDKs refleja la necesidad de cubrir servidor (Python), escritorio (Python también, más los binarios nativos), móvil (Swift + Kotlin) y web (WASM) desde el primer release.

### Fase 7 — WebSocket transport y relay (Semana 21-23)

**Objetivo:** Soportar el browser como participante real en sesiones.

| Entregable | Criterio de aceptación |
|------------|----------------------|
| `WebSocketTransport` | Funcional sobre `tokio-tungstenite` |
| Relay server | Un servidor que acepta conexiones WebSocket y UDP, forwarding paquetes entre participantes |
| Transport negotiation en handshake | Dos peers pueden usar transportes diferentes (uno UDP, otro WebSocket) |
| `examples/relay_server.rs` | Relay funcional que conecta un browser con un nativo |
| Browser demo | Página web que se conecta al relay y reproduce audio de un sender nativo |
| Docker image del relay | `docker pull gravital/sound-relay` funcional |

### Fase 8 — Estabilización y release (Semana 24-26)

**Objetivo:** Preparar la versión 0.1.0 pública.

| Entregable | Descripción |
|------------|-------------|
| `cargo doc` completo | Documentación API de todos los crates públicos |
| `CHANGELOG.md` | Cambios desde inception hasta 0.1.0 |
| `CONTRIBUTING.md` | Guía de contribución, estilo, proceso de PR |
| `SECURITY.md` | Política de reporte de vulnerabilidades |
| CI completo | Tests, cross-compile, benchmarks, fuzzing, docs |
| Release binaries | Binarios precompilados para Linux/macOS/Windows en GitHub Releases |
| Crate en crates.io | `gravital-sound-core`, `gravital-sound-transport`, etc. publicados |
| SDK packages | PyPI, npm, Maven Central / GitHub Packages, SPM |
| Landing page | `gravitalsound.dev` con docs, quickstart, benchmarks |

---

## 7. Modelo de CI/CD

### 7.1 Pipeline de CI (en cada push/PR)

```
┌─────────────┐   ┌──────────┐   ┌──────────┐   ┌──────────────┐
│ cargo fmt   │──▶│ clippy   │──▶│  tests   │──▶│ cross-check  │
│ (formatting)│   │ (lints)  │   │ (unit +  │   │ (wasm32,     │
│             │   │          │   │  integ)  │   │  aarch64)    │
└─────────────┘   └──────────┘   └──────────┘   └──────────────┘
```

### 7.2 Pipeline de release (en tag)

```
┌───────────────┐   ┌────────────────┐   ┌──────────────────┐
│ Build matrix  │──▶│ Package SDKs   │──▶│ Publish          │
│ - linux x86   │   │ - python wheel │   │ - crates.io      │
│ - linux arm64 │   │ - npm package  │   │ - pypi           │
│ - macos x86   │   │ - wasm bundle  │   │ - npm            │
│ - macos arm64 │   │ - xcframework  │   │ - GitHub Release │
│ - windows x86 │   │ - android aar  │   │ - Docker Hub     │
│ - wasm32      │   │                │   │                  │
└───────────────┘   └────────────────┘   └──────────────────┘
```

### 7.3 Pipeline de benchmarks (semanal)

Ejecuta benchmarks de `criterion` y compara contra la baseline almacenada. Si algún benchmark regresa más de 10%, el resultado se marca como warning. Si regresa más de 25%, se bloquea el merge. Los resultados se publican como artefacto del workflow y se pueden visualizar en `gravitalsound.dev/benchmarks`.

---

## 8. Naming y convenciones del proyecto

### 8.1 Nombres de crates (Rust)

| Crate | Nombre en Cargo.toml | Descripción |
|-------|---------------------|-------------|
| Core | `gravital-sound-core` | Tipos, serialización, estados. `no_std`. |
| Transport | `gravital-sound-transport` | UDP, WebSocket. `std` required. |
| Codec | `gravital-sound-codec` | Opus, PCM, resampling. |
| Metrics | `gravital-sound-metrics` | RTT, jitter, loss, MOS. |
| I/O | `gravital-sound-io` | Audio hardware via cpal. |
| FFI | `gravital-sound-ffi` | C ABI exports. |
| CLI | `gravital-sound-cli` | Binary: `gs` |
| Facade | `gravital-sound` | Re-export de todos los crates para uso directo en Rust |

### 8.2 Prefijo del CLI

El binario se llama `gs` (Gravital Sound). Subcommands:

```
gs send     --host 192.168.1.50 --port 9000 --input mic
gs receive  --bind 0.0.0.0 --port 9000 --output speaker
gs bench    --mode loopback --duration 30s
gs info     --host 192.168.1.50 --port 9000
gs doctor                                          # verifica audio devices, network, deps
gs relay    --bind 0.0.0.0 --port 9000             # inicia relay server
```

### 8.3 Prefijo de la API C

Todas las funciones: `gs_*`. Todos los tipos: `Gs*`. Todas las constantes: `GS_*`.

### 8.4 Nombres de paquetes por ecosistema

| Ecosistema | Nombre del paquete |
|------------|-------------------|
| crates.io | `gravital-sound` |
| PyPI | `gravital-sound` |
| npm (Node.js nativo) | `@gravital/sound` |
| npm (WASM/browser) | `@gravital/sound-web` |
| Maven / Gradle | `dev.gravital:sound` |
| Swift Package Manager | `https://github.com/gravital/gravital-sound-swift` |
| Docker Hub | `gravital/sound-relay` |

---

## 9. Criterios de éxito

El proyecto se evalúa contra criterios concretos y medibles:

**Corrección.** La especificación del protocolo es suficientemente precisa para que un implementador independiente pueda construir un emisor o receptor compatible sin consultar el código fuente.

**Rendimiento.** Latencia de encode + transmisión + decode en loopback localhost inferior a 1ms (excluyendo jitter buffer). Latencia total en red LAN con jitter buffer de 20ms inferior a 25ms. Documentado con benchmarks reproducibles.

**Portabilidad verificada.** La misma sesión de audio funciona con un participante en Linux (nativo), otro en Android (SDK Kotlin), otro en iOS (SDK Swift) y un cuarto en el navegador (WASM + WebSocket), sin incompatibilidades de protocolo. Esto se valida con un test de integración multi-plataforma en CI.

**Robustez.** La implementación maneja gracefully pérdida de paquetes del 5%, jitter de ±10ms, y reordenamiento del 2% sin crash, panic, ni corrupción de estado. Validado con tests automatizados usando simulación de red adversa.

**Usabilidad por plataforma.** Un desarrollador puede integrar el SDK en una app existente (iOS, Android, web o servidor) y establecer una sesión funcional en menos de 30 minutos usando la documentación y los ejemplos.

**Tamaño de binario razonable.** La biblioteca compilada (sin debug symbols) pesa menos de 5MB en plataformas nativas y menos de 2MB como WASM. Esto se monitorea en CI.

---

## 10. Riesgos y mitigaciones

| Riesgo | Impacto | Mitigación |
|--------|---------|------------|
| Complejidad de cross-compilation | Alto | Usar `cross-rs` (toolchains en Docker) para todos los targets Linux/Android. Para iOS, scripts dedicados con Xcode toolchain. Para WASM, `wasm-pack`. Cada target tiene un script en `scripts/` que encapsula el proceso. |
| Mantenimiento de 5+ SDKs | Alto | Los SDKs son wrappers delgados sobre la misma FFI. La lógica vive en Rust — los SDKs solo traducen tipos y convenciones idiomáticas. Un cambio en el core requiere actualizar el header C (automático) y ajustar wrappers (manual pero mecánico). |
| Latencia de audio en iOS/Android | Medio | Ambas plataformas tienen paths de baja latencia (AAudio en Android, CoreAudio con AudioUnit en iOS) pero requieren configuración correcta. El crate `cpal` maneja esto, pero se necesitan tests en dispositivos reales, no solo emuladores. |
| WebSocket como transporte para browser añade latencia | Medio | WebSocket opera sobre TCP, lo que puede añadir latencia por retransmisión. Mitigación: WebTransport (UDP-like en browser, sobre QUIC) se añade como transport alternativo cuando la adopción de navegadores lo permita. Mientras tanto, el jitter buffer del browser se configura más agresivamente. |
| `libopus` como dependencia C complica el build en algunas plataformas | Medio | Usar feature `vendored` del crate `opus` que compila libopus desde source. Para WASM, Opus tiene un port a WASM validado. Alternativa: evaluar `opus-rs` con implementación Rust pura cuando exista una opción madura. |
| Scope creep: intentar soportar todas las plataformas desde el día 1 | Alto | Las fases 1-5 se centran exclusivamente en Rust nativo (Linux/macOS). Los SDKs de plataforma son fase 6. iOS y Android no bloquean el desarrollo del core. |
| NAT traversal en UDP | Medio | La v0.1 asume conectividad directa o relay explícito. El relay server (fase 7) resuelve el caso de browser y de peers detrás de NAT. STUN/TURN nativo se evalúa como extensión posterior. |

---

## 11. Relación con el ecosistema Gravital

Gravital Sound es una división autónoma con su propio repositorio, release cycle y versionado. Su relación con otras divisiones de Gravital se define a través de puntos de integración opcionales, nunca de dependencias obligatorias.

**Gravital ID.** Un participante de sesión puede autenticarse con su Gravital ID durante el handshake extendido (fase de seguridad). Esto no es requerido para sesiones anónimas o entre peers que se autentican por otros medios. La integración se implementa como una extensión del handshake, usando el rango de tipos de mensaje `0x40-0x7F` reservado para extensiones de aplicación.

**Gravital Cloud.** Las métricas de sesión (RTT, loss, MOS) pueden enviarse a Gravital Cloud para análisis histórico y dashboards. El relay server puede desplegarse como servicio dentro de la infraestructura de Gravital Cloud. Ninguno de estos es un requisito para que el protocolo funcione — son capas de valor añadido para usuarios que ya están en el ecosistema Gravital.

**Gravital Security / Quimera.** Los endpoints que exponen servicios de Gravital Sound (especialmente el relay server) pueden evaluarse con Quimera como parte de auditorías de seguridad.

---

## 12. Próximos pasos inmediatos

El trabajo comienza con la **Fase 0** y la **Fase 1** en paralelo:

**Inmediato (esta semana):**
1. Crear el repositorio `gravital-sound` con la estructura de workspace.
2. Configurar `Cargo.toml` raíz con todos los crates (vacíos, solo `lib.rs` con `// TODO`).
3. Configurar CI básico en GitHub Actions (fmt, clippy, test).
4. Validar cross-compilation a `aarch64-unknown-linux-gnu` y `wasm32-unknown-unknown`.

**Semana siguiente:**
5. Redactar `docs/protocol-spec.md` con la especificación formal del protocolo.
6. Redactar `docs/packet-format.md` con diagramas de bits y ejemplos hex.
7. Redactar ADR-001 (Rust + C FFI como estrategia de portabilidad).

**Semana 3:**
8. Comenzar implementación de `gravital-sound-core` con `Packet` y `MessageType`.
9. Primeros tests unitarios y fuzzing setup.
