//! Identificación y negociación de codecs.
//!
//! El byte `codec_preferred` / `codec_accepted` del handshake mapea a esta
//! enum. Los valores wire son estables.

use crate::error::CodecError;

/// Codec identificador sobre el wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CodecId {
    /// PCM 16-bit little-endian, passthrough sin compresión.
    #[default]
    Pcm,
    /// Opus (RFC 6716) vía libopus.
    Opus,
    /// Código reservado para extensiones futuras.
    Other(u8),
}

impl CodecId {
    /// Byte en el handshake.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Pcm => 0x01,
            Self::Opus => 0x02,
            Self::Other(b) => b,
        }
    }

    /// Decodifica un byte.
    #[must_use]
    pub const fn from_code(code: u8) -> Self {
        match code {
            0x01 => Self::Pcm,
            0x02 => Self::Opus,
            other => Self::Other(other),
        }
    }
}

/// Lista local de codecs soportados en este build, en orden de preferencia.
#[must_use]
pub fn supported() -> &'static [CodecId] {
    #[cfg(feature = "opus")]
    {
        &[CodecId::Opus, CodecId::Pcm]
    }
    #[cfg(not(feature = "opus"))]
    {
        &[CodecId::Pcm]
    }
}

/// Dado el preferido del cliente, elige el mejor disponible localmente.
pub fn negotiate(client_preferred: CodecId) -> Result<CodecId, CodecError> {
    let local = supported();
    if local.contains(&client_preferred) {
        return Ok(client_preferred);
    }
    local
        .first()
        .copied()
        .ok_or(CodecError::Unsupported(client_preferred))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_roundtrip() {
        for id in [CodecId::Pcm, CodecId::Opus, CodecId::Other(0x40)] {
            assert_eq!(CodecId::from_code(id.code()), id);
        }
    }

    #[test]
    fn negotiate_prefers_client_if_supported() {
        let id = negotiate(CodecId::Pcm).unwrap();
        assert_eq!(id, CodecId::Pcm);
    }

    #[test]
    fn negotiate_falls_back_to_local_preferred() {
        let id = negotiate(CodecId::Other(0x40)).unwrap();
        assert!(matches!(id, CodecId::Pcm | CodecId::Opus));
    }
}
