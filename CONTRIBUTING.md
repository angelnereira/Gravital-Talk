# Contribuir a Gravital Sound

Gracias por tu interés. Gravital Sound es un protocolo de infraestructura. Todo cambio debe preservar tres propiedades no negociables: **correctitud**, **rendimiento**, **portabilidad**. Antes de abrir un PR, verifica que tu cambio cumple los tres.

## Antes de enviar un PR

1. El CI debe pasar localmente:
   ```bash
   make fmt-check
   make clippy
   make test
   make cross-wasm
   ```
2. Si tocaste el *hot path* del core o del transporte, corre `make bench` y compara contra la baseline. Regresiones ≥ 10% requieren justificación escrita en el PR.
3. Si introduces `unsafe`, incluye un comentario `// SAFETY:` explicando las invariantes y por qué se mantienen.
4. Documenta los cambios visibles al usuario en `CHANGELOG.md` bajo la sección `[Unreleased]`.

## Estilo

- Rust 2021 edition, formateado con `rustfmt` (config default).
- Clippy limpio con `-D warnings -W clippy::perf -W clippy::nursery`.
- Mensajes de commit en formato [Conventional Commits](https://www.conventionalcommits.org/): `tipo(scope): descripción`. Ejemplos:
  - `feat(transport): añadir backoff exponencial a handshake`
  - `fix(core): corregir off-by-one en fragment reassembly`
  - `perf(checksum): vectorizar CRC-16 con SSE4.2`
  - `docs(spec): clarificar semántica de HEARTBEAT`
- Sin emojis en commits, docs o código a menos que la feature los requiera.

## Diseño

- **`gravital-sound-core` es `no_std`.** Cualquier dependencia de `std` debe vivir en otro crate. El core sólo puede usar `core` y `alloc` (bajo feature `alloc`).
- **Cero allocs en el hot path.** Si tu cambio añade un `Box::new`, `Vec::push`, `String::from`, etc. dentro de un ciclo de encode/decode o send/recv, justifícalo o búscale un workaround con `bytes`/`smallvec`/pool pre-alocado.
- **Transiciones de estado explícitas.** La máquina de estados de sesión usa tipos marcador. Añadir un estado implica añadir una transición type-safe, no un `if` en runtime.
- **La FFI es API pública estable.** Un cambio en la firma de una función `gs_*` es un *breaking change* y requiere versión mayor.

## Testing

- Los cambios en el core deben venir con property tests (`proptest`) que verifiquen roundtrip serialize/deserialize.
- Los cambios en el transporte deben venir con tests de integración que corran dos sockets en localhost.
- Los cambios en la FFI deben venir con una extensión del smoke test C (`crates/gravital-sound-ffi/tests/c_smoke.c`).

## Seguridad

Reporta vulnerabilidades en privado siguiendo la política de [`SECURITY.md`](SECURITY.md). No abras issues públicas con detalles de exploits.

## Licencia

Al contribuir, aceptas que tu contribución se distribuye bajo la doble licencia del proyecto (MIT + Apache-2.0).
