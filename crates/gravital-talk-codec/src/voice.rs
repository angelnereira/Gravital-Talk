//! Códec de voz para conversación: Opus con corrección de pérdidas y
//! reducción de ruido, todo en Rust.
//!
//! ## Por qué existe además de [`OpusCodec`](crate::OpusCodec)
//!
//! `OpusCodec` enlaza con libopus en C. Eso lo deja fuera justo de donde más
//! falta hace: la librería que usan las apps se compila para Android con
//! cargo-ndk, y ahí la compilación cruzada de libopus falla. El resultado era
//! que las apps mandaban PCM sin comprimir: unos 800 kbps, un paquete de 1 KB
//! cada 10 ms. En una red móvil eso es tarifa, latencia y, sobre todo,
//! fragilidad: cada paquete perdido es un hueco en el audio.
//!
//! Este módulo usa `unsafe-libopus` (el libopus de referencia traducido a Rust)
//! y `nnnoiseless` (RNNoise), así que compila igual para todos los targets.
//!
//! ## Qué aporta frente a mandar PCM
//!
//! - **Tamaño**: de ~800 kbps a 32–128 kbps. Paquetes de 80–320 B en vez de
//!   1 KB, y la mitad de paquetes por segundo (frames de 20 ms).
//! - **Tolerancia a pérdidas**, con dos mecanismos del propio Opus:
//!   - *FEC en banda*: cada paquete lleva una copia reducida del anterior. Si
//!     se pierde uno, se reconstruye a partir del siguiente
//!     ([`VoiceDecoder::recover_previous`]).
//!   - *Ocultación*: si se pierden varios seguidos, el decodificador sintetiza
//!     una continuación plausible ([`VoiceDecoder::conceal`]) en vez de dejar
//!     silencio o un chasquido.
//! - **Reducción de ruido** antes de codificar: lo que no es voz ni se oye ni
//!   gasta bits.

use nnnoiseless::DenoiseState;
use unsafe_libopus::{
    opus_decode, opus_decoder_create, opus_decoder_destroy, opus_encode, opus_encoder_create,
    opus_encoder_ctl, opus_encoder_destroy, OpusDecoder, OpusEncoder, OPUS_APPLICATION_VOIP,
    OPUS_SET_BITRATE_REQUEST, OPUS_SET_COMPLEXITY_REQUEST, OPUS_SET_INBAND_FEC_REQUEST,
    OPUS_SET_PACKET_LOSS_PERC_REQUEST, OPUS_SET_SIGNAL_REQUEST, OPUS_SIGNAL_VOICE,
};

use crate::error::CodecError;
use crate::Result;

/// Tasa mínima a la que se deja bajar el codificador.
pub const MIN_BITRATE: u32 = 16_000;

/// Tasa máxima admitida.
pub const MAX_BITRATE: u32 = 128_000;

/// Tamaño máximo de un paquete codificado.
///
/// A 128 kbps un frame de 20 ms son 320 B; el margen cubre los picos del modo
/// de tasa variable.
pub const MAX_PACKET: usize = 600;

/// Complejidad del codificador (0–10). Cinco es el punto en el que un móvil
/// codifica sin esfuerzo y la calidad ya está cerca del máximo.
const COMPLEXITY: i32 = 5;

/// Probabilidad de voz a partir de la cual un bloque se da por hablado.
const VOICE_THRESHOLD: f32 = 0.5;

/// Bloques de 10 ms que la puerta sigue abierta tras la última voz, para no
/// recortar finales de palabra ni las pausas entre sílabas.
const GATE_HANGOVER: u32 = 30;

/// Ganancia con la puerta cerrada. No es cero a propósito: un silencio digital
/// absoluto entre frases suena a línea cortada.
const GATE_FLOOR: f32 = 0.12;

/// Cuánto se acerca la ganancia a su objetivo en cada bloque al cerrar. Abrir
/// es inmediato: retrasarlo se comería el arranque de cada palabra.
const GATE_RELEASE: f32 = 0.25;

/// Tamaño de frame de RNNoise: 10 ms a 48 kHz.
const DENOISE_FRAME: usize = DenoiseState::FRAME_SIZE;

const fn valid_rate(sample_rate: u32) -> bool {
    matches!(sample_rate, 8_000 | 12_000 | 16_000 | 24_000 | 48_000)
}

fn backend(what: &'static str, code: i32) -> CodecError {
    CodecError::Backend(format!("opus {what}: código {code}"))
}

