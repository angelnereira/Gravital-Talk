//! Camino de audio de la sesión: PCM por fuera, voz comprimida por la red.
//!
//! El embebedor entrega y recibe PCM, como siempre. Aquí se decide qué viaja:
//! si los dos extremos negociaron Opus, cada frame se limpia de ruido y se
//! comprime antes de enviarlo, y al recibir se descomprime tapando lo que se
//! haya perdido por el camino. Si no (un peer que sólo habla PCM), pasa tal
//! cual.
//!
//! Que esto viva en la FFI y no en la app tiene una razón: así Android (que
//! captura con plugins de Flutter) y escritorio (que captura en Rust) usan
//! exactamente el mismo códec, y ninguno tiene que saber que existe.

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, Ordering};
use std::sync::{Arc, Mutex};

use gravital_talk::constants::MAX_PAYLOAD_SIZE;
use gravital_talk::{Session, TransportError};
use gravital_talk_codec::voice::{
    VoiceDecoder, VoiceEncoder, DTX_PACKET, MAX_BITRATE, MAX_PACKET, MIN_BITRATE,
};
use gravital_talk_codec::voice_dsp::Biquad;
use gravital_talk_codec::CodecId;
use gravital_talk_codec::{VoiceProfile, VoiceTuning};

/// Bytes que la sesión añade a cada frame: secuencia de audio + tag AEAD.
const FRAME_OVERHEAD: usize = 4 + 16;

/// Frames entre dos ajustes del códec a la red (1 s con frames de 20 ms). No
/// más espaciado: mientras no se ajusta, una red que empieza a perder
/// paquetes no tiene copia de respaldo.
const ADAPT_EVERY: u32 = 50;

/// Valor de `profile` cuando nadie ha elegido perfil: el códec se queda como
/// arranca ([`VoiceTuning::LEGACY`]).
const NO_PROFILE: u8 = u8::MAX;

/// Centro y anchura del realce de presencia: la zona de las consonantes,
/// con una campana ancha para que no suene nasal.
const PRESENCE_HZ: f32 = 3_000.0;
const PRESENCE_Q: f32 = 0.9;

/// Frames perdidos seguidos que se tapan como mucho. Más allá, sintetizar
/// audio que nadie dijo es peor que un hueco, y además desborda el buffer de
/// quien recibe.
const MAX_CONCEALED: u32 = 3;

/// Suelo de la tasa cuando la red pierde paquetes. Un tope elegido por
/// debajo (el perfil de 16 kbps) manda sobre él.
const FLOOR_BITRATE: u32 = 24_000;

struct Tx {
    encoder: VoiceEncoder,
    samples: Vec<i16>,
    packet: [u8; MAX_PACKET],
    frames: u32,
    bitrate: u32,
}

struct Rx {
    decoder: VoiceDecoder,
    /// Secuencia del último frame entregado, para saber cuántos faltan.
    last: Option<u32>,
    samples: Vec<i16>,
    /// Realce de presencia y los dB para los que se calculó.
    presence: Biquad,
    presence_db: f32,
}

/// Camino de audio de una sesión.
pub struct Pipeline {
    session: Arc<Session>,
    sample_rate: u32,
    channels: u8,
    frame_ms: u8,
    /// Si el formato de la sesión es uno que el códec admite.
    format_ok: bool,
    /// Tope de tasa elegido por el usuario.
    bitrate_cap: AtomicU32,
    noise_suppression: AtomicBool,
    /// Código del perfil de voz, o [`NO_PROFILE`].
    profile: AtomicU8,
    tx: Mutex<Option<Tx>>,
    rx: Mutex<Option<Rx>>,
}

impl core::fmt::Debug for Pipeline {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Pipeline")
            .field("sample_rate", &self.sample_rate)
            .field("frame_ms", &self.frame_ms)
            .finish_non_exhaustive()
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    match m.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

impl Pipeline {
    pub fn new(
        session: Arc<Session>,
        sample_rate: u32,
        channels: u8,
        frame_ms: u8,
        max_bitrate: u32,
    ) -> Self {
        Self {
            session,
            sample_rate,
            channels,
            frame_ms,
            // Mono, y un sample rate y un frame de los de Opus. Se comprueba
            // una vez, creando un codificador de prueba.
            format_ok: channels == 1
                && VoiceEncoder::new(sample_rate, frame_ms, MAX_BITRATE).is_ok(),
            bitrate_cap: AtomicU32::new(max_bitrate.clamp(MIN_BITRATE, MAX_BITRATE)),
            noise_suppression: AtomicBool::new(true),
            profile: AtomicU8::new(NO_PROFILE),
            tx: Mutex::new(None),
            rx: Mutex::new(None),
        }
    }

    /// `true` si lo que viaja es voz comprimida.
    ///
    /// Hace falta que los dos extremos lo hayan negociado y que el formato sea
    /// uno que el códec admita (mono; estéreo se queda en PCM).
    pub fn compressed(&self) -> bool {
        self.format_ok && CodecId::from_code(self.session.negotiated_codec()) == CodecId::Opus
    }

