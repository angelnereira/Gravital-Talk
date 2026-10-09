//! Análisis de señal PCM16 y escritura de WAV.
//!
//! El objetivo es probar que el audio *sobrevivió* al pipeline (UDP, cifrado,
//! FEC, jitter buffer, codec) y no sólo que llegaron datagramas. La fuente de
//! prueba del harness es una senoidal de 440 Hz, así que un pico a esa
//! frecuencia en el PCM recibido demuestra que las muestras llegaron
//! intactas y en orden.

use std::path::Path;

use anyhow::{bail, Context, Result};

/// Estadísticas de una señal PCM16 mono (o del primer canal si es
/// entrelazada).
#[derive(Debug, Clone, Copy)]
pub struct AudioStats {
    /// Número total de muestras (contando todos los canales).
    pub samples: usize,
    pub sample_rate: u32,
    pub channels: u16,
    /// Duración en milisegundos.
    pub duration_ms: f64,
    /// Valor cuadrático medio normalizado a [-1, 1].
    pub rms: f64,
    /// Amplitud máxima normalizada a [-1, 1].
    pub peak: f64,
    /// Frecuencia estimada por conteo de cruces de cero.
    pub freq_hz: f64,
    pub zero_crossings: usize,
    /// `true` si la señal es esencialmente plana.
    pub silent: bool,
}

impl AudioStats {
    /// Analiza `samples`. Usa el primer canal cuando el audio es entrelazado:
    /// para un tono senoidal todos los canales son idénticos.
    pub fn analyze(samples: &[i16], sample_rate: u32, channels: u16) -> Self {
        let channels = channels.max(1) as usize;
        let mono: Vec<i16> = if channels == 1 {
            samples.to_vec()
        } else {
            samples.iter().step_by(channels).copied().collect()
        };

        let mut sum_sq = 0.0f64;
        let mut peak = 0.0f64;
        let mut crossings = 0usize;
        let mut prev_sign = 0i32;

        for &s in &mono {
            let v = f64::from(s) / 32768.0;
            sum_sq = v.mul_add(v, sum_sq);
            peak = peak.max(v.abs());

            let sign = if s > 0 {
                1
            } else if s < 0 {
                -1
            } else {
                prev_sign // una muestra en cero exacto no cuenta como cambio
            };
            if sign != 0 {
                if prev_sign != 0 && sign != prev_sign {
                    crossings += 1;
                }
                prev_sign = sign;
            }
        }

        let n = mono.len().max(1) as f64;
        let duration_ms = (mono.len() as f64 / sample_rate.max(1) as f64) * 1000.0;
        let rms = (sum_sq / n).sqrt();
        // Un tono senoidal puro cruza cero dos veces por ciclo.
        let freq_hz = if duration_ms > 0.0 {
            (crossings as f64 / 2.0) / (duration_ms / 1000.0)
        } else {
            0.0
        };

        Self {
            samples: samples.len(),
            sample_rate,
            channels: channels as u16,
            duration_ms,
            rms,
            peak,
            freq_hz,
            zero_crossings: crossings,
            silent: rms < f64::EPSILON,
        }
    }

    /// Verifica que la señal sea un tono audible de la frecuencia esperada.
    ///
    /// Esto es lo que distingue una transmisión real de un flujo de bytes
    /// corruptos: los datos alterados no conservan ni la frecuencia ni el RMS.
    pub fn expect_tone(&self, expected_hz: f64, tolerance_hz: f64, min_rms: f64) -> Result<()> {
        if self.samples == 0 {
            bail!("no se recibió ninguna muestra de audio");
        }
        if self.silent || self.rms < min_rms {
            bail!(
                "audio recibido prácticamente silencio: rms={:.6} (mínimo {:.6}), {} muestras",
                self.rms,
                min_rms,
                self.samples
            );
        }
        let delta = (self.freq_hz - expected_hz).abs();
        if delta > tolerance_hz {
            bail!(
                "frecuencia inesperada: medida {:.1} Hz, esperada {:.1} Hz (+/- {:.1}), \
                 rms={:.4}, cruces de cero={}",
                self.freq_hz,
                expected_hz,
                tolerance_hz,
                self.rms,
                self.zero_crossings
            );
        }
        if self.peak > 1.0 {
            bail!("audio saturado: peak={:.3}", self.peak);
        }
        Ok(())
    }
}

