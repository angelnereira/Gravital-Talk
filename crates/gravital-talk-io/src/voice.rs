//! Captura y reproducción de voz en el formato de la sesión.
//!
//! [`AudioCapture`](crate::AudioCapture) y [`AudioPlayback`](crate::AudioPlayback)
//! trabajan en el formato por defecto del device: si la tarjeta da 44.1 kHz
//! estéreo, eso es lo que entregan y lo que esperan. Para una llamada eso no
//! sirve: los dos extremos tienen que hablar el mismo formato, y es el de la
//! sesión (mono, a su sample rate), no el de cada tarjeta.
//!
//! Este módulo hace esa adaptación en los dos sentidos:
//!
//! - [`VoiceCapture`] entrega frames **mono** al sample rate pedido, del tamaño
//!   exacto pedido, sea cual sea el formato del micrófono.
//! - [`VoicePlayback`] acepta mono al sample rate pedido y lo adapta a la
//!   salida, con una cola **acotada**: si el emisor va más rápido que la
//!   tarjeta (deriva de reloj) se descarta lo más viejo en vez de acumular
//!   latencia sin límite.
//!
//! Primero se pide al device el formato exacto, que es el camino sin
//! conversión. Sólo si lo rechaza se cae al formato por defecto y se convierte
//! (mezcla de canales + interpolación lineal). La interpolación lineal no es
//! un resampler de estudio, pero es el camino de reserva y para voz basta.

use std::collections::VecDeque;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, SampleRate, Stream, SupportedStreamConfig};

use crate::devices::{pick_input, pick_output};
use crate::error::IoError;
use crate::Result;

/// Frames de captura que caben en la cola antes de descartar.
///
/// Si el consumidor se atasca, es mejor perder audio viejo que entregarlo
/// tarde: en una llamada, el audio retrasado es peor que el que falta.
const CAPTURE_QUEUE_FRAMES: usize = 16;

/// Audio que se acumula antes de empezar a sonar, en milisegundos.
///
/// Absorbe el jitter de llegada: sin colchón, cada paquete que llega un poco
/// tarde es un chasquido.
const PLAYOUT_PREFILL_MS: usize = 30;

/// Tope de la cola de reproducción, en milisegundos.
const PLAYOUT_MAX_MS: usize = 120;

/// Serializa la apertura de devices.
///
/// Captura y reproducción se abren desde hilos distintos, y enumerar devices
/// de ALSA en paralelo falla de forma intermitente: uno de los dos se queda
/// sin device. Abrirlos de uno en uno cuesta unos milisegundos, una vez.
static OPEN_LOCK: Mutex<()> = Mutex::new(());

fn open_guard() -> std::sync::MutexGuard<'static, ()> {
    match OPEN_LOCK.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

/// Formatos de sample que este módulo sabe convertir.
const fn handled(format: SampleFormat) -> bool {
    matches!(
        format,
        SampleFormat::I16 | SampleFormat::F32 | SampleFormat::U16
    )
}

/// Preferencia entre formatos: `i16` no necesita conversión.
const fn format_rank(format: SampleFormat) -> u8 {
    match format {
        SampleFormat::I16 => 0,
        SampleFormat::F32 => 1,
        _ => 2,
    }
}

/// Elige, entre las configuraciones soportadas, la que da `rate` con menos
/// conversión: primero mono, luego el formato más barato.
fn pick_exact<I>(ranges: I, rate: u32) -> Option<SupportedStreamConfig>
where
    I: Iterator<Item = cpal::SupportedStreamConfigRange>,
{
    ranges
        .filter(|r| {
            handled(r.sample_format())
                && r.min_sample_rate().0 <= rate
                && rate <= r.max_sample_rate().0
        })
        .min_by_key(|r| {
            (
                r.channels() != 1,
                r.channels(),
                format_rank(r.sample_format()),
            )
        })
        .map(|r| r.with_sample_rate(SampleRate(rate)))
}

// ── Captura ───────────────────────────────────────────────────────────────

/// Convierte lo que da el micrófono a frames mono del tamaño de la sesión.
struct CaptureAdapter {
    in_channels: usize,
    /// Muestras de entrada por cada muestra de salida.
    step: f64,
    pos: f64,
    prev: f32,
    /// Acumulador del frame de entrada en curso (mezcla de canales).
    chan_sum: f32,
    chan_n: usize,
    frame: Vec<i16>,
    frame_samples: usize,
    tx: SyncSender<Vec<i16>>,
}