    /// Ajustes de voz en vigor: los del perfil elegido, o los de siempre.
    fn tuning(&self) -> VoiceTuning {
        VoiceProfile::from_code(self.profile.load(Ordering::Acquire))
            .map_or(VoiceTuning::LEGACY, VoiceProfile::tuning)
    }

    /// Elige el perfil de voz. Fija de una vez el tratamiento, el tope de
    /// tasa y la redundancia; lo que se ajuste después por separado
    /// (reducción de ruido, tope de tasa) manda sobre el perfil.
    pub fn set_voice_profile(&self, profile: VoiceProfile) {
        self.profile.store(profile as u8, Ordering::Release);
        let tuning = profile.tuning();
        let cap = tuning.bitrate.clamp(MIN_BITRATE, MAX_BITRATE);
        self.bitrate_cap.store(cap, Ordering::Release);
        self.noise_suppression.store(true, Ordering::Release);
        if let Some(tx) = lock(&self.tx).as_mut() {
            let _ = tx.encoder.set_tuning(&tuning);
            tx.encoder.set_noise_suppression(true);
            tx.bitrate = tx.bitrate.min(cap);
            let _ = tx.encoder.set_bitrate(tx.bitrate);
        }
    }

    pub fn set_noise_suppression(&self, enabled: bool) {
        self.noise_suppression.store(enabled, Ordering::Release);
        if let Some(tx) = lock(&self.tx).as_mut() {
            tx.encoder.set_noise_suppression(enabled);
        }
    }

    /// Fija el tope de tasa de bits. La tasa real puede ser menor si la red
    /// pierde paquetes.
    pub fn set_bitrate_cap(&self, bitrate: u32) {
        let cap = bitrate.clamp(MIN_BITRATE, MAX_BITRATE);
        self.bitrate_cap.store(cap, Ordering::Release);
        if let Some(tx) = lock(&self.tx).as_mut() {
            tx.bitrate = tx.bitrate.min(cap);
            let _ = tx.encoder.set_bitrate(tx.bitrate);
        }
    }

    /// Tasa de bits a la que se está codificando, o `0` si viaja PCM.
    pub fn current_bitrate(&self) -> u32 {
        lock(&self.tx).as_ref().map_or(0, |tx| tx.bitrate)
    }

    /// Olvida el estado del códec: tras cerrar o rearmar la sesión, la
    /// siguiente conversación empieza limpia.
    pub fn reset(&self) {
        *lock(&self.tx) = None;
        *lock(&self.rx) = None;
    }

    /// Envía un frame PCM (16 bits, little-endian).
    pub async fn send_pcm(&self, pcm: &[u8]) -> Result<(), TransportError> {
        if !self.compressed() {
            return self.send_raw(pcm).await;
        }
        // Se codifica con el cerrojo cogido y se envía sin él: el envío es
        // asíncrono y el cerrojo no debe cruzar un `await`.
        let packet = {
            let mut slot = lock(&self.tx);
            if slot.is_none() {
                let cap = self.bitrate_cap.load(Ordering::Acquire);
                let mut encoder = VoiceEncoder::new(self.sample_rate, self.frame_ms, cap)
                    .map_err(|_| TransportError::InvalidState("voice encoder"))?;
                encoder
                    .set_tuning(&self.tuning())
                    .map_err(|_| TransportError::InvalidState("voice tuning"))?;
                encoder.set_noise_suppression(self.noise_suppression.load(Ordering::Acquire));
                *slot = Some(Tx {
                    samples: Vec::with_capacity(encoder.frame_samples()),
                    encoder,
                    packet: [0u8; MAX_PACKET],
                    frames: 0,
                    bitrate: cap,
                });
            }
            let Some(tx) = slot.as_mut() else {
                return Err(TransportError::InvalidState("voice encoder"));
            };
            tx.samples.clear();
            tx.samples.extend(
                pcm.as_chunks::<2>()
                    .0
                    .iter()
                    .map(|b| i16::from_le_bytes(*b)),
            );
            tx.frames = tx.frames.wrapping_add(1);
            if tx.frames.is_multiple_of(ADAPT_EVERY) {
                self.adapt(tx);
            }
            let Tx {
                encoder,
                samples,
                packet,
                ..
            } = tx;
            let n = encoder
                .encode(samples, packet)
                .map_err(|_| TransportError::InvalidState("voice frame size"))?;
            // Silencio dentro del turno: Opus avisa con un paquete mínimo de
            // que no hay nada que mandar. Como la secuencia de audio sólo
            // avanza al enviar, el receptor no lo ve como una pérdida.
            if n <= DTX_PACKET && encoder.tuning().dtx {
                return Ok(());
            }
            let encoded = packet[..n].to_vec();
            drop(slot);
            encoded
        };
        self.session.send_audio(&packet).await
    }