/// Codificador de voz mono.
pub struct VoiceEncoder {
    raw: *mut OpusEncoder,
    frame_samples: usize,
    /// Reductor de ruido, si el formato lo permite (48 kHz, frames múltiplo
    /// de 10 ms).
    denoise: Option<Box<DenoiseState<'static>>>,
    denoise_enabled: bool,
    /// Ganancia actual de la puerta de voz y bloques que le quedan abierta.
    gate_gain: f32,
    gate_hold: u32,
    scratch_in: [f32; DENOISE_FRAME],
    scratch_out: [f32; DENOISE_FRAME],
}

// SAFETY: el estado de Opus es memoria propia de este objeto, sin referencias
// a hilo ni a almacenamiento local; moverlo entre hilos es mover un puntero
// con un único dueño. No es `Sync`: todo acceso pasa por `&mut self`.
unsafe impl Send for VoiceEncoder {}

impl core::fmt::Debug for VoiceEncoder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VoiceEncoder")
            .field("frame_samples", &self.frame_samples)
            .field("denoise", &self.denoise_enabled)
            .finish_non_exhaustive()
    }
}

impl VoiceEncoder {
    /// Crea un codificador para frames de `frame_ms` a `sample_rate` Hz.
    pub fn new(sample_rate: u32, frame_ms: u8, bitrate: u32) -> Result<Self> {
        if !valid_rate(sample_rate) {
            return Err(CodecError::InvalidParams(
                "sample rate no admitido por Opus",
            ));
        }
        if !matches!(frame_ms, 10 | 20 | 40 | 60) {
            return Err(CodecError::InvalidParams(
                "frame de Opus: 10, 20, 40 o 60 ms",
            ));
        }
        let frame_samples = sample_rate as usize * usize::from(frame_ms) / 1000;

        let mut err = 0i32;
        // SAFETY: argumentos válidos (comprobados arriba) y `err` es un `i32`
        // escribible. Devuelve nulo si falla, que se comprueba.
        let raw =
            unsafe { opus_encoder_create(sample_rate as i32, 1, OPUS_APPLICATION_VOIP, &mut err) };
        if raw.is_null() || err != 0 {
            return Err(backend("encoder_create", err));
        }
        let denoise = (sample_rate == 48_000 && frame_samples.is_multiple_of(DENOISE_FRAME))
            .then(DenoiseState::new);
        let mut encoder = Self {
            raw,
            frame_samples,
            denoise,
            denoise_enabled: true,
            gate_gain: 1.0,
            gate_hold: GATE_HANGOVER,
            scratch_in: [0.0; DENOISE_FRAME],
            scratch_out: [0.0; DENOISE_FRAME],
        };
        encoder.ctl(OPUS_SET_SIGNAL_REQUEST, OPUS_SIGNAL_VOICE)?;
        encoder.ctl(OPUS_SET_COMPLEXITY_REQUEST, COMPLEXITY)?;
        // La copia de respaldo del frame anterior sólo se incluye si el
        // codificador espera pérdidas: se parte de un 10 %.
        encoder.ctl(OPUS_SET_INBAND_FEC_REQUEST, 1)?;
        encoder.set_expected_loss(10)?;
        encoder.set_bitrate(bitrate)?;
        Ok(encoder)
    }

    fn ctl(&mut self, request: i32, value: i32) -> Result<()> {
        // SAFETY: `self.raw` es un codificador vivo creado en `new`; las
        // peticiones usadas toman un único `i32`.
        let rc = unsafe { opus_encoder_ctl!(self.raw, request, value) };
        if rc == 0 {
            Ok(())
        } else {
            Err(backend("encoder_ctl", rc))
        }
    }

    /// Muestras por frame que espera [`Self::encode`].
    #[must_use]
    pub const fn frame_samples(&self) -> usize {
        self.frame_samples
    }

    /// Fija la tasa de bits, acotada a [`MIN_BITRATE`]..=[`MAX_BITRATE`].
    pub fn set_bitrate(&mut self, bitrate: u32) -> Result<()> {
        let bitrate = bitrate.clamp(MIN_BITRATE, MAX_BITRATE);
        self.ctl(OPUS_SET_BITRATE_REQUEST, bitrate as i32)
    }

    /// Le dice al codificador qué porcentaje de pérdida esperar.
    ///
    /// Con más pérdida esperada dedica más bits a la copia de respaldo y
    /// menos a la calidad del frame: es el intercambio de calidad por solidez.
    pub fn set_expected_loss(&mut self, percent: u8) -> Result<()> {
        self.ctl(
            OPUS_SET_PACKET_LOSS_PERC_REQUEST,
            i32::from(percent.min(50)),
        )
    }

