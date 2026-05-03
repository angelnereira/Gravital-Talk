//! Tipos de mensaje y payloads estructurados.
//!
//! ## Handshake criptográfico (4 pasos)
//!
//! ```text
//! Cliente                            Servidor
//!   │── ClientHello (0x01) ──────────►│  X25519 pubkey efímera + nonce
//!   │◄── ServerHello (0x02) ──────────│  X25519 pubkey efímera + nonce + session_id
//!   │    [ambos derivan session keys via X25519 + HKDF]
//!   │── KeyExchange  (0x04) ──────────►│  auth_tag (prueba de encrypt_key)
//!   │◄── SessionConfirm (0x03) ────────│  auth_tag (prueba de decrypt_key)
//! ```

use crate::error::Error;

/// Códigos de `msg_type` del header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MessageType {
    /// Paso 1: cliente → servidor (X25519 pubkey efímera + nonce + parámetros).
    HandshakeClientHello,
    /// Paso 2: servidor → cliente (X25519 pubkey efímera + nonce + session_id + codec).
    HandshakeServerHello,
    /// Paso 3: cliente → servidor (auth_tag como prueba de posesión de encrypt_key).
    HandshakeSessionConfirm,
    /// Paso 4: servidor → cliente (auth_tag como prueba de posesión de decrypt_key).
    HandshakeKeyExchange,
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
            Self::HandshakeClientHello => 0x01,
            Self::HandshakeServerHello => 0x02,
            Self::HandshakeSessionConfirm => 0x03,
            Self::HandshakeKeyExchange => 0x04,
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
            0x01 => Self::HandshakeClientHello,
            0x02 => Self::HandshakeServerHello,
            0x03 => Self::HandshakeSessionConfirm,
            0x04 => Self::HandshakeKeyExchange,
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
            Self::HandshakeClientHello
                | Self::HandshakeServerHello
                | Self::HandshakeSessionConfirm
                | Self::HandshakeKeyExchange
        )
    }
}

// ─── Payload: ClientHello (0x01) ─────────────────────────────────────────────
//
// 80 bytes:
//   ephemeral_public_key [32]  — X25519 public key efímera del cliente
//   client_nonce         [32]  — nonce criptográfico (OsRng)
//   protocol_version     [1]
//   codec_preferred      [1]
//   sample_rate          [4]   BE
//   channels             [1]
//   frame_duration_ms    [1]
//   max_bitrate          [4]   BE
//   capability_flags     [4]   BE

/// Payload del primer mensaje del handshake (cliente → servidor).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientHello {
    pub ephemeral_public_key: [u8; 32],
    pub client_nonce: [u8; 32],
    pub protocol_version: u8,
    pub codec_preferred: u8,
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_duration_ms: u8,
    pub max_bitrate: u32,
    pub capability_flags: u32,
}

impl ClientHello {
    pub const SIZE: usize = 80;

    pub fn encode(&self, buf: &mut [u8]) -> Result<(), Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::BufferTooSmall);
        }
        buf[0..32].copy_from_slice(&self.ephemeral_public_key);
        buf[32..64].copy_from_slice(&self.client_nonce);
        buf[64] = self.protocol_version;
        buf[65] = self.codec_preferred;
        buf[66..70].copy_from_slice(&self.sample_rate.to_be_bytes());
        buf[70] = self.channels;
        buf[71] = self.frame_duration_ms;
        buf[72..76].copy_from_slice(&self.max_bitrate.to_be_bytes());
        buf[76..80].copy_from_slice(&self.capability_flags.to_be_bytes());
        Ok(())
    }

    pub fn decode(buf: &[u8]) -> Result<Self, Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::MalformedPayload);
        }
        let mut ephemeral_public_key = [0u8; 32];
        ephemeral_public_key.copy_from_slice(&buf[0..32]);
        let mut client_nonce = [0u8; 32];
        client_nonce.copy_from_slice(&buf[32..64]);
        Ok(Self {
            ephemeral_public_key,
            client_nonce,
            protocol_version: buf[64],
            codec_preferred: buf[65],
            sample_rate: u32::from_be_bytes([buf[66], buf[67], buf[68], buf[69]]),
            channels: buf[70],
            frame_duration_ms: buf[71],
            max_bitrate: u32::from_be_bytes([buf[72], buf[73], buf[74], buf[75]]),
            capability_flags: u32::from_be_bytes([buf[76], buf[77], buf[78], buf[79]]),
        })
    }
}