impl CaptureAdapter {
    fn push(&mut self, sample: f32) {
        self.chan_sum += sample;
        self.chan_n += 1;
        if self.chan_n < self.in_channels {
            return;
        }
        let mono = self.chan_sum / self.in_channels as f32;
        self.chan_sum = 0.0;
        self.chan_n = 0;

        if (self.step - 1.0).abs() < f64::EPSILON {
            self.emit(mono);
        } else {
            loop {
                if self.pos >= 1.0 {
                    break;
                }
                let v = (mono - self.prev).mul_add(self.pos as f32, self.prev);
                self.emit(v);
                self.pos += self.step;
            }
            self.pos -= 1.0;
        }
        self.prev = mono;
    }

    fn emit(&mut self, sample: f32) {
        let s = (sample.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
        self.frame.push(s);
        if self.frame.len() >= self.frame_samples {
            let full = core::mem::replace(&mut self.frame, Vec::with_capacity(self.frame_samples));
            // `try_send`: si la cola está llena se pierde este frame. Bloquear
            // aquí pararía el hilo de tiempo real de la tarjeta.
            let _ = self.tx.try_send(full);
        }
    }
}

/// Micrófono entregando frames mono al sample rate de la sesión.
pub struct VoiceCapture {
    _stream: Stream,
}

impl core::fmt::Debug for VoiceCapture {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VoiceCapture").finish_non_exhaustive()
    }
}

