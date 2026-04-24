//! Errores del crate.

use thiserror::Error;

use crate::negotiation::CodecId;

/// Error unificado para todos los codecs.
#[derive(Debug, Error)]
pub enum CodecError {
    /// El buffer de salida es demasiado pequeño.
    #[error("output buffer too small: need {needed}, have {have}")]
    BufferTooSmall { needed: usize, have: usize },

    /// Tamaño de frame incorrecto (el codec espera N samples exactos).
    #[error("frame size mismatch: expected {expected}, got {got}")]
    FrameSizeMismatch { expected: usize, got: usize },

    /// Parámetros de codec inválidos (sample rate, canales, frame ms).
    #[error("invalid codec parameters: {0}")]
    InvalidParams(&'static str),

    /// El codec requerido no está habilitado (feature flag).
    #[error("codec {0:?} is not supported by this build")]
    Unsupported(CodecId),

    /// Error interno del backend (libopus, etc.).
    #[error("backend error: {0}")]
    Backend(String),
}
