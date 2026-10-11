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

/// Amplitud de los tonos de turno: presentes sin tapar la voz.
const TURN_TONE_PEAK: f32 = 0.25 * 32_767.0;

/// Sinusoide de `ms` milisegundos que va de `from_hz` a `to_hz`, con una
/// envolvente que la hace empezar y acabar en cero: sin ella, el corte en
/// seco se oye como un chasquido.
fn swept_pulse(from_hz: f32, to_hz: f32, ms: u32, sample_rate: u32, out: &mut Vec<i16>) {
    let n = (u64::from(sample_rate) * u64::from(ms) / 1000) as usize;
    let mut phase = 0.0f32;
    for i in 0..n {
        let t = i as f32 / n.max(1) as f32;
        let freq = (to_hz - from_hz).mul_add(t, from_hz);
        phase += std::f32::consts::TAU * freq / sample_rate as f32;
        // Semiperiodo de seno: 0 → 1 → 0 a lo largo del pulso.
        let envelope = (std::f32::consts::PI * t).sin();
        out.push((phase.sin() * envelope * TURN_TONE_PEAK) as i16);
    }
}

/// Tono de "empieza un turno": barrido ascendente corto (900 → 1400 Hz, 40 ms).
///
/// Los tonos de turno se reproducen **en local** en cada dispositivo, nunca
/// se envían por la red: el códec de voz y la reducción de ruido deforman o
/// eliminan un tono puro, y llegarían tarde o a trozos.
#[must_use]
pub fn turn_start_tone(sample_rate: u32) -> Vec<i16> {
    let mut out = Vec::with_capacity(sample_rate as usize / 20);
    swept_pulse(900.0, 1400.0, 40, sample_rate, &mut out);
    out
}

/// Tono de "termina el turno": dos pulsos descendentes (1200 y 800 Hz, 35 ms
/// cada uno) separados por 15 ms de silencio.
#[must_use]
pub fn turn_end_tone(sample_rate: u32) -> Vec<i16> {
    let mut out = Vec::with_capacity(sample_rate as usize / 10);
    swept_pulse(1200.0, 1200.0, 35, sample_rate, &mut out);
    out.resize(out.len() + sample_rate as usize * 15 / 1000, 0);
    swept_pulse(800.0, 800.0, 35, sample_rate, &mut out);
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
            .as_chunks::<2>()
            .0
            .iter()
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
    fn los_tonos_de_turno_duran_lo_justo_y_no_chasquean() {
        for rate in [16_000u32, 48_000] {
            let start = turn_start_tone(rate);
            let end = turn_end_tone(rate);
            assert_eq!(start.len(), rate as usize * 40 / 1000);
            assert_eq!(end.len(), rate as usize * 85 / 1000);
            for tone in [&start, &end] {
                // Empiezan y acaban prácticamente en cero.
                assert!(tone[0].abs() < 400, "inicio {}", tone[0]);
                assert!(tone[tone.len() - 1].abs() < 400);
                // Suenan, y lejos de saturar.
                let peak = tone.iter().map(|s| s.unsigned_abs()).max().unwrap();
                assert!((4_000..=8_200).contains(&peak), "pico {peak}");
            }
            // El silencio entre los dos pulsos del tono de cierre.
            let gap = rate as usize * 35 / 1000..rate as usize * 50 / 1000;
            assert!(end[gap].iter().all(|&s| s == 0));
        }
    }

    #[test]
    fn el_tono_de_inicio_sube_de_frecuencia() {
        // Cruces por cero en cada mitad: más en la segunda.
        let tone = turn_start_tone(48_000);
        let crossings = |s: &[i16]| s.windows(2).filter(|w| (w[0] < 0) != (w[1] < 0)).count();
        let (a, b) = tone.split_at(tone.len() / 2);
        assert!(
            crossings(b) > crossings(a),
            "{} vs {}",
            crossings(a),
            crossings(b)
        );
    }

    #[test]
    fn tone_amplitude_in_range() {
        let samples = generate_pcm_tone(440.0, 10, 48_000);
        for s in samples {
            assert!((-20_001..=20_001).contains(&s));
        }
    }
}
