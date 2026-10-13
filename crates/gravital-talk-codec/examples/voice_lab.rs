//! Pasa una grabación de voz por cada perfil y deja el resultado para oírlo.
//!
//! ```bash
//! arecord -f S16_LE -r 48000 -c 1 -d 15 voz.wav
//! cargo run --release -p gravital-talk-codec --no-default-features --features voice \
//!     --example voice_lab -- voz.wav salida/
//! ```
//!
//! Cada WAV de salida es lo que oiría el otro extremo: la voz tratada,
//! codificada, decodificada y con el realce del receptor. La tabla dice cuánto
//! viajaría por la red. No hay red de por medio: aquí no se pierde nada.

use std::path::Path;

use gravital_talk_codec::voice::{DTX_PACKET, MAX_PACKET};
use gravital_talk_codec::voice_dsp::Biquad;
use gravital_talk_codec::{VoiceDecoder, VoiceEncoder, VoiceProfile, VoiceTuning};

const RATE: u32 = 48_000;
const FRAME_MS: u8 = 20;
const FRAME: usize = 960;

fn dbfs(value: f64) -> f64 {
    20.0 * (value.max(1.0) / 32_768.0).log10()
}

fn rms(samples: &[i16]) -> f64 {
    let sum: f64 = samples.iter().map(|&s| f64::from(s).powi(2)).sum();
    (sum / samples.len().max(1) as f64).sqrt()
}

fn read(path: &str) -> Result<Vec<i16>, String> {
    let mut reader = hound::WavReader::open(path).map_err(|e| format!("{path}: {e}"))?;
    let spec = reader.spec();
    if spec.sample_rate != RATE || spec.bits_per_sample != 16 {
        return Err(format!(
            "{path}: hace falta 48 kHz y 16 bits (es {} Hz, {} bits). \
             Graba con: arecord -f S16_LE -r 48000 -c 1 -d 15 voz.wav",
            spec.sample_rate, spec.bits_per_sample
        ));
    }
    let all: Vec<i16> = reader
        .samples::<i16>()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("{path}: {e}"))?;
    let ch = usize::from(spec.channels.max(1));
    // Varios canales se mezclan a mono, como hace la captura de la app.
    Ok(all
        .chunks_exact(ch)
        .map(|c| (c.iter().map(|&s| i32::from(s)).sum::<i32>() / ch as i32) as i16)
        .collect())
}

fn write(path: &Path, samples: &[i16]) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(path, spec).map_err(|e| e.to_string())?;
    for &s in samples {
        writer.write_sample(s).map_err(|e| e.to_string())?;
    }
    writer.finalize().map_err(|e| e.to_string())
}

struct Outcome {
    audio: Vec<i16>,
    bytes: usize,
    skipped: usize,
    frames: usize,
}

fn run(input: &[i16], tuning: &VoiceTuning) -> Result<Outcome, String> {
    let mut enc =
        VoiceEncoder::new(RATE, FRAME_MS, tuning.bitrate).map_err(|e| format!("{e:?}"))?;
    enc.set_tuning(tuning).map_err(|e| format!("{e:?}"))?;
    let mut dec = VoiceDecoder::new(RATE, FRAME_MS).map_err(|e| format!("{e:?}"))?;
    let mut presence = Biquad::peaking(RATE, 3_000.0, 0.9, tuning.presence_db);
    let mut packet = [0u8; MAX_PACKET];
    let mut pcm = [0i16; FRAME];
    let mut out = Outcome {
        audio: Vec::with_capacity(input.len()),
        bytes: 0,
        skipped: 0,
        frames: 0,
    };
    for frame in input.as_chunks::<FRAME>().0 {
        let mut frame = frame.to_vec();
        let n = enc
            .encode(&mut frame, &mut packet)
            .map_err(|e| format!("{e:?}"))?;
        out.frames += 1;
        if n <= DTX_PACKET && tuning.dtx {
            // No se transmite: el receptor no tiene nada que reproducir.
            out.skipped += 1;
            out.audio.extend_from_slice(&[0i16; FRAME]);
            continue;
        }
        out.bytes += n;
        let got = dec
            .decode(&packet[..n], &mut pcm)
            .map_err(|e| format!("{e:?}"))?;
        if tuning.presence_db != 0.0 {
            presence.process(&mut pcm[..got]);
        }
        out.audio.extend_from_slice(&pcm[..got]);
    }
    Ok(out)
}

fn main() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, out_dir] = args.as_slice() else {
        return Err("uso: voice_lab <voz.wav> <carpeta de salida>".into());
    };
    let samples = read(input)?;
    if samples.len() < FRAME * 50 {
        return Err("la grabación es demasiado corta: hace falta al menos un segundo".into());
    }
    let out_dir = Path::new(out_dir);
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let seconds = (samples.len() / FRAME * FRAME) as f64 / f64::from(RATE);

    let before = VoiceTuning {
        bitrate: 128_000,
        ..VoiceTuning::LEGACY
    };
    let cases: [(&str, &str, VoiceTuning); 5] = [
        ("0-antes", "Antes (128 kbps)", before),
        ("1-voz-clara", "Voz clara", VoiceProfile::Clear.tuning()),
        ("2-llamada", "Llamada", VoiceProfile::Call.tuning()),
        ("3-ruidoso", "Entorno ruidoso", VoiceProfile::Noisy.tuning()),
        ("4-ligero", "Ligero", VoiceProfile::Light.tuning()),
    ];

    println!(
        "entrada: {seconds:.1} s · nivel {:.1} dBFS · pico {:.1} dBFS\n",
        dbfs(rms(&samples)),
        dbfs(
            samples
                .iter()
                .map(|s| f64::from(s.unsigned_abs()))
                .fold(0.0, f64::max)
        ),
    );
    println!(
        "{:<18} {:>8} {:>10} {:>10} {:>10} {:>10}",
        "perfil", "kbps", "MB/min", "sin enviar", "nivel dB", "pico dB"
    );
    for (file, name, tuning) in &cases {
        let result = run(&samples, tuning)?;
        let kbps = result.bytes as f64 * 8.0 / seconds / 1000.0;
        // A la red va además la cabecera de cada paquete: protocolo (24 B),
        // secuencia y sello (20 B), UDP e IP (28 B). Y la sesión añade un
        // paquete de paridad por cada tres o cuatro de audio: medido en una
        // conversación real son 65 paquetes por segundo, no 50.
        let sent = result.frames - result.skipped;
        let wire = (result.bytes + sent * 72) as f64 * 1.3 * 8.0 / seconds;
        let peak = result
            .audio
            .iter()
            .map(|s| f64::from(s.unsigned_abs()))
            .fold(0.0, f64::max);
        println!(
            "{:<18} {:>8.1} {:>10.2} {:>9.0}% {:>10.1} {:>10.1}",
            name,
            kbps,
            wire * 60.0 / 8.0 / 1_000_000.0,
            100.0 * result.skipped as f64 / result.frames as f64,
            dbfs(rms(&result.audio)),
            dbfs(peak),
        );
        write(&out_dir.join(format!("{file}.wav")), &result.audio)?;
    }
    println!("\nWAV de cada perfil en {}", out_dir.display());
    Ok(())
}
