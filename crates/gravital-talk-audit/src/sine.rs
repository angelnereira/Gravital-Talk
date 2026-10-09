//! Fuente de audio de prueba.
//!
//! Sustituye al micrófono para que la auditoría no dependa de hardware: una
//! senoidal de frecuencia conocida y separada del ruido de fondo. Como el
//! receptor analiza la frecuencia de lo que recibe, esta señal es la que
//! demuestra que el audio cruzó el pipeline intacto.

/// Genera frames PCM16 de un tono senoidal continuo.
///
/// `samples_per_frame` es el número de muestras **por canal**: el frame
/// completo lleva `samples_per_frame * channels` muestras entrelazadas, que es
/// lo que exige `CodecSession::send_samples`.
///
/// `freq_hz` es lo que el receptor acaba midiendo: por eso el harness usa
/// frecuencias distintas por sesión para detectar cruce de tráfico.
pub fn sine_frames_i16(
    samples_per_frame: usize,
    channels: u8,
    sample_rate: u32,
    freq_hz: f64,
) -> impl Iterator<Item = Vec<i16>> {
    let mut phase: f64 = 0.0;
    let step = 2.0 * std::f64::consts::PI * freq_hz / sample_rate as f64;
    std::iter::from_fn(move || {
        let mut buf = Vec::with_capacity(samples_per_frame * channels.max(1) as usize);
        for _ in 0..samples_per_frame {
            let sample = (phase.sin() * 16_000.0) as i16;
            for _c in 0..channels {
                buf.push(sample);
            }
            phase += step;
            if phase > std::f64::consts::TAU {
                phase -= std::f64::consts::TAU;
            }
        }
        Some(buf)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::AudioStats;

    #[test]
    fn produced_frames_analyze_to_the_requested_frequency() {
        let frame = 960; // 20 ms a 48 kHz mono
        let frames: Vec<Vec<i16>> = sine_frames_i16(frame, 1, 48_000, 440.0).take(50).collect();
        assert_eq!(frames.len(), 50);
        for f in &frames {
            assert_eq!(f.len(), frame);
        }

        let flat: Vec<i16> = frames.concat();
        let a = AudioStats::analyze(&flat, 48_000, 1);
        assert!(
            (a.freq_hz - 440.0).abs() < 2.0,
            "generated freq was {}",
            a.freq_hz
        );
        assert_eq!(a.duration_ms, 1000.0);
        a.expect_tone(440.0, 5.0, 0.01).unwrap();
    }

    #[test]
    fn different_frequencies_are_distinguishable() {
        let take = |hz: f64| -> Vec<i16> {
            sine_frames_i16(960, 1, 48_000, hz)
                .take(25)
                .flatten()
                .collect()
        };
        let a = AudioStats::analyze(&take(400.0), 48_000, 1);
        let b = AudioStats::analyze(&take(500.0), 48_000, 1);
        assert!((a.freq_hz - 400.0).abs() < 2.0, "{}", a.freq_hz);
        assert!((b.freq_hz - 500.0).abs() < 2.0, "{}", b.freq_hz);
    }

    #[test]
    fn interleaves_all_channels() {
        // 960 muestras por canal x 2 canales = 1920 muestras por frame.
        let frames: Vec<Vec<i16>> = sine_frames_i16(960, 2, 48_000, 440.0).take(3).collect();
        for f in &frames {
            assert_eq!(f.len(), 1920);
            // Canal izquierdo == canal derecho.
            assert_eq!(f[0], f[1]);
        }
    }
}