// ─── Payload: ServerHello (0x02) ─────────────────────────────────────────────
//
// 84 bytes:
//   ephemeral_public_key [32]
//   server_nonce         [32]
//   session_id           [4]   BE
//   protocol_version     [1]
//   codec_accepted       [1]
//   sample_rate          [4]   BE
//   channels             [1]
//   frame_duration_ms    [1]
//   max_bitrate          [4]   BE
//   capability_flags     [4]   BE

/// Payload del segundo mensaje del handshake (servidor → cliente).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerHello {
    pub ephemeral_public_key: [u8; 32],
    pub server_nonce: [u8; 32],
    pub session_id: u32,
    pub protocol_version: u8,
    pub codec_accepted: u8,
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_duration_ms: u8,
    pub max_bitrate: u32,
    pub capability_flags: u32,
}

impl ServerHello {
    pub const SIZE: usize = 84;

    pub fn encode(&self, buf: &mut [u8]) -> Result<(), Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::BufferTooSmall);
        }
        buf[0..32].copy_from_slice(&self.ephemeral_public_key);
        buf[32..64].copy_from_slice(&self.server_nonce);
        buf[64..68].copy_from_slice(&self.session_id.to_be_bytes());
        buf[68] = self.protocol_version;
        buf[69] = self.codec_accepted;
        buf[70..74].copy_from_slice(&self.sample_rate.to_be_bytes());
        buf[74] = self.channels;
        buf[75] = self.frame_duration_ms;
        buf[76..80].copy_from_slice(&self.max_bitrate.to_be_bytes());
        buf[80..84].copy_from_slice(&self.capability_flags.to_be_bytes());
        Ok(())
    }

    pub fn decode(buf: &[u8]) -> Result<Self, Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::MalformedPayload);
        }
        let mut ephemeral_public_key = [0u8; 32];
        ephemeral_public_key.copy_from_slice(&buf[0..32]);
        let mut server_nonce = [0u8; 32];
        server_nonce.copy_from_slice(&buf[32..64]);
        Ok(Self {
            ephemeral_public_key,
            server_nonce,
            session_id: u32::from_be_bytes([buf[64], buf[65], buf[66], buf[67]]),
            protocol_version: buf[68],
            codec_accepted: buf[69],
            sample_rate: u32::from_be_bytes([buf[70], buf[71], buf[72], buf[73]]),
            channels: buf[74],
            frame_duration_ms: buf[75],
            max_bitrate: u32::from_be_bytes([buf[76], buf[77], buf[78], buf[79]]),
            capability_flags: u32::from_be_bytes([buf[80], buf[81], buf[82], buf[83]]),
        })
    }
}

// ─── Payload: KeyExchange (0x04) ─────────────────────────────────────────────
//
// 20 bytes:
//   session_id [4]   BE
//   auth_tag   [16]  — HKDF-Expand(encrypt_key, "GS-client-fin-v1" || transcript)[..16]

/// Paso 3: el cliente prueba que derivó la `encrypt_key` correcta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyExchangeMsg {
    pub session_id: u32,
    pub auth_tag: [u8; 16],
}

impl KeyExchangeMsg {
    pub const SIZE: usize = 20;

    pub fn encode(&self, buf: &mut [u8]) -> Result<(), Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::BufferTooSmall);
        }
        buf[0..4].copy_from_slice(&self.session_id.to_be_bytes());
        buf[4..20].copy_from_slice(&self.auth_tag);
        Ok(())
    }

    pub fn decode(buf: &[u8]) -> Result<Self, Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::MalformedPayload);
        }
        let mut auth_tag = [0u8; 16];
        auth_tag.copy_from_slice(&buf[4..20]);
        Ok(Self {
            session_id: u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]),
            auth_tag,
        })
    }
}

// ─── Payload: SessionConfirm (0x03) ──────────────────────────────────────────
//
// 20 bytes:
//   session_id      [4]   BE
//   server_auth_tag [16]  — HKDF-Expand(decrypt_key, "GS-server-fin-v1" || transcript)[..16]

/// Paso 4: el servidor confirma la sesión con su propio auth_tag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionConfirm {
    pub session_id: u32,
    pub server_auth_tag: [u8; 16],
}

