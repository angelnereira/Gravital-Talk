//! Codec PCM signed 16-bit little-endian — passthrough sin compresión.
//!
//! Útil como baseline, para depuración, y para cuando la red es de baja
//! latencia y alta calidad (LAN, loopback).

use crate::error::CodecError;
use crate::negotiation::CodecId;
use crate::{Decoder, Encoder, Result};

#[derive(Debug, Clone, Copy)]
pub struct PcmCodec {
    sample_rate: u32,
    channels: u8,
    frame_duration_ms: u8,
}

impl PcmCodec {
    #[must_use]
    pub const fn new(sample_rate: u32, channels: u8, frame_duration_ms: u8) -> Self {
        Self {
            sample_rate,
            channels,
            frame_duration_ms,
        }
    }

    const fn samples_per_channel(&self) -> usize {
        (self.sample_rate as usize * self.frame_duration_ms as usize) / 1000
    }

    const fn total_samples(&self) -> usize {
        self.samples_per_channel() * self.channels as usize
    }

    fn wire_bytes(&self) -> usize {
        self.total_samples() * 2
    }
}

impl Encoder for PcmCodec {
    fn id(&self) -> CodecId {
        CodecId::Pcm
    }

    fn frame_samples(&self) -> usize {
        self.samples_per_channel()
    }

    fn encode(&mut self, pcm: &[i16], out: &mut [u8]) -> Result<usize> {
        let needed = self.wire_bytes();
        if pcm.len() != self.total_samples() {
            return Err(CodecError::FrameSizeMismatch {
                expected: self.total_samples(),
                got: pcm.len(),
            });
        }
        if out.len() < needed {
            return Err(CodecError::BufferTooSmall {
                needed,
                have: out.len(),
            });
        }
        for (i, &sample) in pcm.iter().enumerate() {
            let bytes = sample.to_le_bytes();
            out[i * 2] = bytes[0];
            out[i * 2 + 1] = bytes[1];
        }
        Ok(needed)
    }
}

impl Decoder for PcmCodec {
    fn id(&self) -> CodecId {
        CodecId::Pcm
    }

    fn frame_samples(&self) -> usize {
        self.samples_per_channel()
    }

    fn decode(&mut self, bytes: &[u8], pcm: &mut [i16]) -> Result<usize> {
        if bytes.len() % 2 != 0 {
            return Err(CodecError::InvalidParams("PCM payload length not even"));
        }
        let samples = bytes.len() / 2;
        if pcm.len() < samples {
            return Err(CodecError::BufferTooSmall {
                needed: samples,
                have: pcm.len(),
            });
        }
        for i in 0..samples {
            pcm[i] = i16::from_le_bytes([bytes[i * 2], bytes[i * 2 + 1]]);
        }
        Ok(samples / self.channels.max(1) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_is_identity() {
        let mut enc = PcmCodec::new(48_000, 1, 10);
        let mut dec = PcmCodec::new(48_000, 1, 10);
        let samples: Vec<i16> = (0..480).map(|i| (i as i16).wrapping_mul(67)).collect();
        let mut wire = vec![0u8; 2048];
        let n = enc.encode(&samples, &mut wire).unwrap();
        assert_eq!(n, 960);
        let mut out = vec![0i16; 480];
        let decoded = dec.decode(&wire[..n], &mut out).unwrap();
        assert_eq!(decoded, 480);
        assert_eq!(out, samples);
    }

    #[test]
    fn rejects_wrong_frame_size() {
        let mut enc = PcmCodec::new(48_000, 1, 10);
        let too_short = vec![0i16; 100];
        let mut wire = vec![0u8; 2048];
        assert!(enc.encode(&too_short, &mut wire).is_err());
    }

    #[test]
    fn rejects_small_buffer() {
        let mut enc = PcmCodec::new(48_000, 1, 10);
        let samples = vec![0i16; 480];
        let mut wire = vec![0u8; 100];
        assert!(enc.encode(&samples, &mut wire).is_err());
    }
}
