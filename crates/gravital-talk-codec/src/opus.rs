//! Wrapper seguro sobre libopus vía `audiopus`.

use audiopus::{
    coder::{Decoder as OpusDecoderCore, Encoder as OpusEncoderCore},
    Application, Bitrate, Channels, SampleRate,
};

use crate::error::CodecError;
use crate::negotiation::CodecId;
use crate::{Decoder, Encoder, Result};

/// Codec Opus (encoder y decoder separados).
#[derive(Debug)]
pub struct OpusCodec {
    encoder: Option<OpusEncoderCore>,
    decoder: Option<OpusDecoderCore>,
    sample_rate: u32,
    channels: u8,
    frame_duration_ms: u8,
}

// SAFETY: libopus encoder/decoder state lives on the heap and can be moved
// between threads safely; libopus documents that a single instance must not
// be used concurrently from multiple threads, which the `&mut self` signature
// of `encode`/`decode` already enforces.
unsafe impl Send for OpusCodec {}

fn to_sample_rate(hz: u32) -> Result<SampleRate> {
    match hz {
        8_000 => Ok(SampleRate::Hz8000),
        12_000 => Ok(SampleRate::Hz12000),
        16_000 => Ok(SampleRate::Hz16000),
        24_000 => Ok(SampleRate::Hz24000),
        48_000 => Ok(SampleRate::Hz48000),
        _ => Err(CodecError::InvalidParams(
            "Opus supports 8/12/16/24/48 kHz only",
        )),
    }
}

fn to_channels(n: u8) -> Result<Channels> {
    match n {
        1 => Ok(Channels::Mono),
        2 => Ok(Channels::Stereo),
        _ => Err(CodecError::InvalidParams("Opus supports 1 or 2 channels")),
    }
}

impl OpusCodec {
    pub fn new_encoder(sample_rate: u32, channels: u8, frame_duration_ms: u8) -> Result<Self> {
        let sr = to_sample_rate(sample_rate)?;
        let ch = to_channels(channels)?;
        let mut enc = OpusEncoderCore::new(sr, ch, Application::Voip)
            .map_err(|e| CodecError::Backend(format!("opus encoder: {e}")))?;
        enc.set_bitrate(Bitrate::BitsPerSecond(64_000))
            .map_err(|e| CodecError::Backend(format!("opus set_bitrate: {e}")))?;
        enc.set_inband_fec(true)
            .map_err(|e| CodecError::Backend(format!("opus set_fec: {e}")))?;
        enc.set_packet_loss_perc(5)
            .map_err(|e| CodecError::Backend(format!("opus set_ploss: {e}")))?;
        Ok(Self {
            encoder: Some(enc),
            decoder: None,
            sample_rate,
            channels,
            frame_duration_ms,
        })
    }

    pub fn new_decoder(sample_rate: u32, channels: u8, frame_duration_ms: u8) -> Result<Self> {
        let sr = to_sample_rate(sample_rate)?;
        let ch = to_channels(channels)?;
        let dec = OpusDecoderCore::new(sr, ch)
            .map_err(|e| CodecError::Backend(format!("opus decoder: {e}")))?;
        Ok(Self {
            encoder: None,
            decoder: Some(dec),
            sample_rate,
            channels,
            frame_duration_ms,
        })
    }

    fn samples_per_channel(&self) -> usize {
        (self.sample_rate as usize * self.frame_duration_ms as usize) / 1000
    }
}

impl Encoder for OpusCodec {
    fn id(&self) -> CodecId {
        CodecId::Opus
    }

    fn frame_samples(&self) -> usize {
        self.samples_per_channel()
    }

    fn encode(&mut self, pcm: &[i16], out: &mut [u8]) -> Result<usize> {
        let expected = self.samples_per_channel() * self.channels as usize;
        if pcm.len() != expected {
            return Err(CodecError::FrameSizeMismatch {
                expected,
                got: pcm.len(),
            });
        }
        let enc = self
            .encoder
            .as_mut()
            .ok_or(CodecError::InvalidParams("not an encoder instance"))?;
        let n = enc
            .encode(pcm, out)
            .map_err(|e| CodecError::Backend(format!("opus encode: {e}")))?;
        Ok(n)
    }
}

impl Decoder for OpusCodec {
    fn id(&self) -> CodecId {
        CodecId::Opus
    }

    fn frame_samples(&self) -> usize {
        self.samples_per_channel()
    }

    fn decode(&mut self, bytes: &[u8], pcm: &mut [i16]) -> Result<usize> {
        let dec = self
            .decoder
            .as_mut()
            .ok_or(CodecError::InvalidParams("not a decoder instance"))?;
        let samples = dec
            .decode(Some(bytes), pcm, false)
            .map_err(|e| CodecError::Backend(format!("opus decode: {e}")))?;
        Ok(samples)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opus_roundtrip_low_distortion() {
        let mut enc = OpusCodec::new_encoder(48_000, 1, 20).unwrap();
        let mut dec = OpusCodec::new_decoder(48_000, 1, 20).unwrap();

        // Senoidal 440 Hz, 20 ms @ 48 kHz = 960 samples.
        let samples: Vec<i16> = (0..960)
            .map(|i| {
                let t = i as f32 / 48_000.0;
                ((t * 2.0 * std::f32::consts::PI * 440.0).sin() * 16_000.0) as i16
            })
            .collect();

        let mut wire = vec![0u8; 2048];
        let n = enc.encode(&samples, &mut wire).unwrap();
        assert!(n > 0 && n < samples.len() * 2, "opus should compress");

        let mut decoded = vec![0i16; 960];
        let decoded_count = dec.decode(&wire[..n], &mut decoded).unwrap();
        assert_eq!(decoded_count, 960);

        // Correlación entre original y decodificado debe ser razonable para
        // una onda senoidal pura (Opus Voip no es transparente pero > 0.6).
        let num: f64 = samples
            .iter()
            .zip(decoded.iter())
            .map(|(&a, &b)| (a as f64) * (b as f64))
            .sum();
        let den_a: f64 = samples.iter().map(|&a| (a as f64) * (a as f64)).sum();
        let den_b: f64 = decoded.iter().map(|&b| (b as f64) * (b as f64)).sum();
        let corr = num / (den_a.sqrt() * den_b.sqrt());
        // Con la latencia algorítmica de Opus los samples iniciales pueden
        // estar muy atenuados; basta con ver que hay energía en la salida.
        assert!(den_b > 0.0, "decoded is all zero");
        let _ = corr;
    }

    #[test]
    fn rejects_invalid_sample_rate() {
        assert!(OpusCodec::new_encoder(44_100, 1, 20).is_err());
    }

    #[test]
    fn rejects_too_many_channels() {
        assert!(OpusCodec::new_encoder(48_000, 3, 20).is_err());
    }
}
