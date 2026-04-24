# Audio Codecs

Gravital Sound implementa codecs de audio a través del crate `gravital-sound-codec`.

## Arquitectura

```
gravital-sound-codec
├── Encoder trait  (encode &[i16] → &[u8])
├── Decoder trait  (decode &[u8] → &[i16])
├── CodecId enum   (Pcm = 0x01, Opus = 0x02)
├── pcm.rs         — passthrough PCM i16-LE
└── opus.rs        — wrapper sobre libopus vía audiopus
```

El `CodecSession` del crate facade combina un `Session` de transporte (bytes) con un par encoder/decoder, exponiendo las APIs de alto nivel `send_samples(&[i16])` y `recv_samples() -> Vec<i16>`.

## PCM (0x01)

Codec passthrough. Los muestras i16 se serializan como little-endian sin transformación. Frame size exacto: `sample_rate × frame_duration_ms / 1000` muestras por canal.

**Uso**: desarrollo, debugging, redes de alta calidad donde el ancho de banda no es limitante.

**Bitrate a 48 kHz mono, 10 ms**: 960 bytes/frame → 768 kbps.

## Opus (0x02)

Wrapper sobre libopus 1.x vía `audiopus = "0.2"`. Configuración por defecto:

| Parámetro | Valor | Razón |
|-----------|-------|-------|
| Application | `Voip` | Optimizado para voz conversacional |
| Bitrate | 64 kbps | Buena calidad con latencia baja |
| FEC | habilitado | Recuperación ante pérdida de 1 paquete |
| Packet loss hint | 5 % | Calibra el FEC |
| Sample rates | 8/12/16/24/48 kHz | Soporte nativo de libopus |
| Channels | 1–2 | Mono/estéreo |

**Bitrate efectivo a 64 kbps mono, 10 ms**: ~80 bytes/frame → 10:1 compresión vs PCM.

### Latencia algorítmica

Opus introduce ~26.5 ms de latencia algorítmica a 48 kHz (1272 muestras de lookahead). Esto debe sumarse al jitter buffer y al RTT de red para calcular la latencia total percibida.

### Rangos de bitrate recomendados

| Uso | Bitrate |
|-----|---------|
| Voz VoIP ultra-low | 8–16 kbps |
| Voz VoIP normal | 32–64 kbps |
| Voz alta calidad | 64–96 kbps |
| Música mono | 96–160 kbps |
| Música estéreo | 128–192 kbps |

## Negociación de codec

`gravital_sound_codec::negotiation::negotiate(preferred)` valida que el codec pedido esté en el conjunto soportado. En la v0.1 no hay negociación wire — ambos peers deben configurar el mismo codec manualmente. La negociación automática en el handshake está planificada para Track B.

## Añadir un codec personalizado

Implementar los traits `Encoder` y `Decoder` (ambos `Send`, sin `Sync`) y registrar un `CodecId::Other(u8)` personalizado:

```rust
struct MyCodec { /* ... */ }
impl Encoder for MyCodec { /* ... */ }
impl Decoder for MyCodec { /* ... */ }

let my_id = CodecId::Other(0x10);
let session = CodecSession::new_with_pair(transport, config, Box::new(enc), Box::new(dec))?;
```
