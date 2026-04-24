//! Tipos de mensaje y payloads estructurados.

use crate::error::Error;

/// Códigos de `msg_type` del header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MessageType {
    HandshakeInit,
    HandshakeAccept,
    HandshakeConfirm,
    AudioFrame,
    AudioFragment,
    Heartbeat,
    HeartbeatAck,
    ControlPause,
    ControlResume,
    ControlMetrics,
    /// Rango `0x40..=0x7F` reservado para extensiones de aplicación.
    Extension(u8),
    Error,
    Close,
}

impl MessageType {
    /// Código numérico estable del mensaje (valor del byte `msg_type`).
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::HandshakeInit => 0x01,
            Self::HandshakeAccept => 0x02,
            Self::HandshakeConfirm => 0x03,
            Self::AudioFrame => 0x10,
            Self::AudioFragment => 0x11,
            Self::Heartbeat => 0x20,
            Self::HeartbeatAck => 0x21,
            Self::ControlPause => 0x30,
            Self::ControlResume => 0x31,
            Self::ControlMetrics => 0x32,
            Self::Extension(b) => b,
            Self::Error => 0xFE,
            Self::Close => 0xFF,
        }
    }

    /// Decodifica un byte en una variante válida.
    pub const fn from_code(code: u8) -> Result<Self, Error> {
        Ok(match code {
            0x01 => Self::HandshakeInit,
            0x02 => Self::HandshakeAccept,
            0x03 => Self::HandshakeConfirm,
            0x10 => Self::AudioFrame,
            0x11 => Self::AudioFragment,
            0x20 => Self::Heartbeat,
            0x21 => Self::HeartbeatAck,
            0x30 => Self::ControlPause,
            0x31 => Self::ControlResume,
            0x32 => Self::ControlMetrics,
            0x40..=0x7F => Self::Extension(code),
            0xFE => Self::Error,
            0xFF => Self::Close,
            other => return Err(Error::UnknownMessageType(other)),
        })
    }

    /// Si el mensaje es parte del handshake.
    #[must_use]
    pub const fn is_handshake(self) -> bool {
        matches!(
            self,
            Self::HandshakeInit | Self::HandshakeAccept | Self::HandshakeConfirm
        )
    }
}

/// Payload de `HANDSHAKE_INIT` (20 bytes). Ver `docs/protocol-spec.md` §6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandshakeInit {
    pub protocol_version: u8,
    pub codec_preferred: u8,
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_duration_ms: u8,
    pub max_bitrate: u32,
    pub capability_flags: u32,
    pub nonce: u32,
}

impl HandshakeInit {
    pub const SIZE: usize = 20;

    /// Codifica en `buf` (≥ 20 bytes).
    pub fn encode(&self, buf: &mut [u8]) -> Result<(), Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::BufferTooSmall);
        }
        buf[0] = self.protocol_version;
        buf[1] = self.codec_preferred;
        buf[2..6].copy_from_slice(&self.sample_rate.to_be_bytes());
        buf[6] = self.channels;
        buf[7] = self.frame_duration_ms;
        buf[8..12].copy_from_slice(&self.max_bitrate.to_be_bytes());
        buf[12..16].copy_from_slice(&self.capability_flags.to_be_bytes());
        buf[16..20].copy_from_slice(&self.nonce.to_be_bytes());
        Ok(())
    }

    /// Decodifica 20 bytes en un `HandshakeInit`.
    pub fn decode(buf: &[u8]) -> Result<Self, Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::MalformedPayload);
        }
        Ok(Self {
            protocol_version: buf[0],
            codec_preferred: buf[1],
            sample_rate: u32::from_be_bytes([buf[2], buf[3], buf[4], buf[5]]),
            channels: buf[6],
            frame_duration_ms: buf[7],
            max_bitrate: u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
            capability_flags: u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]),
            nonce: u32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]),
        })
    }
}

/// Payload de `HANDSHAKE_ACCEPT` (24 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandshakeAccept {
    pub protocol_version: u8,
    pub codec_accepted: u8,
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_duration_ms: u8,
    pub max_bitrate: u32,
    pub capability_flags: u32,
    pub nonce: u32,
    pub session_id: u32,
}

impl HandshakeAccept {
    pub const SIZE: usize = 24;

    pub fn encode(&self, buf: &mut [u8]) -> Result<(), Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::BufferTooSmall);
        }
        buf[0] = self.protocol_version;
        buf[1] = self.codec_accepted;
        buf[2..6].copy_from_slice(&self.sample_rate.to_be_bytes());
        buf[6] = self.channels;
        buf[7] = self.frame_duration_ms;
        buf[8..12].copy_from_slice(&self.max_bitrate.to_be_bytes());
        buf[12..16].copy_from_slice(&self.capability_flags.to_be_bytes());
        buf[16..20].copy_from_slice(&self.nonce.to_be_bytes());
        buf[20..24].copy_from_slice(&self.session_id.to_be_bytes());
        Ok(())
    }

    pub fn decode(buf: &[u8]) -> Result<Self, Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::MalformedPayload);
        }
        Ok(Self {
            protocol_version: buf[0],
            codec_accepted: buf[1],
            sample_rate: u32::from_be_bytes([buf[2], buf[3], buf[4], buf[5]]),
            channels: buf[6],
            frame_duration_ms: buf[7],
            max_bitrate: u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
            capability_flags: u32::from_be_bytes([buf[12], buf[13], buf[14], buf[15]]),
            nonce: u32::from_be_bytes([buf[16], buf[17], buf[18], buf[19]]),
            session_id: u32::from_be_bytes([buf[20], buf[21], buf[22], buf[23]]),
        })
    }
}