impl VoiceCapture {
    /// Abre el micrófono por defecto.
    ///
    /// El `Receiver` entrega vectores de exactamente `frame_samples` muestras
    /// mono a `sample_rate` Hz.
    pub fn start(sample_rate: u32, frame_samples: usize) -> Result<(Self, Receiver<Vec<i16>>)> {
        let _opening = open_guard();
        let device = pick_input(None)?;
        let exact = device
            .supported_input_configs()
            .ok()
            .and_then(|ranges| pick_exact(ranges, sample_rate));
        let fallback = device.default_input_config().map_err(IoError::from);

        let (tx, rx) = mpsc::sync_channel(CAPTURE_QUEUE_FRAMES);

        // Del más ajustado al más permisivo: formato exacto con buffer del
        // tamaño del frame (mínima latencia), el mismo con el buffer que elija
        // el device, y por último el formato por defecto con conversión.
        let mut attempts: Vec<(SupportedStreamConfig, BufferSize)> = Vec::new();
        if let Some(cfg) = exact {
            attempts.push((cfg.clone(), BufferSize::Fixed(frame_samples as u32)));
            attempts.push((cfg, BufferSize::Default));
        }
        if let Ok(cfg) = fallback {
            if handled(cfg.sample_format()) {
                attempts.push((cfg, BufferSize::Default));
            }
        }

        let mut last_err = IoError::NoDevice { kind: "input" };
        for (supported, buffer_size) in attempts {
            let mut config = supported.config();
            config.buffer_size = buffer_size;
            let adapter = CaptureAdapter {
                in_channels: usize::from(config.channels.max(1)),
                step: f64::from(config.sample_rate.0) / f64::from(sample_rate),
                pos: 0.0,
                prev: 0.0,
                chan_sum: 0.0,
                chan_n: 0,
                frame: Vec::with_capacity(frame_samples),
                frame_samples,
                tx: tx.clone(),
            };
            match build_input(&device, &config, supported.sample_format(), adapter) {
                Ok(stream) => {
                    stream.play()?;
                    tracing::debug!(
                        rate = config.sample_rate.0,
                        channels = config.channels,
                        ?buffer_size,
                        "captura de voz abierta"
                    );
                    return Ok((Self { _stream: stream }, rx));
                }
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }
}

fn build_input(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: SampleFormat,
    mut adapter: CaptureAdapter,
) -> Result<Stream> {
    let err_fn = |e| tracing::error!(%e, "error en el stream de captura");
    let stream = match format {
        SampleFormat::I16 => device.build_input_stream(
            config,
            move |data: &[i16], _: &_| {
                for &s in data {
                    adapter.push(f32::from(s) / 32768.0);
                }
            },
            err_fn,
            None,
        )?,
        SampleFormat::F32 => device.build_input_stream(
            config,
            move |data: &[f32], _: &_| {
                for &s in data {
                    adapter.push(s);
                }
            },
            err_fn,
            None,
        )?,
        SampleFormat::U16 => device.build_input_stream(
            config,
            move |data: &[u16], _: &_| {
                for &s in data {
                    adapter.push((f32::from(s) - 32768.0) / 32768.0);
                }
            },
            err_fn,
            None,
        )?,
        other => {
            return Err(IoError::UnsupportedConfig(format!(
                "sample format {other:?} not handled"
            )))
        }
    };
    Ok(stream)
}

// ── Reproducción ──────────────────────────────────────────────────────────

/// Cola de reproducción compartida entre quien empuja y la tarjeta.
struct PlayoutQueue {
    samples: VecDeque<i16>,
    /// `false` mientras se rellena el colchón inicial (o tras vaciarse).
    primed: bool,
    prefill: usize,
    max: usize,
}

/// Lado de la tarjeta: saca mono de la cola y lo adapta a la salida.
struct PlaybackAdapter {
    queue: Arc<Mutex<PlayoutQueue>>,
    out_channels: usize,
    /// Muestras de la sesión por cada muestra de salida.
    step: f64,
    pos: f64,
    prev: f32,
    cur: f32,
}

impl PlaybackAdapter {
    /// Rellena `frames` posiciones de salida llamando a `write` con la muestra
    /// mono de cada una (ya interpolada).
    fn render(&mut self, frames: usize, mut write: impl FnMut(usize, f32)) {
        let mut q = match self.queue.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if !q.primed && q.samples.len() >= q.prefill {
            q.primed = true;
        }
        for i in 0..frames {
            if !q.primed {
                write(i, 0.0);
                continue;
            }
            self.pos += self.step;
            loop {
                if self.pos < 1.0 {
                    break;
                }
                self.prev = self.cur;
                let popped = q.samples.pop_front();
                if popped.is_none() {
                    // Se vació: silencio y a rellenar el colchón otra vez.
                    q.primed = false;
                }
                self.cur = popped.map_or(0.0, |s| f32::from(s) / 32768.0);
                self.pos -= 1.0;
            }
            write(
                i,
                (self.cur - self.prev).mul_add(self.pos as f32, self.prev),
            );
        }
        drop(q);
    }
}

/// Mezcla sonidos cortos (tonos de aviso) sobre lo que esté sonando.
///
/// A diferencia de [`VoicePlayback::push`], no añade cola: suma las muestras
/// a las que ya esperan, así que un tono no retrasa la voz que viene detrás.
/// Se puede clonar y usar desde otro hilo que el que abrió el altavoz.
#[derive(Clone)]
pub struct PlayoutMixer {
    queue: Arc<Mutex<PlayoutQueue>>,
}

impl core::fmt::Debug for PlayoutMixer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PlayoutMixer").finish_non_exhaustive()
    }
}

impl PlayoutMixer {
    /// Suma `mono` a lo pendiente de sonar, empezando ya.
    pub fn mix(&self, mono: &[i16]) {
        let mut q = match self.queue.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        q.mix(mono);
    }
}

impl PlayoutQueue {
    fn mix(&mut self, mono: &[i16]) {
        let overlap = mono.len().min(self.samples.len());
        for (queued, &extra) in self.samples.iter_mut().zip(&mono[..overlap]) {
            *queued = queued.saturating_add(extra);
        }
        self.samples.extend(mono[overlap..].iter().copied());
        // Un tono es más corto que el colchón inicial: si se esperase a
        // llenarlo, no sonaría hasta que llegara voz.
        self.primed = true;
    }
}

/// Altavoz que acepta mono al sample rate de la sesión.
pub struct VoicePlayback {
    _stream: Stream,
    queue: Arc<Mutex<PlayoutQueue>>,
}

impl core::fmt::Debug for VoicePlayback {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VoicePlayback").finish_non_exhaustive()
    }
}

