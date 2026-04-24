# ADR-001 · Rust core con C FFI como estrategia de portabilidad

**Estado:** Aceptado (2026-04)
**Autor:** Angel Nereira

## Contexto

El protocolo debe correr en servidores Linux, escritorio (macOS/Windows), móvil (iOS/Android), navegador (WASM) y embebido. Cada plataforma tiene un ecosistema de lenguajes preferidos (Swift en iOS, Kotlin en Android, TypeScript en web, Python en servidor/scripting). Implementar el protocolo en cada lenguaje multiplicaría la superficie de bugs y violaría la premisa de "una especificación, una implementación".

## Decisión

El protocolo se implementa **una sola vez en Rust**. La portabilidad se logra mediante dos mecanismos:

1. **Capa FFI C estable** (`gravital-sound-ffi`). Expone el núcleo como funciones `extern "C"`. El header `gravital_sound.h` se genera con `cbindgen`. Cualquier lenguaje que soporte FFI C (prácticamente todos) consume este header.
2. **SDKs idiomáticos** por lenguaje. Wrappers delgados que traducen la API C a tipos y convenciones nativas. Los SDKs no reimplementan lógica del protocolo.

## Alternativas consideradas

### A. Reimplementar el protocolo en cada lenguaje
- ✅ API nativa perfecta en cada plataforma.
- ❌ N implementaciones = N bugs, N especificaciones de facto, N auditorías.
- ❌ Drift entre implementaciones inevitable.
- **Rechazada.**

### B. Go como lenguaje principal
- ✅ Fácil cross-compilation.
- ❌ Runtime pesado (GC, goroutines) inapropiado para real-time de baja latencia.
- ❌ FFI penosa (cgo overhead, bindings frágiles).
- ❌ No `no_std` equivalente → no usable en embebido ni WASM ligero.
- **Rechazada.**

### C. C/C++ como lenguaje principal
- ✅ FFI trivial (no hay capa).
- ❌ Memoria manual → clase entera de bugs que Rust previene por compilación.
- ❌ Tooling (build system, paquetes, testing) inferior.
- **Rechazada.**

## Consecuencias

**Positivas:**
- Un único código que mantener para la lógica del protocolo.
- `cargo` como build system unificado (todos los crates + SDKs Python/WASM viven en el mismo workspace).
- Seguridad de memoria gratis en el core.
- `no_std` compat permite embebido y WASM sin recompilaciones.

**Negativas:**
- La capa FFI introduce una superficie donde `unsafe` es inevitable. Se mitiga con auditoría específica de ese crate.
- Cada cambio en la API pública del core obliga a actualizar el header C y, mecánicamente, cada SDK. `cbindgen` automatiza el 80% del trabajo.
- El bindgen de Python (PyO3) y Web (wasm-bindgen) no pasa por el header C — usan directamente los tipos Rust. Es una excepción al modelo, aceptada por el beneficio de integración más natural con esos ecosistemas.

## Referencias

- [`portability.md`](../portability.md)
- [`seed.md`](../../seed.md) §2 y §5
