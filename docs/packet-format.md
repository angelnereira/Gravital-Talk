# Formato binario del paquete

## 1. Layout del header (24 bytes)

Big-endian, sin padding.

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|            magic              | version  |  flags  | msg_type |  ← 8
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                         session_id                            |  ← 12
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                          sequence                             |  ← 16
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                                                               |  ← 20
+                         timestamp (u64)                       +
|                                                               |  ← 24
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|         payload_len         |          checksum (CRC-16)      |  ← 28 (FIN header)
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           payload                             |
+                            (N bytes)                          +
|                              ...                              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

Nota: el diagrama muestra 28 bytes porque se incluyen `payload_len (2)` + `checksum (2)` al final. El header ocupa **24 bytes** en total. Los 4 bytes finales son parte del header, no del payload.

## 2. Campos

| Offset | Tamaño | Nombre         | Tipo    | Descripción                                          |
|--------|--------|----------------|---------|------------------------------------------------------|
| 0      | 2      | `magic`        | u16     | Constante `0x4753` (ASCII `"GS"`).                   |
| 2      | 1      | `version`      | u8      | Versión del protocolo. v0.1 → `0x01`.                |
| 3      | 1      | `flags`        | u8      | Ver §5 de [`protocol-spec.md`](protocol-spec.md).    |
| 4      | 1      | `msg_type`     | u8      | Tipo de mensaje (tabla §4 del spec).                 |
| 5      | 3      | `_reserved0`   | —       | Padding para alinear `session_id` a 4 bytes. = `0`.  |
| 8      | 4      | `session_id`   | u32     | Id de sesión. `0` durante handshake pre-ACCEPT.      |
| 12     | 4      | `sequence`     | u32     | Contador monotónico por dirección.                   |
| 16     | 8      | `timestamp`    | u64     | Microsegundos desde creación de la sesión.           |
| 24     | ...    | (payload)      | bytes   | ...                                                  |
| −4     | 2      | `payload_len`  | u16     | Longitud del payload en bytes.                       |
| −2     | 2      | `checksum`     | u16     | CRC-16/CCITT-FALSE del header (con `checksum=0`) + payload. |

Aclaración: el diseño alternativo que evaluamos colocaba `payload_len` y `checksum` inmediatamente después del flags, pero quitaba alineación natural a `timestamp`. Se prefirió mantener alineación y colocar esos dos campos al final del header físico; esto requiere un parseo en dos pasos (leer 24 bytes → validar → leer payload). El beneficio es que `timestamp` (campo más leído en hot path) queda alineado a 8 bytes y se decodifica con un solo `u64::from_be_bytes` sin unaligned load.

## 3. Invariantes y validación

Al decodificar, el receptor verifica en este orden (errores raros al final vía `#[cold]`):

1. `buf.len() >= 24`. Si no, `Error::TooShort`.
2. `magic == 0x4753`. Si no, `Error::BadMagic`.
3. `version == PROTOCOL_VERSION`. Si no, `Error::ProtocolMismatch`.
4. `payload_len as usize + 24 == buf.len()`. Si no, `Error::LengthMismatch`.
5. `checksum == computed_crc16(header_with_zeroed_checksum || payload)`. Si no, `Error::BadChecksum`.
6. `msg_type` válido. Si no, responder con `ERROR UNKNOWN_MESSAGE_TYPE`.

Los pasos 1-4 son puros sobre el buffer, sin necesidad de computar CRC. Para paquetes de 1200 bytes esto cuesta < 20 ns en x86_64.

## 4. Ejemplos binarios

### 4.1 `HANDSHAKE_INIT` (cliente)

Parámetros: sample_rate = 48000, channels = 2, frame_duration_ms = 20, nonce = `0xDEADBEEF`.

```
Offset  Bytes                                     Campo
------  ----------------------------------------- ----------------
0x00    47 53                                     magic ("GS")
0x02    01                                        version
0x03    00                                        flags
0x04    01                                        msg_type = HANDSHAKE_INIT
0x05    00 00 00                                  _reserved0
0x08    00 00 00 00                               session_id = 0
0x0C    00 00 00 00                               sequence = 0
0x10    00 00 00 00 00 00 00 00                   timestamp = 0
0x18    00 14                                     payload_len = 20
0x1A    ?? ??                                     checksum (calculado)
0x1C    01 02 00 00 BB 80 02 14                   payload: ver_proto, codec_pcm, 48000, ch=2, dur=20
0x24    00 00 FA 00 00 00 00 00                   max_bitrate=64000, caps=0
0x2C    DE AD BE EF                               nonce
```

### 4.2 `AUDIO_FRAME` PCM mono 20 ms a 48 kHz (960 samples, int16)

```
Header (24 bytes)
─────────────────
47 53              magic
01                 version
00                 flags
10                 msg_type = AUDIO_FRAME
00 00 00           _reserved0
12 34 56 78        session_id = 0x12345678
00 00 0A 01        sequence = 2561
00 00 00 00 00 05 F5 E1   timestamp = 100000000 µs
07 80              payload_len = 1920 (960 * 2)
?? ??              checksum

Payload: 1920 bytes PCM little-endian... (nota: el PCM no es network order, es
formato del codec). El protocolo no reinterpreta el payload.
```

Nota sobre fragmentación: si un frame supera 1176 bytes, se envía como `AUDIO_FRAGMENT` (msg_type `0x11`) con un sub-header de 4 bytes al inicio del payload:

```
0x00    00 03           fragment_index = 3
0x02    00 08           total_fragments = 8
0x04    <payload del fragmento>
```

## 5. CRC-16/CCITT-FALSE

Parámetros:

- Polinomio: `0x1021`
- Init: `0xFFFF`
- RefIn: `false`
- RefOut: `false`
- XorOut: `0x0000`
- Check value para `"123456789"`: `0x29B1`

Implementación de referencia (Rust, `no_std`):

```rust
pub fn crc16_ccitt_false(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}
```

En producción se usa una lookup table de 256 entradas + versión SIMD opcional (SSE4.2 `_mm_crc32_u8` con post-procesamiento, o NEON). Ver `crates/gravital-sound-core/src/checksum.rs`.

## 6. Consideraciones de MTU

- **Internet público**: PMTU efectivo de 1280 bytes (mínimo IPv6). Default = 1200 para dejar holgura (header IP 40 + UDP 8 + padding).
- **LAN Ethernet**: 1500 MTU. Se puede subir a `1472 - 24 = 1448` payload si se detecta LAN.
- **Jumbo frames (9000 MTU)**: opt-in vía config. El protocolo soporta hasta 65535 − 24 = 65511 bytes de payload sin fragmentación aplicativa.
- **WebSocket**: no hay MTU en el nivel de framing, pero los relays intermedios pueden fragmentar. Se mantiene 1200 por consistencia.
