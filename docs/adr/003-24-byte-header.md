# ADR-003 · Header de 24 bytes

**Estado:** Aceptado (2026-04)

## Contexto

El header se lee/escribe en cada paquete. Tamaño y alineación afectan tanto el throughput (bytes útiles / bytes totales) como la latencia de parsing. Evaluamos headers de 12, 16, 20, 24 y 32 bytes.

## Decisión

Header de **24 bytes**:

| Offset | Tamaño | Campo          |
|--------|--------|----------------|
| 0      | 2      | `magic`        |
| 2      | 1      | `version`      |
| 3      | 1      | `flags`        |
| 4      | 1      | `msg_type`     |
| 5      | 3      | `_reserved0` (padding) |
| 8      | 4      | `session_id`   |
| 12     | 4      | `sequence`     |
| 16     | 8      | `timestamp` (u64) |
| 24     | —      | (fin del header base; `payload_len` y `checksum` van al final físico del paquete) |

## Rationale

- **24 bytes** permite alinear `timestamp` (u64) a offset 16, natural en x86_64 y aarch64. Un acceso `u64::from_be_bytes` es un solo `bswap` sin unaligned load.
- El overhead relativo es **24 / (24 + payload)**. Con payload de 960 muestras × 2 bytes (mono 20 ms PCM) = 1920 bytes, overhead = 1.2%. Aceptable.
- Header más pequeño (12-16 bytes) obligaba a timestamps más cortos (u32 en ms, wraparound en ~50 días) o a colocar `session_id` y `sequence` desalineados.
- Header más grande (32 bytes) añadía campos especulativos (MAC field, KID) prematuramente. La capa cripto (v0.4) los insertará en su propio sub-header.

## Alternativas

### A. Header de 16 bytes (estilo compacto)
- ✅ Menor overhead.
- ❌ `timestamp` u32 en ms → wraparound ~50 días, imposible con sesiones largas.
- ❌ Menor alineación natural.
- **Rechazada.**

### B. Header de 20 bytes (estilo RTP)
- ✅ Compatibilidad mental con RTP.
- ❌ No alinea `timestamp`.
- ❌ No hay espacio para `checksum` dedicado (RTP confía en UDP; no aceptable para WebSocket o transportes futuros sin CRC nativo).
- **Rechazada.**

### C. Header de 32 bytes (con MAC/KID)
- ✅ Espacio para cripto desde v0.1.
- ❌ +33% de overhead inmediato sin beneficio en 0.1.
- ❌ Cambios tardíos del MAC format obligarían a migrar igual.
- **Rechazada** — la capa cripto irá en un sub-header opcional.

## Consecuencias

- `PacketHeader` tiene `#[repr(C)]` con padding explícito. `size_of::<PacketHeader>() == 24` verificado con `const assert`.
- `payload_len` y `checksum` **no** están en el header — viven al final del paquete. Esto obliga a parsing en dos pasos pero permite calcular el CRC en un solo sweep.
- El campo `_reserved0` (3 bytes) puede usarse en v0.2 para una extensión menor sin romper wire format (cambia sólo semántica, no layout).

## Referencias

- [`packet-format.md`](../packet-format.md)
