//! Fragmentación y reensamblado de frames mayores al payload máximo.
//!
//! Cada fragmento viene como `MessageType::AudioFragment` con un sub-header
//! de 4 bytes al inicio del payload:
//!
//! ```text
//! [ fragment_index (u16 BE) ][ total_fragments (u16 BE) ][ fragment payload... ]
//! ```

use crate::constants::{FRAGMENT_SUBHEADER_SIZE, MAX_FRAGMENTS};
use crate::error::Error;

#[cfg(feature = "alloc")]
use alloc::vec::Vec;

/// Sub-header de fragmentación.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FragmentHeader {
    pub index: u16,
    pub total: u16,
}

impl FragmentHeader {
    pub const SIZE: usize = FRAGMENT_SUBHEADER_SIZE;

    pub fn encode(&self, buf: &mut [u8]) -> Result<(), Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::BufferTooSmall);
        }
        buf[0..2].copy_from_slice(&self.index.to_be_bytes());
        buf[2..4].copy_from_slice(&self.total.to_be_bytes());
        Ok(())
    }

    pub fn decode(buf: &[u8]) -> Result<Self, Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::MalformedPayload);
        }
        let index = u16::from_be_bytes([buf[0], buf[1]]);
        let total = u16::from_be_bytes([buf[2], buf[3]]);
        if total == 0 || total > MAX_FRAGMENTS {
            return Err(Error::TooManyFragments);
        }
        if index >= total {
            return Err(Error::FragmentOutOfRange);
        }
        Ok(Self { index, total })
    }
}

/// Reensamblador de un frame fragmentado identificado por `sequence`.
/// Mantiene los fragmentos en memoria hasta completarse o ser descartado.
#[cfg(feature = "alloc")]
#[derive(Debug)]
pub struct FragmentReassembler {
    expected_total: u16,
    buffers: Vec<Option<Vec<u8>>>,
    received: u16,
}

#[cfg(feature = "alloc")]
impl FragmentReassembler {
    /// Construye un reensamblador esperando `total` fragmentos.
    pub fn new(total: u16) -> Result<Self, Error> {
        if total == 0 || total > MAX_FRAGMENTS {
            return Err(Error::TooManyFragments);
        }
        let mut buffers = Vec::with_capacity(total as usize);
        buffers.resize_with(total as usize, || None);
        Ok(Self {
            expected_total: total,
            buffers,
            received: 0,
        })
    }

    /// Inserta un fragmento. El payload debe ser sólo el contenido del
    /// fragmento (sin el sub-header).
    pub fn insert(&mut self, index: u16, payload: &[u8]) -> Result<(), Error> {
        if index >= self.expected_total {
            return Err(Error::FragmentOutOfRange);
        }
        let slot = &mut self.buffers[index as usize];
        if slot.is_some() {
            return Err(Error::DuplicateFragment);
        }
        *slot = Some(payload.to_vec());
        self.received += 1;
        Ok(())
    }

    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.received == self.expected_total
    }

    /// Consume el reensamblador devolviendo el frame completo concatenado.
    pub fn finish(self) -> Result<Vec<u8>, Error> {
        if !self.is_complete() {
            return Err(Error::IncompleteReassembly);
        }
        let total_len: usize = self
            .buffers
            .iter()
            .map(|b| b.as_ref().map_or(0, Vec::len))
            .sum();
        let mut out = Vec::with_capacity(total_len);
        for buf in self.buffers.into_iter().flatten() {
            out.extend_from_slice(&buf);
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_roundtrip() {
        let h = FragmentHeader { index: 3, total: 8 };
        let mut buf = [0u8; FragmentHeader::SIZE];
        h.encode(&mut buf).unwrap();
        assert_eq!(FragmentHeader::decode(&buf).unwrap(), h);
    }

    #[test]
    fn header_rejects_out_of_range() {
        let mut buf = [0u8; 4];
        // index = 5, total = 3 → index ≥ total.
        buf[0..2].copy_from_slice(&5u16.to_be_bytes());
        buf[2..4].copy_from_slice(&3u16.to_be_bytes());
        assert_eq!(FragmentHeader::decode(&buf), Err(Error::FragmentOutOfRange));
    }

    #[test]
    fn header_rejects_zero_total() {
        let mut buf = [0u8; 4];
        buf[0..2].copy_from_slice(&0u16.to_be_bytes());
        buf[2..4].copy_from_slice(&0u16.to_be_bytes());
        assert_eq!(FragmentHeader::decode(&buf), Err(Error::TooManyFragments));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn reassembly_in_order() {
        let mut r = FragmentReassembler::new(3).unwrap();
        assert!(!r.is_complete());
        r.insert(0, b"hola ").unwrap();
        r.insert(1, b"mundo").unwrap();
        r.insert(2, b" gs!").unwrap();
        assert!(r.is_complete());
        let out = r.finish().unwrap();
        assert_eq!(&out, b"hola mundo gs!");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn reassembly_out_of_order() {
        let mut r = FragmentReassembler::new(3).unwrap();
        r.insert(2, b" gs!").unwrap();
        r.insert(0, b"hola ").unwrap();
        r.insert(1, b"mundo").unwrap();
        let out = r.finish().unwrap();
        assert_eq!(&out, b"hola mundo gs!");
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn reassembly_rejects_duplicate() {
        let mut r = FragmentReassembler::new(2).unwrap();
        r.insert(0, b"a").unwrap();
        assert_eq!(r.insert(0, b"a"), Err(Error::DuplicateFragment));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn reassembly_rejects_incomplete() {
        let mut r = FragmentReassembler::new(3).unwrap();
        r.insert(0, b"a").unwrap();
        r.insert(1, b"b").unwrap();
        assert_eq!(r.finish(), Err(Error::IncompleteReassembly));
    }

    #[cfg(feature = "alloc")]
    #[test]
    fn reassembly_rejects_too_many() {
        assert_eq!(
            FragmentReassembler::new(0).err(),
            Some(Error::TooManyFragments)
        );
        assert_eq!(
            FragmentReassembler::new(MAX_FRAGMENTS + 1).err(),
            Some(Error::TooManyFragments)
        );
    }
}
