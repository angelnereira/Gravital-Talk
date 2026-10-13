# Audio Codecs

Gravital Talk implementa codecs de audio a través del crate `gravital-talk-codec`.

## Arquitectura

```
gravital-talk-codec
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

## Voz: tratamiento y perfiles

Lo que usan las apps no es `OpusCodec` sino `voice` (feature `voice`): Opus y RNNoise en
Rust puro, con una cadena de tratamiento alrededor (`src/voice_dsp.rs`).

```
micrófono → pasa-altos 90 Hz → RNNoise + puerta de voz → nivelador + limitador → Opus → red
red → Opus → realce de presencia (+2,5 dB en 3 kHz) → altavoz
```

- El **nivelador** lleva la voz a −20 dBFS (sube hasta +15 dB, baja hasta −12 dB) y sólo
  aprende el nivel mientras RNNoise dice que hay voz, para no subir el fondo en las pausas.
- RNNoise entrega cada bloque con **10 ms de retraso**. Para mezclar su salida con la señal
  original (reducción «suave») la original se retrasa lo mismo.
- El **realce de presencia** es del receptor: no cuesta nada en la red.

| Perfil | Ruido (mezcla) | Tope | DTX | Datos medidos o estimados |
|---|---|---|---|---|
| `Clear` (por defecto en la app) | 75 %, puerta a 0,25 | 32 kbps | no | 0,55 MB/min (medido) |
| `Call` | 90 % | 16 kbps | no | ~0,45 MB/min |
| `Noisy` | 100 %, puerta a 0,05 | 24 kbps | no | ~0,5 MB/min |
| `Light` | 90 % | 12 kbps | sí | ~0,4 MB/min |
| `Raw` | ninguno | el que se pida | no | — |

Antes de los perfiles la app pedía 128 kbps: ~1,5 MB/min.

Tres cosas medidas que conviene saber antes de tocar esto:

- **Bajar la tasa ahorra poco por debajo de 32 kbps.** Cada paquete lleva 72 B de
  cabeceras y van 50 por segundo, más uno de paridad por cada tres o cuatro: eso son
  ~37 kbps fijos. Para gastar de verdad menos habría que mandar menos paquetes (frames de
  40 ms), y eso es retardo.
- **La copia de respaldo (FEC en banda) no añade bytes**: Opus respeta la tasa pedida y le
  quita esos bits a la voz. Por eso los perfiles sólo la activan cuando la red pierde
  paquetes (se revisa cada segundo).
- **DTX ahorra un 4–6 %** con voz normal en PTT, y a cambio el otro extremo oye silencio
  absoluto en las pausas. Sólo lo lleva `Light`. No rompe nada en el receptor: la secuencia
  de audio sólo avanza al enviar, así que un frame no enviado no parece una pérdida.

`cargo run --release -p gravital-talk-codec --no-default-features --features voice
--example voice_lab -- voz.wav salida/` pasa una grabación (48 kHz, 16 bits) por cada
perfil y deja un WAV de cada uno para oírlos, con su tabla de datos y niveles.

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

`gravital_talk_codec::negotiation::negotiate(preferred)` valida que el codec pedido esté en el conjunto soportado. En la v0.1 no hay negociación wire — ambos peers deben configurar el mismo codec manualmente. La negociación automática en el handshake está planificada para Track B.

## Añadir un codec personalizado

Implementar los traits `Encoder` y `Decoder` (ambos `Send`, sin `Sync`) y registrar un `CodecId::Other(u8)` personalizado:

```rust
struct MyCodec { /* ... */ }
impl Encoder for MyCodec { /* ... */ }
impl Decoder for MyCodec { /* ... */ }

let my_id = CodecId::Other(0x10);
let session = CodecSession::new_with_pair(transport, config, Box::new(enc), Box::new(dec))?;
```