/// Escribe `samples` como WAV PCM16 de 16 bits.
///
/// Si el códec de la sesión fue Opus, el PCM recibido llega ya decodificado y
/// se guarda en ese estado para poder analizarlo.
pub fn write_wav(path: &Path, samples: &[i16], sample_rate: u32, channels: u16) -> Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("cannot create dir {}", parent.display()))?;
        }
    }
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec)
        .with_context(|| format!("cannot create wav {}", path.display()))?;
    for &s in samples {
        writer.write_sample(s)?;
    }
    writer
        .finalize()
        .with_context(|| format!("cannot finalize wav {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Genera un tono senoidal puro de `freq_hz`.
    fn tone(freq_hz: f64, ms: u64, sample_rate: u32) -> Vec<i16> {
        let n = (sample_rate as f64 * ms as f64 / 1000.0) as usize;
        (0..n)
            .map(|i| {
                let phase = 2.0 * std::f64::consts::PI * freq_hz * i as f64 / sample_rate as f64;
                (phase.sin() * 16_000.0) as i16
            })
            .collect()
    }

    #[test]
    fn detects_440_hz_exactly() {
        let s = tone(440.0, 500, 48_000);
        let a = AudioStats::analyze(&s, 48_000, 1);
        assert!((a.freq_hz - 440.0).abs() < 2.0, "freq was {}", a.freq_hz);
        // El conteo exacto depende de los extremos del buffer, asi que se
        // comprueba con margen.
        assert!(
            (a.zero_crossings as i64 - 440).abs() <= 2,
            "crossings were {}",
            a.zero_crossings
        );
        assert!((a.rms - 0.353_553).abs() < 0.01, "rms was {}", a.rms);
        assert!(a.peak > 0.48 && a.peak < 0.5, "peak was {}", a.peak);
        assert_eq!(a.duration_ms, 500.0);
        a.expect_tone(440.0, 5.0, 0.01).unwrap();
    }

    #[test]
    fn distinguishes_two_frequencies() {
        // Base de la detección de cross-talk entre sesiones: si dos sesiones
        // comparten línea, el par receptor mide la frecuencia del emisor
        // equivocado.
        let a = AudioStats::analyze(&tone(400.0, 400, 48_000), 48_000, 1);
        let b = AudioStats::analyze(&tone(500.0, 400, 48_000), 48_000, 1);
        assert!((a.freq_hz - 400.0).abs() < 2.0);
        assert!((b.freq_hz - 500.0).abs() < 2.0);
        // Y cada uno rechaza la frecuencia del otro.
        assert!(a.expect_tone(500.0, 5.0, 0.01).is_err());
        assert!(b.expect_tone(400.0, 5.0, 0.01).is_err());
    }

    #[test]
    fn rejects_silence_and_empty() {
        let silence = vec![0i16; 9_600];
        let a = AudioStats::analyze(&silence, 48_000, 1);
        assert!(a.silent);
        assert_eq!(a.zero_crossings, 0);
        let err = a.expect_tone(440.0, 5.0, 0.01).unwrap_err().to_string();
        assert!(err.contains("silencio"), "unexpected error: {err}");

        let empty = AudioStats::analyze(&[], 48_000, 1);
        assert!(empty.expect_tone(440.0, 5.0, 0.01).is_err());
    }

    #[test]
    fn rejects_wrong_frequency_and_noise() {
        // Ruido pseudoaleatorio: no es un tono, debe fallar el criterio.
        let mut noise = Vec::with_capacity(9_600);
        let mut x: u32 = 12_345;
        for _ in 0..9_600 {
            x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            noise.push(((x >> 16) as i16) & 0x3fff);
        }
        let a = AudioStats::analyze(&noise, 48_000, 1);
        assert!(a.expect_tone(440.0, 5.0, 0.01).is_err());

        let a = AudioStats::analyze(&tone(300.0, 200, 48_000), 48_000, 1);
        assert!(a.expect_tone(440.0, 5.0, 0.01).is_err());
    }

    #[test]
    fn handles_interleaved_stereo() {
        // 2 canales con el mismo tono: el análisis usa el primer canal.
        // 500 ms para que la resolución por cruce de cero sea suficiente.
        let mono = tone(440.0, 500, 48_000);
        let mut stereo = Vec::with_capacity(mono.len() * 2);
        for &s in &mono {
            stereo.push(s);
            stereo.push(s);
        }
        let a = AudioStats::analyze(&stereo, 48_000, 2);
        assert!((a.freq_hz - 440.0).abs() < 3.0, "freq was {}", a.freq_hz);
        assert_eq!(a.channels, 2);
    }

    #[test]
    fn writes_readable_wav() {
        let s = tone(440.0, 200, 48_000);
        let path = std::env::temp_dir().join(format!(
            "gs-audit-test-{}-{}.wav",
            std::process::id(),
            line!()
        ));
        write_wav(&path, &s, 48_000, 1).unwrap();

        let reader = hound::WavReader::open(&path).unwrap();
        assert_eq!(reader.spec().sample_rate, 48_000);
        assert_eq!(reader.spec().channels, 1);
        assert_eq!(reader.spec().bits_per_sample, 16);
        let roundtrip: Vec<i16> = reader
            .into_samples::<i16>()
            .collect::<hound::Result<Vec<i16>>>()
            .expect("wav samples must be readable");
        assert_eq!(roundtrip.len(), s.len());
        assert_eq!(roundtrip, s);
        std::fs::remove_file(&path).unwrap();
    }
}
