//! Audio nativo de la sesión: micrófono → red y red → altavoz.
//!
//! ## Por qué vive aquí y no en el embebedor
//!
//! En escritorio la app no tiene un plugin de audio que capture y reproduzca
//! PCM con baja latencia, así que el audio se quedaba en un tono de prueba y
//! lo recibido se descartaba. Hacerlo en Rust con `cpal` lo resuelve sin
//! pasar cada frame por el lenguaje anfitrión: el camino micrófono → socket no
//! cruza ninguna frontera.
//!
//! Sólo se compila con la feature `audio`. Sin ella (Android, iOS, wasm) las
//! funciones `gs_audio_*` existen igual —la ABI no cambia según el target—
//! pero devuelven `GS_ERR_UNSUPPORTED` y el embebedor usa su propio audio.
//!
//! ## Hilos
//!
//! Dos hilos propios, porque los streams de `cpal` no son `Send` y tienen que
//! vivir en el hilo que los crea:
//!
//! - **tx**: lee frames del micrófono y, con PTT activo, los envía.
//! - **rx**: saca frames de la sesión y los encola en el altavoz.
//!
//! Ambos usan `Runtime::block_on` sobre el runtime de la sesión. Tiene que ser
//! el `Runtime` y no un `Handle`: en un runtime `current_thread`, sólo
//! `Runtime::block_on` mueve el driver de IO y los timers.

// Sin la feature sólo queda el estado compartido, que entonces nadie escribe.
#![cfg_attr(not(feature = "audio"), allow(dead_code))]

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};

/// Estado compartido entre los hilos de audio y la FFI.
#[derive(Debug, Default)]
pub struct Shared {
    stop: AtomicBool,
    ptt: AtomicBool,
    /// `f32` en bits: nivel 0..1 del micrófono.
    mic_level: AtomicU32,
    /// `f32` en bits: nivel 0..1 del audio recibido.
    rx_level: AtomicU32,
    tx_frames: AtomicU64,
    rx_frames: AtomicU64,
    capture_ok: AtomicBool,
    playback_ok: AtomicBool,
    /// Por qué no abrió un device, para que el embebedor pueda decirlo.
    problems: std::sync::Mutex<Vec<String>>,
}

/// Foto del estado del audio, para la UI y el diagnóstico.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stats {
    pub mic_level: f32,
    pub rx_level: f32,
    pub tx_frames: u64,
    pub rx_frames: u64,
    pub capture_ok: bool,
    pub playback_ok: bool,
}

impl Shared {
    pub fn set_ptt(&self, on: bool) {
        self.ptt.store(on, Ordering::Release);
    }

    fn report(&self, problem: String) {
        if let Ok(mut p) = self.problems.lock() {
            p.push(problem);
        }
    }

    /// Fallos de apertura de devices, en una línea. `None` si no hubo.
    pub fn problems(&self) -> Option<String> {
        let p = self.problems.lock().ok()?;
        (!p.is_empty()).then(|| p.join("; "))
    }

    pub fn stats(&self) -> Stats {
        Stats {
            mic_level: f32::from_bits(self.mic_level.load(Ordering::Relaxed)),
            rx_level: f32::from_bits(self.rx_level.load(Ordering::Relaxed)),
            tx_frames: self.tx_frames.load(Ordering::Relaxed),
            rx_frames: self.rx_frames.load(Ordering::Relaxed),
            capture_ok: self.capture_ok.load(Ordering::Relaxed),
            playback_ok: self.playback_ok.load(Ordering::Relaxed),
        }
    }
}

/// Nivel 0..1 de un bloque de muestras (RMS con ganancia para el medidor).
fn level(samples: &[i16]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum: f64 = samples
        .iter()
        .map(|&s| {
            let f = f64::from(s) / 32768.0;
            f * f
        })
        .sum();
    ((sum / samples.len() as f64).sqrt() as f32 * 3.0).clamp(0.0, 1.0)
}

#[cfg(feature = "audio")]
pub use imp::Engine;

#[cfg(feature = "audio")]
mod imp {
    use std::sync::atomic::Ordering;
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::sync::Arc;
    use std::thread::JoinHandle;
    use std::time::{Duration, Instant};

    use gravital_talk::constants::MAX_PAYLOAD_SIZE;
    use gravital_talk::Session;
    use gravital_talk_io::{VoiceCapture, VoicePlayback};
    use tokio::runtime::Runtime;

    use super::{level, Shared};

    /// Bytes que la sesión añade a cada frame: secuencia de audio + tag AEAD.
    const FRAME_OVERHEAD: usize = 4 + 16;

