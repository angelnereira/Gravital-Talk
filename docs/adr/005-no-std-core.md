# ADR-005 · Núcleo `no_std`

**Estado:** Aceptado (2026-04)

## Contexto

El protocolo debe correr en entornos heterogéneos: servidores (std completo), móvil (std), navegador (std limitado en WASM), embebido (Linux musl, o `no_std` puro con RTOS). Forzar dependencia de la stdlib cierra la puerta a embebido y añade runtime innecesario en WASM.

## Decisión

`gravital-sound-core` es **`#![no_std]`** por default, con feature `alloc` activada por default y feature `std` opt-in:

- **Sin `std`:** el crate sólo usa `core`. Error types implementan `core::fmt::Display`, no `std::error::Error`.
- **Con `alloc`** (default): permite `Box`, `Vec`, `BTreeMap` internamente.
- **Con `std`** (opt-in): añade `impl std::error::Error for Error`.

Los crates dependientes (`transport`, `metrics`, `ffi`, `cli`) sí usan `std` libremente. La restricción es **exclusivamente en core**.

## Implicaciones técnicas

- No `std::time::Instant` en el core. Los timestamps se pasan como `u64` (microsegundos) desde fuera.
- No `std::net::SocketAddr` en el core. Tipos de red viven en `transport`.
- No `std::io::Read/Write`. El core opera sobre `&[u8]` / `&mut [u8]` directamente.
- No `println!`/`eprintln!`. El logging se emite vía `tracing` en crates superiores.
- No `Mutex`/`RwLock` en el core. El core es **thread-exclusive por sesión**; sincronización (si se necesita) vive en el transport/ffi.

## Alternativas

### A. `std` obligatorio
- ✅ API más rica (`Read`/`Write`, `Instant`).
- ❌ Bloquea embebido.
- ❌ En WASM, `std::time::Instant` tiene coste extra (llamada a `performance.now`).
- **Rechazada.**

### B. `no_std` puro sin `alloc`
- ✅ Compatible con firmware sin allocator.
- ❌ API torpe (todo sobre buffers del caller, complica fragment reassembly).
- ❌ El segmento de mercado que lo requiere es marginal.
- **Rechazada** — alcanzable bajo demanda con una feature `no-alloc` futura.

## Consecuencias

- `cargo check --target wasm32-unknown-unknown -p gravital-sound-core --no-default-features` debe pasar siempre. Es parte del CI.
- Contribuciones que introduzcan dependencias de `std` en el core se bloquean en el review.
- Los tests del core que necesitan `std` (por ejemplo, para medir tiempo) se gated con `#[cfg(feature = "std")]`.

## Referencias

- [`portability.md`](../portability.md) §3