impl VoicePlayback {
    /// Abre la salida por defecto para reproducir mono a `sample_rate` Hz.
    pub fn start(sample_rate: u32, frame_samples: usize) -> Result<Self> {
        let _opening = open_guard();
        let device = pick_output(None)?;
        let exact = device
            .supported_output_configs()
            .ok()
            .and_then(|ranges| pick_exact(ranges, sample_rate));
        let fallback = device.default_output_config().map_err(IoError::from);

        let per_ms = sample_rate as usize / 1000;
        let queue = Arc::new(Mutex::new(PlayoutQueue {
            samples: VecDeque::with_capacity(per_ms * PLAYOUT_MAX_MS * 2),
            primed: false,
            prefill: per_ms * PLAYOUT_PREFILL_MS,
            max: per_ms * PLAYOUT_MAX_MS,
        }));

        let mut attempts: Vec<(SupportedStreamConfig, BufferSize)> = Vec::new();
        if let Some(cfg) = exact {
            attempts.push((cfg.clone(), BufferSize::Fixed(frame_samples as u32)));
            attempts.push((cfg, BufferSize::Default));
        }
        if let Ok(cfg) = fallback {
            if handled(cfg.sample_format()) {
                attempts.push((cfg, BufferSize::Default));
            }
        }

        let mut last_err = IoError::NoDevice { kind: "output" };
        for (supported, buffer_size) in attempts {
            let mut config = supported.config();
            config.buffer_size = buffer_size;
            let adapter = PlaybackAdapter {
                queue: queue.clone(),
                out_channels: usize::from(config.channels.max(1)),
                step: f64::from(sample_rate) / f64::from(config.sample_rate.0),
                pos: 0.0,
                prev: 0.0,
                cur: 0.0,
            };
            match build_output(&device, &config, supported.sample_format(), adapter) {
                Ok(stream) => {
                    stream.play()?;
                    tracing::debug!(
                        rate = config.sample_rate.0,
                        channels = config.channels,
                        ?buffer_size,
                        "reproducción de voz abierta"
                    );
                    return Ok(Self {
                        _stream: stream,
                        queue,
                    });
                }
                Err(e) => last_err = e,
            }
        }
        Err(last_err)
    }

    /// Mezclador para sonar avisos sobre la voz, utilizable desde otro hilo.
    #[must_use]
    pub fn mixer(&self) -> PlayoutMixer {
        PlayoutMixer {
            queue: self.queue.clone(),
        }
    }

    /// Encola muestras mono para reproducir.
    ///
    /// Si la cola supera el tope se descarta lo más viejo hasta volver al
    /// colchón: es lo que mantiene la latencia acotada cuando el reloj del
    /// emisor corre más que el de esta tarjeta.
    pub fn push(&self, mono: &[i16]) {
        let mut q = match self.queue.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        q.samples.extend(mono.iter().copied());
        if q.samples.len() > q.max {
            let excess = q.samples.len() - q.prefill;
            q.samples.drain(..excess);
        }
    }

    /// Milisegundos de audio pendientes de sonar.
    pub fn queued_ms(&self, sample_rate: u32) -> usize {
        let q = match self.queue.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        q.samples.len() * 1000 / (sample_rate as usize).max(1)
    }
}

