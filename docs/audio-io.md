# Audio I/O

El crate `gravital-sound-io` proporciona captura y playback de audio usando `cpal 0.15` (ALSA en Linux, CoreAudio en macOS, WASAPI en Windows).

## Arquitectura

```
AudioCapture                          AudioPlayback
  │                                       │
  │ cpal stream callback                  │ cpal stream callback
  │   → conv F32/U16 to i16             │   ← VecDeque<i16>
  │   → accum hasta frame_samples        │
  │   → mpsc::Sender<Vec<i16>>           │ pump thread
  │                                       │   ← mpsc::Receiver<Vec<i16>>
  ▼                                       ▼
mpsc::Receiver<Vec<i16>>           Sender<Vec<i16>>
        │                                 ▲
        └──── CodecSession.send ─────────┘
              CodecSession.recv ──────────►
```

## Diseño de desacoplamiento

El callback de cpal corre en un thread de tiempo real con restricciones de latencia. Para evitar bloqueos, el callback **solo** hace:
1. Conversión de formato (F32/I16/U16 → i16).
2. Acumulación en buffer local hasta `frame_samples`.
3. `tx.send(frame)` — no bloquea porque el canal std::mpsc es ilimitado.

El lado consumidor (encoder / CodecSession) corre en el runtime Tokio sin interacción directa con el thread RT.

## AudioCapture

```rust
let stream_cfg = StreamConfig { sample_rate: 48_000, channels: 1, frame_duration_ms: 10 };
let (_capture, rx) = AudioCapture::start(stream_cfg, Some("default"))?;

// rx entrega Vec<i16> de longitud = samples_per_frame()
while let Ok(samples) = rx.recv() {
    codec_session.send_samples(&samples).await?;
}
```

`_capture` debe mantenerse en scope — su Drop para el stream de cpal.

## AudioPlayback

```rust
let pb = AudioPlayback::start(stream_cfg, Some("default"))?;
let tx = pb.sender(); // clona el Sender para enviar desde múltiples tasks

while let Ok(samples) = codec_session.recv_samples().await {
    pb.push(samples)?; // non-blocking
}
```

Un "pump thread" interno drena el `Receiver<Vec<i16>>` y acumula en un `VecDeque<i16>` compartido con el callback de cpal.

## Selección de device

Pasar `None` como `device_hint` selecciona el device por defecto del sistema. Pasar `"default"` equivale a `None`. Pasar el nombre exacto de un device (obtenido con `list_input_devices()`) selecciona ese device.

```rust
for d in gravital_sound_io::list_input_devices()? {
    println!("{}{}", d.name, if d.is_default { " [default]" } else { "" });
}
```

## Sample rate mismatch

Si el device no soporta el sample rate solicitado, cpal usará el sample rate por defecto del device y `AudioCapture`/`AudioPlayback` emitirá un `tracing::warn`. En una versión futura se integrará `rubato` para resamplear automáticamente. Por ahora, la recomendación es usar el sample rate reportado por el device o 48 kHz (soportado por la mayoría de hardware moderno).

## Backpressure y jitter buffer

El `JitterBuffer` del transport crate absorbe variaciones de hasta ~100 ms. Si el decoder produce frames más rápido de lo que el altavoz los consume, el `VecDeque` del playback crece (backpressure hacia arriba). No hay mecanismo de drop automático — en producción se recomienda monitorear `pb.queue_len()` (pendiente Track B) y descartar frames obsoletos.

## CI headless

En CI no hay hardware de audio. Los tests de io se protegen con la feature `hw-smoke` (desactivada por defecto). Los ejemplos `mic_to_speaker` y `voip_peer` degradan automáticamente a un source sintético 440 Hz si no hay device disponible.