    /// Activa o desactiva la reducción de ruido. No hace nada si el formato
    /// no la admite.
    pub const fn set_noise_suppression(&mut self, enabled: bool) {
        self.denoise_enabled = enabled;
    }

    /// `true` si la reducción de ruido está disponible y activa.
    #[must_use]
    pub const fn noise_suppression(&self) -> bool {
        self.denoise_enabled && self.denoise.is_some()
    }

    /// Codifica un frame. `pcm` se modifica: queda con el ruido reducido.
    ///
    /// Devuelve los bytes escritos en `out`.
    pub fn encode(&mut self, pcm: &mut [i16], out: &mut [u8]) -> Result<usize> {
        if pcm.len() != self.frame_samples {
            return Err(CodecError::FrameSizeMismatch {
                expected: self.frame_samples,
                got: pcm.len(),
            });
        }
        if self.denoise_enabled {
            if let Some(denoise) = self.denoise.as_mut() {
                for block in pcm.as_chunks_mut::<DENOISE_FRAME>().0 {
                    // RNNoise trabaja en `f32` con la escala de `i16`.
                    for (dst, src) in self.scratch_in.iter_mut().zip(block.iter()) {
                        *dst = f32::from(*src);
                    }
                    let voice = denoise.process_frame(&mut self.scratch_out, &self.scratch_in);
                    // Puerta de voz sobre lo ya filtrado. RNNoise atenúa el
                    // ruido que acompaña a la voz, pero con sólo ruido deja
                    // pasar bastante; su propia estimación de "aquí hay voz"
                    // sirve para bajar el resto.
                    if voice >= VOICE_THRESHOLD {
                        self.gate_hold = GATE_HANGOVER;
                        self.gate_gain = 1.0;
                    } else if self.gate_hold > 0 {
                        self.gate_hold -= 1;
                    } else {
                        self.gate_gain =
                            (GATE_FLOOR - self.gate_gain).mul_add(GATE_RELEASE, self.gate_gain);
                    }
                    let gain = self.gate_gain;
                    for (dst, src) in block.iter_mut().zip(self.scratch_out.iter()) {
                        *dst = (src * gain).clamp(-32768.0, 32767.0) as i16;
                    }
                }
            }
        }
        // SAFETY: `self.raw` es un codificador vivo; `pcm` tiene exactamente
        // `frame_samples` muestras mono y `out` tiene `out.len()` bytes
        // escribibles, que es el tope que se le pasa.
        let written = unsafe {
            opus_encode(
                self.raw,
                pcm.as_ptr(),
                self.frame_samples as i32,
                out.as_mut_ptr(),
                out.len().min(i32::MAX as usize) as i32,
            )
        };
        if written < 0 {
            return Err(backend("encode", written));
        }
        Ok(written as usize)
    }
}

impl Drop for VoiceEncoder {
    fn drop(&mut self) {
        // SAFETY: `self.raw` lo creó `opus_encoder_create` y nadie más lo
        // libera.
        unsafe { opus_encoder_destroy(self.raw) };
    }
}

/// Decodificador de voz mono.
pub struct VoiceDecoder {
    raw: *mut OpusDecoder,
    frame_samples: usize,
}

// SAFETY: igual que `VoiceEncoder`: memoria propia con un único dueño.
unsafe impl Send for VoiceDecoder {}

impl core::fmt::Debug for VoiceDecoder {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VoiceDecoder")
            .field("frame_samples", &self.frame_samples)
            .finish_non_exhaustive()
    }
}

impl VoiceDecoder {
    /// Crea un decodificador que entrega frames de `frame_ms` a `sample_rate`.
    pub fn new(sample_rate: u32, frame_ms: u8) -> Result<Self> {
        if !valid_rate(sample_rate) {
            return Err(CodecError::InvalidParams(
                "sample rate no admitido por Opus",
            ));
        }
        let frame_samples = sample_rate as usize * usize::from(frame_ms) / 1000;
        let mut err = 0i32;
        // SAFETY: argumentos válidos y `err` es un `i32` escribible.
        let raw = unsafe { opus_decoder_create(sample_rate as i32, 1, &mut err) };
        if raw.is_null() || err != 0 {
            return Err(backend("decoder_create", err));
        }
        Ok(Self { raw, frame_samples })
    }

