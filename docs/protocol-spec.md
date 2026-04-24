# Gravital Sound Protocol — Especificación v0.1 (draft)

**Estado:** draft. Esta especificación puede cambiar de forma incompatible hasta la versión 0.1.0 final.

## 1. Convenciones

- Toda la numeración es en base hexadecimal salvo indicación contraria.
- Los enteros multibyte se codifican en **big-endian** (orden de red).
- Los campos reservados deben escribirse como `0x00` y los receptores deben ignorarlos.
- `MTU` referencia la unidad máxima de transmisión del transporte subyacente. El valor efectivo usado por el protocolo es `1200 bytes` por defecto, negociable durante el handshake.

## 2. Capas

| Capa | Responsabilidad | Crate |
|------|-----------------|-------|
| Aplicación | Captura/playback de audio, UI. | fuera del protocolo |
| Codec | PCM, Opus (futuro). | `gravital-sound-codec` |
| **Sesión** | Handshake, estados, heartbeat, métricas. | `gravital-sound-core` + `transport` |
| **Paquete** | Framing, header, checksum, fragmentación. | `gravital-sound-core` |
| Transporte | UDP, WebSocket, WebTransport. | `gravital-sound-transport` |

## 3. Paquete base

Cada datagrama UDP (o frame WebSocket binario) contiene exactamente un `Packet`:

```
┌────────────────────┐
│  Header (24 bytes) │
├────────────────────┤
│  Payload (N bytes) │
└────────────────────┘
```

`N` ≤ `MAX_PAYLOAD_SIZE` = `1176 bytes` por defecto (MTU 1200 − header 24).

Ver [`packet-format.md`](packet-format.md) para el diagrama de bits completo.

## 4. Tipos de mensaje

| Código | Nombre              | Dirección       | Descripción                                         |
|--------|---------------------|-----------------|-----------------------------------------------------|
| `0x01` | `HANDSHAKE_INIT`    | C → S           | Inicia sesión. Anuncia capacidades.                 |
| `0x02` | `HANDSHAKE_ACCEPT`  | S → C           | Acepta sesión. Asigna `session_id`.                 |
| `0x03` | `HANDSHAKE_CONFIRM` | C → S           | Confirma parámetros, entra a `Active`.              |
| `0x10` | `AUDIO_FRAME`       | bidireccional   | Un frame de audio codificado.                       |
| `0x11` | `AUDIO_FRAGMENT`    | bidireccional   | Fragmento de un frame grande.                       |
| `0x20` | `HEARTBEAT`         | bidireccional   | Ping. Usa `timestamp` para calcular RTT.            |
| `0x21` | `HEARTBEAT_ACK`     | bidireccional   | Pong. Eco del timestamp original.                   |
| `0x30` | `CONTROL_PAUSE`     | bidireccional   | Pausa envío de audio sin cerrar sesión.             |
| `0x31` | `CONTROL_RESUME`    | bidireccional   | Reanuda sesión pausada.                             |
| `0x32` | `CONTROL_METRICS`   | bidireccional   | Envía métricas del emisor al receptor.              |
| `0x40`..`0x7F` | **Extensiones de aplicación.** | — | Reservado para integraciones Gravital ID/Cloud. |
| `0xFE` | `ERROR`             | bidireccional   | Notifica un error fatal con código.                 |
| `0xFF` | `CLOSE`             | bidireccional   | Finaliza sesión grácilmente.                        |

Todo valor no listado debe ser rechazado con `ERROR` código `UNKNOWN_MESSAGE_TYPE`.

## 5. Flags del header

Campo `flags` del header (1 byte):

| Bit | Símbolo        | Significado                                         |
|-----|----------------|-----------------------------------------------------|
| 7   | `FRAGMENTED`   | El payload es parte de un frame fragmentado.        |
| 6   | `LAST_FRAGMENT`| Último fragmento del frame actual.                  |
| 5   | `ENCRYPTED`    | Payload cifrado (reservado, no usado en 0.1).       |
| 4   | `RETRANSMIT`   | Retransmisión de un paquete perdido (opcional).     |
| 3-0 | Reservado      | Debe ser 0.                                         |

## 6. Handshake (3-way)

```
Cliente                                 Servidor
   │                                        │
   │─── HANDSHAKE_INIT (proposed caps) ────▶│
   │                                        │
   │◀── HANDSHAKE_ACCEPT (session_id, caps)─│
   │                                        │
   │─── HANDSHAKE_CONFIRM ─────────────────▶│
   │                                        │
   │══════════ Active ════════════════════ │
```

**Payload de `HANDSHAKE_INIT`** (20 bytes):
- `protocol_version`  — 1 byte (valor fijo `0x01` en v0.1).
- `codec_preferred`   — 1 byte (`0x00` = auto, `0x01` = Opus, `0x02` = PCM).
- `sample_rate`       — 4 bytes u32 (Hz).
- `channels`          — 1 byte (1 o 2).
- `frame_duration_ms` — 1 byte (valores válidos: 5, 10, 20, 40, 60).
- `max_bitrate`       — 4 bytes u32 (bps, 0 = negociable).
- `capability_flags`  — 4 bytes u32 (bitfield de features opcionales).
- `nonce`             — 4 bytes aleatorios (descartar duplicados recientes).

