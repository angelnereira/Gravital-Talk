# Guía de portabilidad

## 1. Arquitectura de 3 niveles

```
┌─────────────────────────────────────────────────────────┐
│                    SDKs idiomáticos                      │
│  Python · TypeScript/Web  (0.1)                          │
│  Swift · Kotlin · Node.js (roadmap)                      │
└────────────────────────┬────────────────────────────────┘
                         │ llaman funciones C via FFI
                         │ (o PyO3/wasm-bindgen directo)
┌────────────────────────┴────────────────────────────────┐
│              gravital-sound-ffi (C ABI)                  │
│  Interfaz C estable, header generado con cbindgen        │
└────────────────────────┬────────────────────────────────┘
                         │ Rust internals
┌────────────────────────┴────────────────────────────────┐
│     Rust Core (gravital-sound-core · transport · …)      │
│  no_std compatible en el core.                           │
└─────────────────────────────────────────────────────────┘
```

## 2. Matriz de plataformas

| Plataforma | Rust target | Biblioteca | Transporte | SDK 0.1 |
|------------|-------------|------------|------------|---------|
| Linux x86_64 | `x86_64-unknown-linux-gnu` | `.so` + `.a` | UDP | Rust + Python |
| Linux aarch64 | `aarch64-unknown-linux-gnu` | `.so` + `.a` | UDP | Rust + Python |
| Linux musl | `x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl` | `.a` static | UDP | Rust |
| macOS x86_64 | `x86_64-apple-darwin` | `.dylib` + `.a` | UDP | Rust + Python |
| macOS aarch64 | `aarch64-apple-darwin` | `.dylib` + `.a` | UDP | Rust + Python |
| Windows | `x86_64-pc-windows-msvc` | `.dll` + `.lib` | UDP | Rust |
| Android | `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android` | `.so` | UDP | roadmap |
| iOS | `aarch64-apple-ios`, `aarch64-apple-ios-sim` | `.a` static (XCFramework) | UDP | roadmap |
| Browser | `wasm32-unknown-unknown` | `.wasm` + JS glue | WebSocket | Web |
| Embebido Linux | `aarch64-unknown-linux-musl`, `armv7-unknown-linux-musleabihf` | `.a` static | UDP | C via FFI |

## 3. Núcleo `no_std`

`gravital-sound-core` es `#![no_std]` por default. Esto implica:

- Sin `std::error::Error` → usamos `core::fmt::Display` + feature `std` opcional.
- Sin `std::net` → tipos `SocketAddr` pertenecen al crate `transport`.
- Sin `std::time::Instant` → el core recibe timestamps como `u64` externamente.
- Sin `Box`/`Vec` en el API público → sí internamente bajo `alloc`.

Features:

- `std` (opcional): habilita `std::error::Error` para `Error`.
- `alloc` (default): habilita `Box`, `Vec`, `BTreeMap` donde sea necesario.
- `simd-crc` (default en x86_64 con SSE4.2): usa intrínsecos SIMD para CRC.

## 4. Compilación cross

### Linux → Linux aarch64
```bash
cross build --target aarch64-unknown-linux-gnu --release -p gravital-sound-ffi
```

### Linux → WASM (validación no_std)
```bash
cargo check --target wasm32-unknown-unknown -p gravital-sound-core --no-default-features
```

### Linux → Android (roadmap)
```bash
cargo ndk --target aarch64-linux-android --target armv7-linux-androideabi \
  build --release -p gravital-sound-ffi
```

### macOS → iOS (roadmap)
```bash
./scripts/build-ios.sh  # genera XCFramework
```

## 5. Manejo de dependencias de sistema

| Dependencia | Solución |
|-------------|----------|
| `libopus` (roadmap) | Feature `vendored` del crate `opus` compila desde source. |
| ALSA/PulseAudio (roadmap) | Sólo en `gravital-sound-io`, crate opt-in. El core no depende. |
| OpenSSL (WebSocket TLS) | Usamos `rustls` para evitar dependencia de OpenSSL del sistema. |
| `libc` | Directamente vía crate `libc`; WASM no necesita. |

## 6. Notas por plataforma

### 6.1 iOS (roadmap)

Apple rechaza dynamic linking de bibliotecas de terceros. Solución:

- Biblioteca **estática** (`.a`) para `aarch64-apple-ios`.
- Empaquetada como **XCFramework** que contiene `.a` para device y simulator.
- Distribuida via Swift Package Manager o CocoaPods.

### 6.2 Android (roadmap)

- Biblioteca **`.so`** para cada ABI objetivo: `arm64-v8a`, `armeabi-v7a`, `x86_64`.
- Empaquetada como **AAR** con bindings JNI.
- Distribuida vía Maven Central o GitHub Packages.

### 6.3 Browser (v0.1)

- Compilado a **`.wasm`** con `wasm-bindgen`.
- Transporte limitado a WebSocket (UDP no disponible).
- Captura/reproducción de audio vía `AudioWorklet` (JS-side, no WASM).
- Distribuido como paquete npm `@gravital/sound-web`.

### 6.4 Servidores sin audio I/O

En un relay o mezclador, no hay mic/speaker. El core nunca toca hardware de audio — `gravital-sound-io` es opt-in. Los binarios de servidor no incluyen la dependencia, reduciendo tamaño y superficie.

## 7. Tamaño de binario

Targets 0.1:

| Artefacto | Target | Tamaño objetivo |
|-----------|--------|-----------------|
| `libgravital_sound.so` (FFI dyn) | Linux x86_64 | < 2 MB |
| `libgravital_sound.a` (FFI static) | Linux aarch64 musl | < 5 MB |
| `gravital_sound.wasm` (core + ffi, stripped) | wasm32 | < 1.5 MB |
| `gs` (CLI release) | Linux x86_64 | < 4 MB |

Técnicas aplicadas: `lto = "fat"`, `codegen-units = 1`, `strip = "symbols"`, `panic = "abort"`, `-Z build-std` opcional.

## 8. Pruebas cross-platform

El test `tests/cross_platform.rs` (roadmap) corre el mismo protocolo con un cliente nativo Rust y un cliente Python (via FFI), garantizando que los bytes en el wire son idénticos.
