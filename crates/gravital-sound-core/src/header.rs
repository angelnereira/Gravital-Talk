//! Header de 24 bytes del paquete Gravital Sound.
//!
//! Layout (big-endian):
//!
//! ```text
//! 0  magic (2)         — 0x4753 ("GS")
//! 2  version (1)
//! 3  flags (1)
//! 4  msg_type (1)
//! 5  _reserved0 (3)    — debe ser 0
//! 8  session_id (4)
//! 12 sequence (4)
//! 16 timestamp (8)
//! 24 ─ fin del header base
//! ```
//!
//! `payload_len` (u16) y `checksum` (u16) viven físicamente al final del
//! paquete, no en el header base, para preservar la alineación natural de
//! `timestamp`. Ver [`crate::packet`].

use crate::constants::{offsets, HEADER_SIZE, MAGIC_BYTES, PROTOCOL_VERSION};
use crate::error::Error;

/// Flags del header (bitfield en un único byte).
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flags(pub u8);

impl Flags {
    pub const FRAGMENTED: Self = Self(0b1000_0000);
    pub const LAST_FRAGMENT: Self = Self(0b0100_0000);
    pub const ENCRYPTED: Self = Self(0b0010_0000);
    pub const RETRANSMIT: Self = Self(0b0001_0000);

    #[inline]
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[inline]
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline]
    pub fn set(&mut self, other: Self) {
        self.0 |= other.0;
    }

    #[inline]
    pub fn unset(&mut self, other: Self) {
        self.0 &= !other.0;
    }

    #[inline]
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }
}

impl core::ops::BitOr for Flags {
    type Output = Self;
    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

/// Header de un paquete, deserializado. Struct aligned-safe: los campos se
/// copian al leer y no hay punning sobre el buffer del wire.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PacketHeader {
    pub version: u8,
    pub flags: Flags,
    pub msg_type: u8,
    pub session_id: u32,
    pub sequence: u32,
    pub timestamp: u64,
}

impl PacketHeader {
    /// Decodifica los primeros 24 bytes de `buf` en un `PacketHeader`.
    ///
    /// El `payload_len` y el `checksum` no están aquí — viven al final del
    /// paquete (ver [`crate::packet::Packet::decode`]).
    #[inline]
    pub fn decode(buf: &[u8]) -> Result<Self, Error> {
        if buf.len() < HEADER_SIZE {
            return Err(Error::TooShort);
        }
        // Magic + versión: los errores aquí se marcan como "cold" implícitamente
        // (el caller hace early return en la primera validación).
        if buf[offsets::MAGIC] != MAGIC_BYTES[0] || buf[offsets::MAGIC + 1] != MAGIC_BYTES[1] {
            return Err(Error::BadMagic);
        }
        let version = buf[offsets::VERSION];
        if version != PROTOCOL_VERSION {
            return Err(Error::ProtocolMismatch);
        }

        // Lecturas alineadas (las offsets fueron elegidas para esto).
        let flags = Flags(buf[offsets::FLAGS]);
        let msg_type = buf[offsets::MSG_TYPE];

        let session_id = u32::from_be_bytes([
            buf[offsets::SESSION_ID],
            buf[offsets::SESSION_ID + 1],
            buf[offsets::SESSION_ID + 2],
            buf[offsets::SESSION_ID + 3],
        ]);
        let sequence = u32::from_be_bytes([
            buf[offsets::SEQUENCE],
            buf[offsets::SEQUENCE + 1],
            buf[offsets::SEQUENCE + 2],
            buf[offsets::SEQUENCE + 3],
        ]);
        let timestamp = u64::from_be_bytes([
            buf[offsets::TIMESTAMP],
            buf[offsets::TIMESTAMP + 1],
            buf[offsets::TIMESTAMP + 2],
            buf[offsets::TIMESTAMP + 3],
            buf[offsets::TIMESTAMP + 4],
            buf[offsets::TIMESTAMP + 5],
            buf[offsets::TIMESTAMP + 6],
            buf[offsets::TIMESTAMP + 7],
        ]);

        Ok(Self {
            version,
            flags,
            msg_type,
            session_id,
            sequence,
            timestamp,
        })
    }

    /// Codifica este header en los primeros 24 bytes de `buf`.
    /// Los 3 bytes reservados se escriben como 0.
    #[inline]
    pub fn encode(&self, buf: &mut [u8]) -> Result<(), Error> {
        if buf.len() < HEADER_SIZE {
            return Err(Error::BufferTooSmall);
        }
        buf[offsets::MAGIC] = MAGIC_BYTES[0];
        buf[offsets::MAGIC + 1] = MAGIC_BYTES[1];
        buf[offsets::VERSION] = self.version;
        buf[offsets::FLAGS] = self.flags.0;
        buf[offsets::MSG_TYPE] = self.msg_type;
        buf[offsets::RESERVED] = 0;
        buf[offsets::RESERVED + 1] = 0;
        buf[offsets::RESERVED + 2] = 0;
        buf[offsets::SESSION_ID..offsets::SESSION_ID + 4]
            .copy_from_slice(&self.session_id.to_be_bytes());
        buf[offsets::SEQUENCE..offsets::SEQUENCE + 4].copy_from_slice(&self.sequence.to_be_bytes());
        buf[offsets::TIMESTAMP..offsets::TIMESTAMP + 8]
            .copy_from_slice(&self.timestamp.to_be_bytes());
        Ok(())
    }

    /// Crea un header nuevo con la versión y flags vacíos.
    #[inline]
    #[must_use]
    pub const fn new(msg_type: u8, session_id: u32, sequence: u32, timestamp: u64) -> Self {
        Self {
            version: PROTOCOL_VERSION,
            flags: Flags::empty(),
            msg_type,
            session_id,
            sequence,
            timestamp,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let h = PacketHeader {
            version: PROTOCOL_VERSION,
            flags: Flags::FRAGMENTED | Flags::LAST_FRAGMENT,
            msg_type: 0x10,
            session_id: 0x1234_5678,
            sequence: 0xDEAD_BEEF,
            timestamp: 0x0123_4567_89AB_CDEF,
        };
        let mut buf = [0u8; HEADER_SIZE];
        h.encode(&mut buf).unwrap();
        let decoded = PacketHeader::decode(&buf).unwrap();
        assert_eq!(h, decoded);
    }

    #[test]
    fn bad_magic_rejected() {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0] = b'X';
        buf[1] = b'Y';
        buf[2] = PROTOCOL_VERSION;
        assert_eq!(PacketHeader::decode(&buf), Err(Error::BadMagic));
    }

    #[test]
    fn bad_version_rejected() {
        let mut buf = [0u8; HEADER_SIZE];
        buf[0] = MAGIC_BYTES[0];
        buf[1] = MAGIC_BYTES[1];
        buf[2] = 0xFF;
        assert_eq!(PacketHeader::decode(&buf), Err(Error::ProtocolMismatch));
    }

    #[test]
    fn short_buffer_rejected() {
        let buf = [0u8; 5];
        assert_eq!(PacketHeader::decode(&buf), Err(Error::TooShort));
    }

    #[test]
    fn flags_bitops() {
        let f = Flags::FRAGMENTED | Flags::LAST_FRAGMENT;
        assert!(f.contains(Flags::FRAGMENTED));
        assert!(f.contains(Flags::LAST_FRAGMENT));
        assert!(!f.contains(Flags::ENCRYPTED));
    }
}