    /// Muestras por frame que produce cada llamada.
    #[must_use]
    pub const fn frame_samples(&self) -> usize {
        self.frame_samples
    }

    fn run(&mut self, packet: Option<&[u8]>, out: &mut [i16], fec: bool) -> Result<usize> {
        if out.len() < self.frame_samples {
            return Err(CodecError::BufferTooSmall {
                needed: self.frame_samples,
                have: out.len(),
            });
        }
        let (data, len) = packet.map_or((core::ptr::null(), 0), |p| (p.as_ptr(), p.len() as i32));
        // SAFETY: `self.raw` es un decodificador vivo; `data`/`len` describen
        // un slice válido (o nulo/0, que Opus entiende como "paquete
        // perdido"); `out` tiene al menos `frame_samples` muestras
        // escribibles, que es lo que se le dice.
        let samples = unsafe {
            opus_decode(
                self.raw,
                data,
                len,
                out.as_mut_ptr(),
                self.frame_samples as i32,
                i32::from(fec),
            )
        };
        if samples < 0 {
            return Err(backend("decode", samples));
        }
        Ok(samples as usize)
    }

    /// Decodifica un paquete recibido.
    pub fn decode(&mut self, packet: &[u8], out: &mut [i16]) -> Result<usize> {
        self.run(Some(packet), out, false)
    }

    /// Reconstruye el frame **anterior** a `next_packet`, que se perdió, a
    /// partir de la copia de respaldo que éste lleva.
    ///
    /// Hay que llamarlo antes de decodificar `next_packet`. Si el paquete no
    /// trae copia, Opus sintetiza una ocultación en su lugar.
    pub fn recover_previous(&mut self, next_packet: &[u8], out: &mut [i16]) -> Result<usize> {
        self.run(Some(next_packet), out, true)
    }

    /// Sintetiza un frame para tapar uno perdido del que no hay copia.
    pub fn conceal(&mut self, out: &mut [i16]) -> Result<usize> {
        self.run(None, out, false)
    }
}