impl SessionConfirm {
    pub const SIZE: usize = 20;

    pub fn encode(&self, buf: &mut [u8]) -> Result<(), Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::BufferTooSmall);
        }
        buf[0..4].copy_from_slice(&self.session_id.to_be_bytes());
        buf[4..20].copy_from_slice(&self.server_auth_tag);
        Ok(())
    }

    pub fn decode(buf: &[u8]) -> Result<Self, Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::MalformedPayload);
        }
        let mut server_auth_tag = [0u8; 16];
        server_auth_tag.copy_from_slice(&buf[4..20]);
        Ok(Self {
            session_id: u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]),
            server_auth_tag,
        })
    }
}

// ─── Aliases de compatibilidad (nombres anteriores) ──────────────────────────

/// Alias de compatibilidad → [`ClientHello`].
pub type HandshakeInit = ClientHello;
/// Alias de compatibilidad → [`ServerHello`].
pub type HandshakeAccept = ServerHello;
/// Alias de compatibilidad → [`SessionConfirm`].
pub type HandshakeConfirm = SessionConfirm;

// ─── Códigos de error del protocolo ──────────────────────────────────────────

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
    AuthenticationFailed,
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
            Self::AuthenticationFailed => 0x08,
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
            0x08 => Self::AuthenticationFailed,
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
            0x01u8, 0x02, 0x03, 0x04, 0x10, 0x11, 0x20, 0x21, 0x30, 0x31, 0x32, 0xFE, 0xFF,
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
        assert!(MessageType::from_code(0x05).is_err());
        assert!(MessageType::from_code(0x80).is_err());
        assert!(MessageType::from_code(0xAA).is_err());
    }

    #[test]
    fn client_hello_roundtrip() {
        let hello = ClientHello {
            ephemeral_public_key: [0xABu8; 32],
            client_nonce: [0xCDu8; 32],
            protocol_version: 1,
            codec_preferred: 2,
            sample_rate: 48000,
            channels: 2,
            frame_duration_ms: 20,
            max_bitrate: 64000,
            capability_flags: 0xDEAD_BEEF,
        };
        let mut buf = [0u8; ClientHello::SIZE];
        hello.encode(&mut buf).unwrap();
        assert_eq!(ClientHello::decode(&buf).unwrap(), hello);
    }

    #[test]
    fn server_hello_roundtrip() {
        let hello = ServerHello {
            ephemeral_public_key: [0x11u8; 32],
            server_nonce: [0x22u8; 32],
            session_id: 0x4242_4242,
            protocol_version: 1,
            codec_accepted: 2,
            sample_rate: 48000,
            channels: 1,
            frame_duration_ms: 10,
            max_bitrate: 128000,
            capability_flags: 0,
        };
        let mut buf = [0u8; ServerHello::SIZE];
        hello.encode(&mut buf).unwrap();
        assert_eq!(ServerHello::decode(&buf).unwrap(), hello);
    }

    #[test]
    fn key_exchange_roundtrip() {
        let msg = KeyExchangeMsg {
            session_id: 0x1234_5678,
            auth_tag: [0xFEu8; 16],
        };
        let mut buf = [0u8; KeyExchangeMsg::SIZE];
        msg.encode(&mut buf).unwrap();
        assert_eq!(KeyExchangeMsg::decode(&buf).unwrap(), msg);
    }

    #[test]
    fn session_confirm_roundtrip() {
        let confirm = SessionConfirm {
            session_id: 0x7777_1111,
            server_auth_tag: [0x99u8; 16],
        };
        let mut buf = [0u8; SessionConfirm::SIZE];
        confirm.encode(&mut buf).unwrap();
        assert_eq!(SessionConfirm::decode(&buf).unwrap(), confirm);
    }

    #[test]
    fn error_code_roundtrip() {
        for code in [0x01u8, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0xFF] {
            let e = ErrorCode::from_code(code).unwrap();
            assert_eq!(e.code(), code);
        }
    }

    #[test]
    fn is_handshake_covers_all_four() {
        assert!(MessageType::HandshakeClientHello.is_handshake());
        assert!(MessageType::HandshakeServerHello.is_handshake());
        assert!(MessageType::HandshakeSessionConfirm.is_handshake());
        assert!(MessageType::HandshakeKeyExchange.is_handshake());
        assert!(!MessageType::AudioFrame.is_handshake());
    }
}
