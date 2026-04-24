//! Gravital Sound — capa de codec.
//!
//! Provee traits `Encoder`/`Decoder` y dos implementaciones:
//!
//! - [`pcm::PcmCodec`] — passthrough, i16 little-endian.
//! - [`opus::OpusCodec`] (detrás de feature `opus`) — wrapper seguro sobre
//!   libopus vía `audiopus`.
//!
//! La selección ocurre en el handshake según `CodecId`.

#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(missing_debug_implementations)]

pub mod error;
pub mod negotiation;
#[cfg(feature = "opus")]
pub mod opus;
pub mod pcm;

pub use error::CodecError;
pub use negotiation::CodecId;
pub use pcm::PcmCodec;

#[cfg(feature = "opus")]
pub use opus::OpusCodec;

/// Alias corto del resultado del crate.
pub type Result<T> = core::result::Result<T, CodecError>;

/// Codifica samples PCM signed 16-bit a bytes wire-ready.
///
/// El encoder no es `Sync`: las instancias libopus no son reentrantes. El
/// caller debe mantener acceso exclusivo (por ejemplo, dentro de un Mutex)
/// cuando comparte la misma instancia entre hilos.
pub trait Encoder: core::fmt::Debug + Send {
    /// Identificador de codec para el wire.
    fn id(&self) -> CodecId;

    /// Número de samples (por canal) por frame fijo que espera este encoder.
    fn frame_samples(&self) -> usize;

    /// Codifica `pcm` en `out`, devuelve bytes escritos.
    fn encode(&mut self, pcm: &[i16], out: &mut [u8]) -> Result<usize>;
}

/// Decodifica bytes wire a samples PCM signed 16-bit.
pub trait Decoder: core::fmt::Debug + Send {
    fn id(&self) -> CodecId;

    fn frame_samples(&self) -> usize;

    /// Decodifica `bytes` en `pcm`, devuelve samples escritos (por canal).
    fn decode(&mut self, bytes: &[u8], pcm: &mut [i16]) -> Result<usize>;
}

/// Construye un par encoder/decoder desde un `CodecId` y parámetros.
pub fn build_pair(
    id: CodecId,
    sample_rate: u32,
    channels: u8,
    frame_duration_ms: u8,
) -> Result<(Box<dyn Encoder>, Box<dyn Decoder>)> {
    match id {
        CodecId::Pcm => {
            let enc = PcmCodec::new(sample_rate, channels, frame_duration_ms);
            let dec = PcmCodec::new(sample_rate, channels, frame_duration_ms);
            Ok((Box::new(enc), Box::new(dec)))
        }
        #[cfg(feature = "opus")]
        CodecId::Opus => {
            let enc = OpusCodec::new_encoder(sample_rate, channels, frame_duration_ms)?;
            let dec = OpusCodec::new_decoder(sample_rate, channels, frame_duration_ms)?;
            Ok((Box::new(enc), Box::new(dec)))
        }
        #[cfg(not(feature = "opus"))]
        CodecId::Opus => Err(CodecError::Unsupported(id)),
        CodecId::Other(_) => Err(CodecError::Unsupported(id)),
    }
}