impl Drop for VoiceDecoder {
    fn drop(&mut self) {
        // SAFETY: `self.raw` lo creó `opus_decoder_create` y nadie más lo
        // libera.
        unsafe { opus_decoder_destroy(self.raw) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;
    const FRAME: usize = 960;

    /// Un frame de tono a `hz`, continuando la fase en `phase`.
    fn tone(hz: f32, phase: &mut f32) -> Vec<i16> {
        (0..FRAME)
            .map(|_| {
                let s = (phase.sin() * 9000.0) as i16;
                *phase += core::f32::consts::TAU * hz / RATE as f32;
                s
            })
            .collect()
    }

    fn rms(samples: &[i16]) -> f64 {
        let sum: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
        (sum / samples.len() as f64).sqrt()
    }

    fn pair(bitrate: u32) -> (VoiceEncoder, VoiceDecoder) {
        let mut enc = VoiceEncoder::new(RATE, 20, bitrate).unwrap();
        // Con un tono puro el reductor de ruido lo trataría como ruido.
        enc.set_noise_suppression(false);
        (enc, VoiceDecoder::new(RATE, 20).unwrap())
    }

    #[test]
    fn comprime_al_orden_de_la_tasa_pedida() {
        for (bitrate, max_bytes) in [(32_000, 130), (64_000, 220), (128_000, 400)] {
            let (mut enc, _) = pair(bitrate);
            let mut phase = 0.0;
            let mut out = [0u8; MAX_PACKET];
            let mut total = 0;
            for _ in 0..50 {
                total += enc.encode(&mut tone(440.0, &mut phase), &mut out).unwrap();
            }
            let avg = total / 50;
            // Un frame PCM son 1920 B: cualquiera de las tres tasas tiene que
            // quedar muy por debajo.
            assert!(avg <= max_bytes, "{bitrate} bps: {avg} B por frame");
        }
    }

    #[test]
    fn lo_decodificado_conserva_la_energia_de_la_voz() {
        let (mut enc, mut dec) = pair(64_000);
        let mut phase = 0.0;
        let mut packet = [0u8; MAX_PACKET];
        let mut pcm = [0i16; FRAME];
        let mut level = 0.0;
        for i in 0..30 {
            let n = enc
                .encode(&mut tone(440.0, &mut phase), &mut packet)
                .unwrap();
            assert_eq!(dec.decode(&packet[..n], &mut pcm).unwrap(), FRAME);
            // Los primeros frames son el arranque del códec.
            if i >= 10 {
                level = rms(&pcm);
            }
        }
        // El tono de entrada tiene un RMS de ~6360.
        assert!((4000.0..9000.0).contains(&level), "rms {level}");
    }

    #[test]
    fn un_paquete_perdido_se_reconstruye_con_el_siguiente() {
        let (mut enc, mut dec) = pair(64_000);
        let mut phase = 0.0;
        let mut packets = Vec::new();
        for _ in 0..30 {
            let mut out = [0u8; MAX_PACKET];
            let n = enc.encode(&mut tone(440.0, &mut phase), &mut out).unwrap();
            packets.push(out[..n].to_vec());
        }
        let mut pcm = [0i16; FRAME];
        for packet in &packets[..20] {
            dec.decode(packet, &mut pcm).unwrap();
        }
        // El 20 se pierde. Con el 21 en la mano se recupera el 20...
        assert_eq!(dec.recover_previous(&packets[21], &mut pcm).unwrap(), FRAME);
        let recovered = rms(&pcm);
        // ...y luego se decodifica el 21 con normalidad.
        assert_eq!(dec.decode(&packets[21], &mut pcm).unwrap(), FRAME);
        // Lo reconstruido es señal, no silencio.
        assert!(recovered > 2000.0, "rms recuperado {recovered}");
    }

    #[test]
    fn varios_perdidos_seguidos_se_ocultan_sin_silencio_brusco() {
        let (mut enc, mut dec) = pair(64_000);
        let mut phase = 0.0;
        let mut packet = [0u8; MAX_PACKET];
        let mut pcm = [0i16; FRAME];
        for _ in 0..20 {
            let n = enc
                .encode(&mut tone(440.0, &mut phase), &mut packet)
                .unwrap();
            dec.decode(&packet[..n], &mut pcm).unwrap();
        }
        assert_eq!(dec.conceal(&mut pcm).unwrap(), FRAME);
        assert!(rms(&pcm) > 1000.0, "la ocultación no debe ser silencio");
    }

    #[test]
    fn la_reduccion_de_ruido_quita_un_ruido_de_fondo_grave() {
        // Ruido grave y continuo, como un ventilador o el motor de un coche:
        // no hay voz que conservar, así que tiene que salir muy por debajo de
        // como entró.
        //
        // No se prueba con ruido blanco a propósito: RNNoise lo toma por una
        // consonante sorda (probabilidad de voz ~0,5) y apenas lo toca. Es
        // una limitación del modelo, medida, no un fallo de esta integración.
        let mut enc = VoiceEncoder::new(RATE, 20, 64_000).unwrap();
        assert!(enc.noise_suppression());
        let mut seed = 0x1234_5678u32;
        let mut low = 0.0f32;
        let mut out = [0u8; MAX_PACKET];
        let (mut before, mut after) = (0.0, 0.0);
        for i in 0..300 {
            let mut frame: Vec<i16> = (0..FRAME)
                .map(|_| {
                    seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    let white = f32::from((seed >> 16) as i16) / 32768.0;
                    low = 0.97f32.mul_add(low, 0.03 * white * 18_000.0);
                    low as i16
                })
                .collect();
            let input = rms(&frame);
            enc.encode(&mut frame, &mut out).unwrap();
            // El reductor necesita unos segundos para estimar el ruido: se
            // mide el último.
            if i >= 250 {
                before += input / 50.0;
                after += rms(&frame) / 50.0;
            }
        }
        assert!(
            before > 800.0,
            "el ruido de prueba debe ser audible: {before:.0}"
        );
        assert!(after < before * 0.1, "ruido {before:.0} → {after:.0}");
    }

    #[test]
    fn sin_reduccion_de_ruido_el_audio_pasa_intacto() {
        let mut enc = VoiceEncoder::new(RATE, 20, 64_000).unwrap();
        enc.set_noise_suppression(false);
        assert!(!enc.noise_suppression());
        let mut phase = 0.0;
        let original = tone(440.0, &mut phase);
        let mut frame = original.clone();
        enc.encode(&mut frame, &mut [0u8; MAX_PACKET]).unwrap();
        assert_eq!(frame, original);
    }

    #[test]
    fn los_parametros_invalidos_se_rechazan() {
        assert!(VoiceEncoder::new(44_100, 20, 64_000).is_err());
        assert!(VoiceEncoder::new(48_000, 15, 64_000).is_err());
        let (mut enc, _) = pair(64_000);
        let mut short = [0i16; 100];
        assert!(enc.encode(&mut short, &mut [0u8; MAX_PACKET]).is_err());
    }
}