    /// Variable de entorno de diagnóstico: sustituye el micrófono por un tono
    /// de esa frecuencia (Hz). Permite medir el camino de ida sin depender de
    /// que haya alguien hablando.
    const TEST_TONE_ENV: &str = "GS_AUDIO_TEST_TONE";

    /// Motor de audio de una sesión. Se para al soltarlo.
    #[derive(Debug)]
    pub struct Engine {
        shared: Arc<Shared>,
        threads: Vec<JoinHandle<()>>,
    }

    impl Engine {
        pub fn start(
            runtime: Arc<Runtime>,
            session: Arc<Session>,
            sample_rate: u32,
            channels: u8,
            frame_ms: u8,
        ) -> Result<Self, String> {
            let channels = usize::from(channels.max(1));
            let frame_samples = sample_rate as usize * usize::from(frame_ms) / 1000;
            if frame_samples == 0 {
                return Err("frame de audio vacío: revisa sample_rate y frame_duration_ms".into());
            }
            let frame_bytes = frame_samples * channels * 2;
            if frame_bytes + FRAME_OVERHEAD > MAX_PAYLOAD_SIZE {
                return Err(format!(
                    "un frame PCM de {frame_ms} ms son {frame_bytes} B y el tope por paquete es \
                     {} B: usa un frame más corto",
                    MAX_PAYLOAD_SIZE - FRAME_OVERHEAD
                ));
            }

            let shared = Arc::new(Shared::default());
            let (ready_tx, ready_rx) = mpsc::channel::<()>();

            let tx_thread = {
                let (shared, runtime, session, ready) = (
                    shared.clone(),
                    runtime.clone(),
                    session.clone(),
                    ready_tx.clone(),
                );
                std::thread::Builder::new()
                    .name("gs-audio-tx".into())
                    .spawn(move || {
                        tx_loop(
                            &shared,
                            &runtime,
                            &session,
                            sample_rate,
                            frame_samples,
                            channels,
                            &ready,
                        );
                    })
                    .map_err(|e| format!("hilo de captura: {e}"))?
            };
            let rx_thread = {
                let (shared, ready) = (shared.clone(), ready_tx);
                std::thread::Builder::new()
                    .name("gs-audio-rx".into())
                    .spawn(move || {
                        rx_loop(
                            &shared,
                            &runtime,
                            &session,
                            sample_rate,
                            frame_samples,
                            channels,
                            &ready,
                        );
                    })
                    .map_err(|e| format!("hilo de reproducción: {e}"))?
            };

            // Se espera a que los dos hayan intentado abrir su device, para que
            // `capture_ok`/`playback_ok` ya digan la verdad al volver.
            for _ in 0..2 {
                let _ = ready_rx.recv_timeout(Duration::from_secs(5));
            }

            Ok(Self {
                shared,
                threads: vec![tx_thread, rx_thread],
            })
        }

        pub const fn shared(&self) -> &Arc<Shared> {
            &self.shared
        }
    }

    impl Drop for Engine {
        fn drop(&mut self) {
            self.shared.stop.store(true, Ordering::Release);
            for t in self.threads.drain(..) {
                let _ = t.join();
            }
        }
    }

    /// Origen de los frames a transmitir.
    enum Source {
        /// El stream se guarda sólo para mantenerlo vivo: se cierra al soltarlo.
        Mic {
            _capture: VoiceCapture,
            frames: mpsc::Receiver<Vec<i16>>,
        },
        Tone {
            step: f64,
            phase: f64,
            next: Instant,
        },
        /// Sin micrófono: la sesión sigue, sólo que no se puede hablar.
        None,
    }