/// Payload de `HANDSHAKE_CONFIRM` (4 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandshakeConfirm {
    pub session_id: u32,
}

impl HandshakeConfirm {
    pub const SIZE: usize = 4;

    pub fn encode(&self, buf: &mut [u8]) -> Result<(), Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::BufferTooSmall);
        }
        buf[0..4].copy_from_slice(&self.session_id.to_be_bytes());
        Ok(())
    }

    pub fn decode(buf: &[u8]) -> Result<Self, Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::MalformedPayload);
        }
        Ok(Self {
            session_id: u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]),
        })
    }
}

/// Códigos de error enviados en payloads de `MessageType::Error`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ErrorCode {
    UnknownMessageType,
    InvalidChecksum,
    InvalidState,
    UnsupportedCodec,
    ProtocolMismatch,
    PeerTimeout,
    ResourceExhausted,
    Internal,
}

impl ErrorCode {
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::UnknownMessageType => 0x01,
            Self::InvalidChecksum => 0x02,
            Self::InvalidState => 0x03,
            Self::UnsupportedCodec => 0x04,
            Self::ProtocolMismatch => 0x05,
            Self::PeerTimeout => 0x06,
            Self::ResourceExhausted => 0x07,
            Self::Internal => 0xFF,
        }
    }

    pub const fn from_code(code: u8) -> Result<Self, Error> {
        Ok(match code {
            0x01 => Self::UnknownMessageType,
            0x02 => Self::InvalidChecksum,
            0x03 => Self::InvalidState,
            0x04 => Self::UnsupportedCodec,
            0x05 => Self::ProtocolMismatch,
            0x06 => Self::PeerTimeout,
            0x07 => Self::ResourceExhausted,
            0xFF => Self::Internal,
            _ => return Err(Error::MalformedPayload),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_type_roundtrip_canonical() {
        for code in [
            0x01u8, 0x02, 0x03, 0x10, 0x11, 0x20, 0x21, 0x30, 0x31, 0x32, 0xFE, 0xFF,
        ] {
            let m = MessageType::from_code(code).unwrap();
            assert_eq!(m.code(), code);
        }
    }

    #[test]
    fn message_type_extension_range() {
        for code in 0x40u8..=0x7F {
            assert_eq!(
                MessageType::from_code(code).unwrap(),
                MessageType::Extension(code)
            );
        }
    }

    #[test]
    fn message_type_unknown_rejected() {
        assert!(MessageType::from_code(0x04).is_err());
        assert!(MessageType::from_code(0x80).is_err());
        assert!(MessageType::from_code(0xAA).is_err());
    }

    #[test]
    fn handshake_init_roundtrip() {
        let init = HandshakeInit {
            protocol_version: 1,
            codec_preferred: 2,
            sample_rate: 48000,
            channels: 2,
            frame_duration_ms: 20,
            max_bitrate: 64000,
            capability_flags: 0xDEAD_BEEF,
            nonce: 0x1234_5678,
        };
        let mut buf = [0u8; HandshakeInit::SIZE];
        init.encode(&mut buf).unwrap();
        assert_eq!(HandshakeInit::decode(&buf).unwrap(), init);
    }

    #[test]
    fn handshake_accept_roundtrip() {
        let a = HandshakeAccept {
            protocol_version: 1,
            codec_accepted: 2,
            sample_rate: 48000,
            channels: 1,
            frame_duration_ms: 10,
            max_bitrate: 128000,
            capability_flags: 0,
            nonce: 0xCAFE_BABE,
            session_id: 0x4242_4242,
        };
        let mut buf = [0u8; HandshakeAccept::SIZE];
        a.encode(&mut buf).unwrap();
        assert_eq!(HandshakeAccept::decode(&buf).unwrap(), a);
    }

    #[test]
    fn handshake_confirm_roundtrip() {
        let c = HandshakeConfirm {
            session_id: 0x7777_1111,
        };
        let mut buf = [0u8; HandshakeConfirm::SIZE];
        c.encode(&mut buf).unwrap();
        assert_eq!(HandshakeConfirm::decode(&buf).unwrap(), c);
    }

    #[test]
    fn error_code_roundtrip() {
        for code in [0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0xFF] {
            let e = ErrorCode::from_code(code).unwrap();
            assert_eq!(e.code(), code);
        }
    }
}
