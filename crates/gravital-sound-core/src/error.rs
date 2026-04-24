//! Tipos de error del core. `no_std` compatible.

use core::fmt;

/// Error del core. Todas las variantes son enumerables desde la FFI via
/// [`Error::code`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// Buffer demasiado corto para contener un paquete.
    TooShort,
    /// `magic` no coincide con `"GS"`.
    BadMagic,
    /// `version` distinto a [`crate::PROTOCOL_VERSION`].
    ProtocolMismatch,
    /// `payload_len` declarado no concuerda con el buffer real.
    LengthMismatch,
    /// CRC-16 incorrecto.
    BadChecksum,
    /// `msg_type` no reconocido.
    UnknownMessageType(u8),
    /// Buffer de salida demasiado pequeño para encode.
    BufferTooSmall,
    /// `payload_len` excede el máximo permitido.
    PayloadTooLarge,
    /// Demasiados fragmentos.
    TooManyFragments,
    /// Índice de fragmento fuera de rango.
    FragmentOutOfRange,
    /// Fragmento duplicado.
    DuplicateFragment,
    /// Reassembly incompleto al pop.
    IncompleteReassembly,
    /// Transición de estado inválida.
    InvalidStateTransition,
    /// Payload malformado (estructura interna).
    MalformedPayload,
}

impl Error {
    /// Código numérico estable para la FFI.
    #[must_use]
    pub const fn code(&self) -> u16 {
        match self {
            Self::TooShort => 1,
            Self::BadMagic => 2,
            Self::ProtocolMismatch => 3,
            Self::LengthMismatch => 4,
            Self::BadChecksum => 5,
            Self::UnknownMessageType(_) => 6,
            Self::BufferTooSmall => 7,
            Self::PayloadTooLarge => 8,
            Self::TooManyFragments => 9,
            Self::FragmentOutOfRange => 10,
            Self::DuplicateFragment => 11,
            Self::IncompleteReassembly => 12,
            Self::InvalidStateTransition => 13,
            Self::MalformedPayload => 14,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort => f.write_str("buffer shorter than header size"),
            Self::BadMagic => f.write_str("magic bytes mismatch"),
            Self::ProtocolMismatch => f.write_str("unsupported protocol version"),
            Self::LengthMismatch => f.write_str("payload length does not match buffer"),
            Self::BadChecksum => f.write_str("CRC-16 checksum mismatch"),
            Self::UnknownMessageType(t) => write!(f, "unknown message type: 0x{t:02X}"),
            Self::BufferTooSmall => f.write_str("output buffer too small"),
            Self::PayloadTooLarge => f.write_str("payload exceeds maximum size"),
            Self::TooManyFragments => f.write_str("fragment count exceeds maximum"),
            Self::FragmentOutOfRange => f.write_str("fragment index out of range"),
            Self::DuplicateFragment => f.write_str("duplicate fragment received"),
            Self::IncompleteReassembly => f.write_str("cannot pop incomplete reassembly"),
            Self::InvalidStateTransition => f.write_str("invalid session state transition"),
            Self::MalformedPayload => f.write_str("malformed payload structure"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}
