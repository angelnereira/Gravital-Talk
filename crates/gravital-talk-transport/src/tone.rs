//! Generación de tonos PTT — señales audibles de "press" y "release".
//!
//! Los tonos se sintetizan como PCM i16 mono sin dependencias externas.
//! Son compatibles con `Session::send_audio()` directamente.

pub const PTT_PRESS_FREQ_HZ: f32 = 880.0;
pub const PTT_RELEASE_FREQ_HZ: f32 = 440.0;
pub const PTT_PRESS_DURATION_MS: u32 = 100;
pub const PTT_RELEASE_DURATION_MS: u32 = 80;

/// Genera muestras PCM i16 de una onda sinusoidal.
///
/// El volumen se limita a 20 000 (≈61 % de i16::MAX) para evitar clipping.
pub fn generate_pcm_tone(freq_hz: f32, duration_ms: u32, sample_rate: u32) -> Vec<i16> {
    let n_samples = (sample_rate as u64 * duration_ms as u64 / 1000) as usize;
    let step = 2.0 * std::f32::consts::PI * freq_hz / sample_rate as f32;
    let mut phase = 0.0f32;
    let mut out = Vec::with_capacity(n_samples);
    for _ in 0..n_samples {
        out.push((phase.sin() * 20_000.0) as i16);
        phase += step;
        if phase > std::f32::consts::TAU {
            phase -= std::f32::consts::TAU;
        }
    }
    out
}

/// Convierte muestras i16 a bytes little-endian para `send_audio()`.
pub fn pcm_to_bytes(samples: &[i16]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for &s in samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    out
}

/// Tono de 880 Hz / 100 ms — se emite al presionar PTT.
pub fn ptt_press_tone(sample_rate: u32) -> Vec<u8> {
    pcm_to_bytes(&generate_pcm_tone(
        PTT_PRESS_FREQ_HZ,
        PTT_PRESS_DURATION_MS,
        sample_rate,
    ))
}

/// Tono de 440 Hz / 80 ms — se emite al soltar PTT.
pub fn ptt_release_tone(sample_rate: u32) -> Vec<u8> {
    pcm_to_bytes(&generate_pcm_tone(
        PTT_RELEASE_FREQ_HZ,
        PTT_RELEASE_DURATION_MS,
        sample_rate,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tone_length_correct() {
        let sr = 48_000u32;
        let samples = generate_pcm_tone(440.0, 100, sr);
        assert_eq!(samples.len(), 4_800);

        let samples80 = generate_pcm_tone(880.0, 80, sr);
        assert_eq!(samples80.len(), 3_840);
    }

    #[test]
    fn pcm_to_bytes_roundtrip() {
        let samples = vec![0i16, i16::MAX, i16::MIN, 1234, -1234];
        let bytes = pcm_to_bytes(&samples);
        assert_eq!(bytes.len(), samples.len() * 2);
        let decoded: Vec<i16> = bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        assert_eq!(decoded, samples);
    }

    #[test]
    fn press_and_release_tones_have_different_lengths() {
        let press = ptt_press_tone(48_000);
        let release = ptt_release_tone(48_000);
        assert_ne!(press.len(), release.len());
        // press = 100 ms @ 48 kHz × 2 bytes = 9 600 bytes
        assert_eq!(press.len(), 9_600);
        // release = 80 ms @ 48 kHz × 2 bytes = 7 680 bytes
        assert_eq!(release.len(), 7_680);
    }

    #[test]
    fn tone_amplitude_in_range() {
        let samples = generate_pcm_tone(440.0, 10, 48_000);
        for s in samples {
            assert!(s >= -20_001 && s <= 20_001);
        }
    }
}