fn build_output(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    format: SampleFormat,
    mut adapter: PlaybackAdapter,
) -> Result<Stream> {
    let err_fn = |e| tracing::error!(%e, "error en el stream de reproducción");
    let ch = adapter.out_channels;
    let stream = match format {
        SampleFormat::I16 => device.build_output_stream(
            config,
            move |data: &mut [i16], _: &_| {
                adapter.render(data.len() / ch, |i, v| {
                    let s = (v.clamp(-1.0, 1.0) * f32::from(i16::MAX)) as i16;
                    data[i * ch..(i + 1) * ch].fill(s);
                });
            },
            err_fn,
            None,
        )?,
        SampleFormat::F32 => device.build_output_stream(
            config,
            move |data: &mut [f32], _: &_| {
                adapter.render(data.len() / ch, |i, v| {
                    data[i * ch..(i + 1) * ch].fill(v);
                });
            },
            err_fn,
            None,
        )?,
        SampleFormat::U16 => device.build_output_stream(
            config,
            move |data: &mut [u16], _: &_| {
                adapter.render(data.len() / ch, |i, v| {
                    let s = (v.clamp(-1.0, 1.0).mul_add(32767.0, 32768.0)) as u16;
                    data[i * ch..(i + 1) * ch].fill(s);
                });
            },
            err_fn,
            None,
        )?,
        other => {
            return Err(IoError::UnsupportedConfig(format!(
                "sample format {other:?} not handled"
            )))
        }
    };
    Ok(stream)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capture(
        in_channels: usize,
        in_rate: u32,
        out_rate: u32,
        frame: usize,
    ) -> (CaptureAdapter, Receiver<Vec<i16>>) {
        let (tx, rx) = mpsc::sync_channel(256);
        (
            CaptureAdapter {
                in_channels,
                step: f64::from(in_rate) / f64::from(out_rate),
                pos: 0.0,
                prev: 0.0,
                chan_sum: 0.0,
                chan_n: 0,
                frame: Vec::with_capacity(frame),
                frame_samples: frame,
                tx,
            },
            rx,
        )
    }

    #[test]
    fn captura_estereo_se_mezcla_a_mono_en_frames_exactos() {
        let (mut a, rx) = capture(2, 48_000, 48_000, 480);
        // Un canal a +0.5 y otro a -0.5 se cancelan: la mezcla es silencio.
        for _ in 0..480 {
            a.push(0.5);
            a.push(-0.5);
        }
        let frame = rx.try_recv().expect("un frame completo");
        assert_eq!(frame.len(), 480);
        assert!(frame.iter().all(|&s| s == 0));
        assert!(rx.try_recv().is_err(), "no debe sobrar un segundo frame");
    }

    #[test]
    fn captura_a_44100_entrega_el_ritmo_de_48000() {
        let (mut a, rx) = capture(1, 44_100, 48_000, 480);
        // Un segundo de entrada a 44.1 kHz debe dar ~un segundo a 48 kHz.
        for _ in 0..44_100 {
            a.push(0.25);
        }
        let frames = rx.try_iter().count();
        assert!((99..=100).contains(&frames), "frames = {frames}");
    }

    #[test]
    fn la_cola_de_reproduccion_no_crece_sin_limite() {
        let queue = Arc::new(Mutex::new(PlayoutQueue {
            samples: VecDeque::new(),
            primed: false,
            prefill: 48 * PLAYOUT_PREFILL_MS,
            max: 48 * PLAYOUT_MAX_MS,
        }));
        // Mismo recorte que `VoicePlayback::push`, sin abrir una tarjeta.
        for _ in 0..1000 {
            let mut q = queue.lock().unwrap();
            q.samples.extend(core::iter::repeat_n(1i16, 480));
            if q.samples.len() > q.max {
                let excess = q.samples.len() - q.prefill;
                q.samples.drain(..excess);
            }
        }
        assert!(queue.lock().unwrap().samples.len() <= 48 * PLAYOUT_MAX_MS);
    }

    #[test]
    fn un_aviso_se_suma_a_la_voz_sin_alargar_la_cola() {
        let mut q = PlayoutQueue {
            samples: VecDeque::from(vec![100i16, 100, i16::MAX]),
            primed: false,
            prefill: 480,
            max: 10_000,
        };
        q.mix(&[5, -5, 10, 7, 8]);
        // Lo que coincide se suma (saturando); lo que sobra se añade.
        assert_eq!(q.samples, VecDeque::from(vec![105, 95, i16::MAX, 7, 8]));
        // Y suena ya, aunque no llegue al colchón.
        assert!(q.primed);
    }

    #[test]
    fn la_reproduccion_espera_al_colchon_y_luego_suena() {
        let queue = Arc::new(Mutex::new(PlayoutQueue {
            samples: VecDeque::new(),
            primed: false,
            prefill: 100,
            max: 10_000,
        }));
        let mut a = PlaybackAdapter {
            queue: queue.clone(),
            out_channels: 1,
            step: 1.0,
            pos: 0.0,
            prev: 0.0,
            cur: 0.0,
        };
        // Por debajo del colchón: silencio y no se consume nada.
        queue.lock().unwrap().samples.extend([16_000i16; 50]);
        let mut out = [1.0f32; 10];
        a.render(10, |i, v| out[i] = v);
        assert!(out.iter().all(|&v| v == 0.0));
        assert_eq!(queue.lock().unwrap().samples.len(), 50);

        // Con el colchón lleno empieza a sonar.
        queue.lock().unwrap().samples.extend([16_000i16; 100]);
        let mut out = [0.0f32; 10];
        a.render(10, |i, v| out[i] = v);
        assert!(out[9] > 0.4, "debe salir la señal, salió {}", out[9]);
    }
}
