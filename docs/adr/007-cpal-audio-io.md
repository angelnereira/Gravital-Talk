# ADR 007 — cpal para captura y playback de audio

**Estado**: Aceptado  
**Fecha**: 2026-04-24  
**Autores**: Angel Nereira

## Contexto

Track A añade soporte de audio real (micrófono → red → altavoz). Se necesita una capa de I/O de audio que:
- Funcione en Linux (ALSA), macOS (CoreAudio) y Windows (WASAPI) sin cambios de código.
- No exponga directamente APIs de bajo nivel al consumer.
- Tenga bajo overhead (callback de tiempo real sin allocaciones en hot path).
- Sea suficientemente madura para producción.

## Decisión

Se adopta **`cpal 0.15`** como capa de abstracción de audio. Implementado en el crate `gravital-sound-io`.

## Diseño del adaptador

El callback de cpal corre en un thread de tiempo real. Para desacoplarlo del pipeline Gravital Sound (que vive en el runtime Tokio):

- **Captura**: callback → conversión de formato → `mpsc::Sender<Vec<i16>>` sin bloqueo.
- **Playback**: `mpsc::Receiver<Vec<i16>>` en pump thread → `VecDeque<i16>` → callback de cpal.

Este diseño garantiza que el thread RT nunca espera al runtime Tokio.

## Alternativas descartadas

| Opción | Razón del descarte |
|--------|--------------------|
| `portaudio-rs` | Requiere bindings FFI no-safe, poco activo |
| `libpulse-binding` | Solo Linux/PulseAudio, no cross-platform |
| `rodio` | Solo playback, no captura; abstracción demasiado alta |
| Llamadas ALSA directas | No portable, mucho boilerplate |

## Consecuencias

- En Linux se necesita `libasound2-dev` para compilar.
- En macOS CoreAudio está incluido en el SDK (sin paquete extra).
- Los tests de hardware se marcan `#[cfg(feature = "hw-smoke")]` para no fallar en CI headless.
- Si el device no soporta 48 kHz nativo, `AudioCapture`/`AudioPlayback` emite un warning y usa el sample rate por defecto del device.