**Payload de `HANDSHAKE_ACCEPT`** (24 bytes):
- Mismos campos que `INIT` más:
- `session_id`        — 4 bytes u32 asignado por el servidor.

**Payload de `HANDSHAKE_CONFIRM`** (4 bytes):
- `session_id` — echo del asignado.

Si el `session_id` no coincide en `CONFIRM`, el servidor descarta el paquete silenciosamente.

### Timeouts

- El cliente reintenta `HANDSHAKE_INIT` con backoff exponencial (inicio 200 ms, factor 2, jitter ±25%, máximo 5 intentos, timeout total 10 s).
- El servidor olvida un `INIT` sin `CONFIRM` después de 5 s.

## 7. Sesión activa

- Cada extremo mantiene un `sequence` monotónicamente creciente (`u32`, envuelve cada 2³²).
- `timestamp` es microsegundos desde la creación de la sesión (`u64`).
- Un `HEARTBEAT` se envía cada 1 s si no ha habido otro tráfico.
- Si no se recibe nada durante 10 s, la sesión se cierra con `ERROR` código `PEER_TIMEOUT`.

## 8. Fragmentación

Cuando `payload_len > MAX_PAYLOAD_SIZE`:

1. El emisor divide el frame en fragmentos de ≤ `MAX_PAYLOAD_SIZE − 4` bytes.
2. Cada fragmento se envía como `AUDIO_FRAGMENT` con los primeros 4 bytes del payload siendo:
   - `fragment_index` — 2 bytes (0-based).
   - `total_fragments` — 2 bytes.
3. El flag `FRAGMENTED` está presente en todos los fragmentos; `LAST_FRAGMENT` sólo en el último.
4. El receptor reensambla en buffer indexado por `sequence` del primer fragmento y un offset.

Frames que exceden `MAX_PAYLOAD_SIZE × 16` se descartan.

## 9. Checksum

Cada paquete incluye un CRC-16/CCITT-FALSE (polinomio `0x1021`, init `0xFFFF`, no XOR final) calculado sobre el header (con `checksum = 0`) y el payload. Paquetes con checksum incorrecto se descartan silenciosamente y se incrementa el contador de errores de integridad.

## 10. Métricas

La sesión expone las siguientes métricas como contadores atómicos (`u64` monotónicos) y derivados (`f32` calculados):

- `packets_sent`, `packets_received`, `bytes_sent`, `bytes_received` (contadores).
- `rtt_ms` (EWMA, α = 0.125).
- `jitter_ms` (estimador RFC 3550).
- `loss_percent` (bitmap de ventana de 64 paquetes).
- `reorder_percent` (fracción de `sequence` recibidos fuera de orden).
- `buffer_fill_percent` (ocupación del jitter buffer).
- `estimated_mos` (MOS-LQ derivado de RTT + loss + jitter).

Un `CONTROL_METRICS` opcional envía el snapshot del emisor al receptor con periodicidad configurable (default 5 s).

## 11. Cierre

- Cualquier extremo envía `CLOSE`. El otro responde con `CLOSE` y ambos transicionan a `Closed`.
- Si no hay respuesta en 500 ms, el iniciador pasa a `Closed` unilateralmente.

## 12. Errores

Códigos de `ERROR` (1 byte en el primer byte del payload):

| Código | Símbolo                 | Significado                              |
|--------|-------------------------|------------------------------------------|
| `0x01` | `UNKNOWN_MESSAGE_TYPE`  | Tipo de mensaje no reconocido.           |
| `0x02` | `INVALID_CHECKSUM`      | CRC incorrecto (típicamente silencioso). |
| `0x03` | `INVALID_STATE`         | Mensaje fuera de secuencia.              |
| `0x04` | `UNSUPPORTED_CODEC`     | Codec pedido no soportado.               |
| `0x05` | `PROTOCOL_MISMATCH`     | Versión del protocolo incompatible.      |
| `0x06` | `PEER_TIMEOUT`          | Heartbeat perdido.                       |
| `0x07` | `RESOURCE_EXHAUSTED`    | Sin capacidad (memoria, sockets).        |
| `0xFF` | `INTERNAL`              | Error no especificado.                   |

## 13. Extensiones reservadas

Los tipos `0x40`..`0x7F` están reservados para extensiones de aplicación:

- `0x40` `GRAVITAL_ID_AUTH` — token JWT emitido por Gravital ID.
- `0x41` `GRAVITAL_CLOUD_METRICS` — push de métricas al backend de Gravital Cloud.
- `0x42`..`0x4F` — reservados Gravital.
- `0x50`..`0x7F` — libres para uso del integrador.

## 14. Máquina de estados

Ver [`session-model.md`](session-model.md).

## 15. Ejemplos binarios

Ver [`packet-format.md`](packet-format.md) §4.