    fn tx_loop(
        shared: &Shared,
        runtime: &Runtime,
        session: &Arc<Session>,
        sample_rate: u32,
        frame_samples: usize,
        channels: usize,
        ready: &mpsc::Sender<()>,
    ) {
        let tone_hz = std::env::var(TEST_TONE_ENV)
            .ok()
            .and_then(|v| v.parse::<f64>().ok())
            .filter(|hz| *hz > 0.0);
        let open_mic = || match VoiceCapture::start(sample_rate, frame_samples) {
            Ok((capture, frames)) => Source::Mic {
                _capture: capture,
                frames,
            },
            Err(e) => {
                tracing::warn!(%e, "sin micrófono: sólo escucha");
                shared.report(format!("micrófono: {e}"));
                Source::None
            }
        };
        let mut source = tone_hz.map_or_else(open_mic, |hz| Source::Tone {
            step: std::f64::consts::TAU * hz / f64::from(sample_rate),
            phase: 0.0,
            next: Instant::now(),
        });
        shared
            .capture_ok
            .store(!matches!(source, Source::None), Ordering::Release);
        let _ = ready.send(());

        let frame_time =
            Duration::from_micros(frame_samples as u64 * 1_000_000 / u64::from(sample_rate));
        let mut bytes = vec![0u8; frame_samples * channels * 2];

        while !shared.stop.load(Ordering::Acquire) {
            let frame = match &mut source {
                Source::Mic { frames, .. } => match frames.recv_timeout(Duration::from_millis(100))
                {
                    Ok(f) => f,
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => break,
                },
                Source::Tone { step, phase, next } => {
                    let now = Instant::now();
                    if *next > now {
                        std::thread::sleep(*next - now);
                    }
                    *next += frame_time;
                    (0..frame_samples)
                        .map(|_| {
                            let s = (phase.sin() * 9000.0) as i16;
                            *phase = (*phase + *step) % std::f64::consts::TAU;
                            s
                        })
                        .collect()
                }
                Source::None => {
                    std::thread::sleep(Duration::from_millis(100));
                    continue;
                }
            };

            if !shared.ptt.load(Ordering::Acquire) {
                shared.mic_level.store(0f32.to_bits(), Ordering::Relaxed);
                continue;
            }
            shared
                .mic_level
                .store(level(&frame).to_bits(), Ordering::Relaxed);

            for (i, s) in frame.iter().enumerate() {
                let le = s.to_le_bytes();
                for c in 0..channels {
                    let o = (i * channels + c) * 2;
                    bytes[o] = le[0];
                    bytes[o + 1] = le[1];
                }
            }
            if runtime.block_on(session.send_audio(&bytes)).is_ok() {
                shared.tx_frames.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    fn rx_loop(
        shared: &Shared,
        runtime: &Runtime,
        session: &Arc<Session>,
        sample_rate: u32,
        frame_samples: usize,
        channels: usize,
        ready: &mpsc::Sender<()>,
    ) {
        let playback = match VoicePlayback::start(sample_rate, frame_samples) {
            Ok(p) => Some(p),
            Err(e) => {
                tracing::warn!(%e, "sin salida de audio: lo recibido no sonará");
                shared.report(format!("altavoz: {e}"));
                None
            }
        };
        shared
            .playback_ok
            .store(playback.is_some(), Ordering::Release);
        let _ = ready.send(());

        let mut mono: Vec<i16> = Vec::with_capacity(frame_samples * 2);
        while !shared.stop.load(Ordering::Acquire) {
            let res = runtime.block_on(async {
                tokio::time::timeout(Duration::from_millis(200), session.recv_audio()).await
            });
            match res {
                Ok(Ok(frame)) => {
                    mono.clear();
                    mono.extend(frame.payload.chunks_exact(2 * channels).map(|c| {
                        // Varios canales se mezclan a mono: el altavoz se abre
                        // en mono y reparte él a sus canales.
                        let sum: i32 = c
                            .as_chunks::<2>()
                            .0
                            .iter()
                            .map(|b| i32::from(i16::from_le_bytes(*b)))
                            .sum();
                        (sum / channels as i32) as i16
                    }));
                    shared
                        .rx_level
                        .store(level(&mono).to_bits(), Ordering::Relaxed);
                    shared.rx_frames.fetch_add(1, Ordering::Relaxed);
                    if let Some(p) = &playback {
                        p.push(&mono);
                    }
                }
                // Sesión aún no activa o ya cerrada: no hay nada que leer, y
                // girar en vacío quemaría un núcleo.
                Ok(Err(_)) => {
                    shared.rx_level.store(0f32.to_bits(), Ordering::Relaxed);
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(_) => shared.rx_level.store(0f32.to_bits(), Ordering::Relaxed),
            }
        }
    }
}

/// Sin la feature `audio` no hay motor: el tipo existe para que el resto del
/// crate no tenga que condicionar cada uso.
#[cfg(not(feature = "audio"))]
#[derive(Debug)]
pub struct Engine {
    shared: std::sync::Arc<Shared>,
}

#[cfg(not(feature = "audio"))]
impl Engine {
    pub const fn shared(&self) -> &std::sync::Arc<Shared> {
        &self.shared
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_nivel_del_silencio_es_cero_y_el_de_fondo_de_escala_satura() {
        assert_eq!(level(&[0; 480]), 0.0);
        assert_eq!(level(&[]), 0.0);
        assert!((level(&[i16::MAX; 480]) - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn las_estadisticas_reflejan_el_estado_compartido() {
        let s = Shared::default();
        s.set_ptt(true);
        s.tx_frames.store(7, Ordering::Relaxed);
        s.mic_level.store(0.5f32.to_bits(), Ordering::Relaxed);
        let st = s.stats();
        assert_eq!(st.tx_frames, 7);
        assert!((st.mic_level - 0.5).abs() < f32::EPSILON);
        assert!(!st.capture_ok);
    }
}