    /// Ajusta el códec a cómo va la red.
    ///
    /// Con pérdidas se baja la tasa (paquetes más pequeños, menos carga sobre
    /// un enlace que ya no da abasto) y se le dice al codificador que dedique
    /// más bits a la copia de respaldo. Sin pérdidas se vuelve a subir poco a
    /// poco hasta el tope. Es cambiar calidad por solidez cuando hace falta, y
    /// sólo entonces.
    fn adapt(&self, tx: &mut Tx) {
        let loss = self.session.metrics().loss.loss_percent();
        let cap = self.bitrate_cap.load(Ordering::Acquire);
        let scaled = |factor: f32| (tx.bitrate as f32 * factor) as u32;
        let target = if loss > 8.0 {
            scaled(0.7)
        } else if loss > 3.0 {
            scaled(0.85)
        } else if loss < 1.0 {
            scaled(1.15)
        } else {
            tx.bitrate
        };
        tx.bitrate = target.clamp(FLOOR_BITRATE.min(cap), cap);
        let _ = tx.encoder.set_bitrate(tx.bitrate);
        // La copia de respaldo no añade bytes: se los quita a la voz. En una
        // red que no pierde nada es calidad tirada, salvo que el perfil la
        // pida siempre.
        let expected = if loss < 1.0 && !tx.encoder.tuning().fec_always {
            0
        } else {
            (loss * 1.5).clamp(5.0, 40.0) as u8
        };
        let _ = tx.encoder.set_expected_loss(expected);
    }

    /// PCM sin comprimir. Un frame que no cabe en un paquete se parte en
    /// trozos iguales: el receptor los reproduce seguidos.
    async fn send_raw(&self, pcm: &[u8]) -> Result<(), TransportError> {
        let limit = MAX_PAYLOAD_SIZE - FRAME_OVERHEAD;
        if pcm.len() <= limit {
            return self.session.send_audio(pcm).await;
        }
        let align = usize::from(self.channels.max(1)) * 2;
        let parts = pcm.len().div_ceil(limit);
        let size = (pcm.len().div_ceil(parts)).next_multiple_of(align);
        for chunk in pcm.chunks(size) {
            self.session.send_audio(chunk).await?;
        }
        Ok(())
    }

    /// Recibe el siguiente tramo de audio, ya en PCM.
    ///
    /// Puede traer más de un frame: si antes de éste se perdió alguno, llegan
    /// delante los reconstruidos.
    pub async fn recv_pcm(&self) -> Result<Vec<u8>, TransportError> {
        let frame = self.session.recv_audio().await?;
        if !self.compressed() {
            return Ok(frame.payload.to_vec());
        }
        let mut slot = lock(&self.rx);
        if slot.is_none() {
            let decoder = VoiceDecoder::new(self.sample_rate, self.frame_ms)
                .map_err(|_| TransportError::InvalidState("voice decoder"))?;
            *slot = Some(Rx {
                samples: vec![0i16; decoder.frame_samples()],
                decoder,
                last: None,
                presence: Biquad::peaking(self.sample_rate, PRESENCE_HZ, PRESENCE_Q, 0.0),
                presence_db: 0.0,
            });
        }
        let Some(rx) = slot.as_mut() else {
            return Err(TransportError::InvalidState("voice decoder"));
        };
        let missing = rx
            .last
            .map_or(0, |last| frame.sequence.wrapping_sub(last).wrapping_sub(1));
        rx.last = Some(frame.sequence);
        // Un salto enorme (o hacia atrás) no es una pérdida que tapar: es otra
        // conversación o un reinicio de secuencia.
        let missing = if missing > 1_000 {
            0
        } else {
            missing.min(MAX_CONCEALED)
        };

        // El realce es cosa de quien escucha: sale de su perfil, no del de
        // quien habla, y no cuesta nada en la red.
        let presence_db = self.tuning().presence_db;
        if (presence_db - rx.presence_db).abs() > f32::EPSILON {
            rx.presence = Biquad::peaking(self.sample_rate, PRESENCE_HZ, PRESENCE_Q, presence_db);
            rx.presence_db = presence_db;
        }
        let Rx {
            decoder,
            samples: scratch,
            presence,
            ..
        } = rx;
        let boost = presence_db != 0.0;

        let mut out = Vec::with_capacity((missing as usize + 1) * scratch.len() * 2);
        let mut append = |samples: &[i16], out: &mut Vec<u8>| {
            for &s in samples {
                let s = if boost {
                    presence.step(f32::from(s)).clamp(-32_768.0, 32_767.0) as i16
                } else {
                    s
                };
                out.extend_from_slice(&s.to_le_bytes());
            }
        };
        // De los que faltan, el último se reconstruye con la copia que trae
        // este paquete; los anteriores sólo se pueden disimular.
        for _ in 1..missing {
            if let Ok(n) = decoder.conceal(scratch) {
                append(&scratch[..n], &mut out);
            }
        }
        if missing >= 1 {
            if let Ok(n) = decoder.recover_previous(&frame.payload, scratch) {
                append(&scratch[..n], &mut out);
            }
        }
        match decoder.decode(&frame.payload, scratch) {
            Ok(n) => append(&scratch[..n], &mut out),
            // Un paquete que no decodifica se trata como perdido: mejor una
            // ocultación que cortar la sesión por un frame.
            Err(_) => {
                if let Ok(n) = decoder.conceal(scratch) {
                    append(&scratch[..n], &mut out);
                }
            }
        }
        drop(slot);
        Ok(out)
    }
}
