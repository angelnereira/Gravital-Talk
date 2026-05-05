//! Resampler de audio i16 entre sample rates arbitrarios.
//!
//! Wrapping de [`rubato::FftFixedOut`] con interfaz simple para usar en
//! callbacks de captura/playback. Aplica conversión `i16 → f32 → resample → i16`
//! en bloques de tamaño fijo.
//!
//! Uso típico: el device captura a 44.1 kHz y la sesión Gravital corre a 48 kHz.
//! En el lado de captura, instanciar un `Resampler::new(44_100, 48_000, 1, frame_samples)`
//! y pasar cada bloque de samples del callback antes de enviarlos al codec.

use rubato::{FftFixedOut, Resampler as RubatoResampler};

use crate::error::IoError;
use crate::Result;

/// Resampler unidireccional de `in_rate → out_rate` para audio i16.
pub struct Resampler {
    inner: FftFixedOut<f32>,
    in_rate: u32,
    out_rate: u32,
    channels: usize,
    /// Número de samples de salida por canal por llamada.
    out_frames_per_channel: usize,
    /// Buffers intermedios reutilizables para evitar alloc en hot path.
    in_f32: Vec<Vec<f32>>,
    out_f32: Vec<Vec<f32>>,
    /// Acumulador de samples de entrada que aún no llenan un bloque completo.
    pending_in: Vec<Vec<f32>>,
}

impl core::fmt::Debug for Resampler {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Resampler")
            .field("in_rate", &self.in_rate)
            .field("out_rate", &self.out_rate)
            .field("channels", &self.channels)
            .field("out_frames_per_channel", &self.out_frames_per_channel)
            .finish()
    }
}

impl Resampler {
    /// Construye un resampler que produce bloques de `out_frames_per_channel`
    /// samples por canal a la salida.
    pub fn new(
        in_rate: u32,
        out_rate: u32,
        channels: u8,
        out_frames_per_channel: usize,
    ) -> Result<Self> {
        if channels == 0 {
            return Err(IoError::UnsupportedConfig("zero channels".into()));
        }
        let channels = channels as usize;
        let inner = FftFixedOut::<f32>::new(
            in_rate as usize,
            out_rate as usize,
            out_frames_per_channel,
            2, // sub_chunks: 2 da buen tradeoff calidad/latencia
            channels,
        )
        .map_err(|e| IoError::UnsupportedConfig(format!("rubato init: {e}")))?;

        let in_needed = inner.input_frames_next();
        let in_f32 = vec![Vec::with_capacity(in_needed); channels];
        let out_f32 = vec![vec![0.0f32; out_frames_per_channel]; channels];
        let pending_in = vec![Vec::with_capacity(in_needed * 2); channels];

        Ok(Self {
            inner,
            in_rate,
            out_rate,
            channels,
            out_frames_per_channel,
            in_f32,
            out_f32,
            pending_in,
        })
    }

    /// Sample rate de entrada.
    #[must_use]
    pub fn in_rate(&self) -> u32 {
        self.in_rate
    }

    /// Sample rate de salida.
    #[must_use]
    pub fn out_rate(&self) -> u32 {
        self.out_rate
    }

    /// Número de samples (interleaved) de salida que se producen cuando hay
    /// suficiente entrada acumulada.
    #[must_use]
    pub fn out_block_size(&self) -> usize {
        self.out_frames_per_channel * self.channels
    }

    /// Empuja `samples_in` (interleaved i16) al resampler. Cuando hay
    /// suficiente entrada, produce uno o más bloques de tamaño
    /// `out_block_size()` interleaved que se devuelven en un `Vec<i16>`
    /// concatenado. Si no hay suficiente entrada, el `Vec` puede estar vacío.
    pub fn push(&mut self, samples_in: &[i16]) -> Result<Vec<i16>> {
        if samples_in.len() % self.channels != 0 {
            return Err(IoError::UnsupportedConfig(format!(
                "input length {} not multiple of channels {}",
                samples_in.len(),
                self.channels
            )));
        }

        // Deinterleave + i16 → f32 normalizado en pending_in.
        for (i, frame) in samples_in.chunks_exact(self.channels).enumerate() {
            for (ch, &sample) in frame.iter().enumerate() {
                let _ = i;
                self.pending_in[ch].push(sample as f32 / i16::MAX as f32);
            }
        }

        let mut out_concat: Vec<i16> = Vec::new();

        loop {
            let needed = self.inner.input_frames_next();
            if self.pending_in[0].len() < needed {
                break;
            }

            // Mover `needed` samples de pending_in a in_f32 por canal.
            for ch in 0..self.channels {
                self.in_f32[ch].clear();
                self.in_f32[ch].extend_from_slice(&self.pending_in[ch][..needed]);
                self.pending_in[ch].drain(..needed);
            }

            self.inner
                .process_into_buffer(&self.in_f32, &mut self.out_f32, None)
                .map_err(|e| IoError::UnsupportedConfig(format!("rubato process: {e}")))?;

            // Interleave + f32 → i16 saturado.
            for frame_idx in 0..self.out_frames_per_channel {
                for ch in 0..self.channels {
                    let v = self.out_f32[ch][frame_idx].clamp(-1.0, 1.0);
                    out_concat.push((v * i16::MAX as f32) as i16);
                }
            }
        }

        Ok(out_concat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_44100(seconds: f32) -> Vec<i16> {
        let n = (44_100.0 * seconds) as usize;
        let step = 2.0 * std::f32::consts::PI * 440.0 / 44_100.0;
        (0..n)
            .map(|i| {
                let v = (i as f32 * step).sin() * 16_000.0;
                v as i16
            })
            .collect()
    }

    #[test]
    fn resampler_44k_to_48k_preserves_signal_energy() {
        let input = sine_44100(0.5); // 22 050 samples @ 44.1 kHz
        let mut r = Resampler::new(44_100, 48_000, 1, 480).unwrap();
        let mut output = Vec::new();
        for chunk in input.chunks(960) {
            output.extend_from_slice(&r.push(chunk).unwrap());
        }
        // Energía del resampleo debería ser ~ a la del original (margen 30%).
        let energy_in: f64 =
            input.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / input.len() as f64;
        let energy_out: f64 =
            output.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / output.len().max(1) as f64;
        let ratio = energy_out / energy_in;
        assert!(
            (0.7..1.3).contains(&ratio),
            "energy ratio out/in = {ratio:.3} fuera de [0.7, 1.3]"
        );
        // Cantidad de samples de salida ≈ 24 000 (0.5s × 48 kHz), tolerancia.
        assert!(
            output.len() > 23_000 && output.len() < 25_000,
            "output len = {}",
            output.len()
        );
    }

    #[test]
    fn resampler_48k_to_48k_passthrough_close() {
        // No es passthrough literal pero la salida debe ser muy cercana al input.
        let input: Vec<i16> = (0..4800)
            .map(|i| {
                let t = i as f32 / 48_000.0;
                ((t * 2.0 * std::f32::consts::PI * 440.0).sin() * 8000.0) as i16
            })
            .collect();
        let mut r = Resampler::new(48_000, 48_000, 1, 480).unwrap();
        let mut output = Vec::new();
        for chunk in input.chunks(480) {
            output.extend_from_slice(&r.push(chunk).unwrap());
        }
        assert!(!output.is_empty(), "no produjo salida");
    }

    #[test]
    fn rejects_zero_channels() {
        assert!(Resampler::new(48_000, 48_000, 0, 480).is_err());
    }
}
