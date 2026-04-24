//! Errores del transporte.

use gravital_sound_core::error::Error as CoreError;
use thiserror::Error;

/// Errores que puede devolver cualquier `Transport`.
#[derive(Debug, Error)]
pub enum TransportError {
    /// Error de I/O subyacente (socket UDP, TLS, WebSocket).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// El peer no respondió dentro del deadline configurado.
    #[error("operation timed out")]
    Timeout,

    /// El transporte ya está cerrado y no acepta nuevas operaciones.
    #[error("transport is closed")]
    Closed,

    /// El buffer de destino es demasiado pequeño.
    #[error("buffer too small: need {needed}, have {have}")]
    BufferTooSmall { needed: usize, have: usize },

    /// Error al decodificar o validar el protocolo.
    #[error("protocol error: {0}")]
    Protocol(#[from] CoreError),

    /// Handshake falló (timeout, mismatch de versión, etc.).
    #[error("handshake failed: {0}")]
    Handshake(&'static str),

    /// Se intentó enviar en un estado inválido.
    #[error("invalid state: {0}")]
    InvalidState(&'static str),

    /// El peer envió un `Error` o cerró la sesión.
    #[error("peer closed session: {0}")]
    PeerClosed(&'static str),
}

impl TransportError {
    /// Código numérico estable para la FFI.
    #[must_use]
    pub const fn code(&self) -> i32 {
        match self {
            Self::Io(_) => -1,
            Self::Timeout => -2,
            Self::Closed => -3,
            Self::BufferTooSmall { .. } => -4,
            Self::Protocol(_) => -5,
            Self::Handshake(_) => -6,
            Self::InvalidState(_) => -7,
            Self::PeerClosed(_) => -8,
        }
    }
}
