//! Ensamblado y desensamblado de paquetes completos.
//!
//! El layout físico del paquete es:
//!
//! ```text
//! [  header base (24 bytes)  ][  payload (N bytes)  ][ payload_len (2) ][ checksum (2) ]
//!                                                      ^tail-4             ^tail-2
//! ```
//!
//! `payload_len` y `checksum` viven al final del paquete para preservar
//! alineación natural de `timestamp` en el header. Ver ADR-003.

use crate::checksum::crc16_two_segments;
use crate::constants::{
    CHECKSUM_TAIL_OFFSET, HEADER_SIZE, MAX_PAYLOAD_SIZE, PAYLOAD_LEN_TAIL_OFFSET,
};
use crate::error::Error;
use crate::header::PacketHeader;

/// Un paquete decodificado **sin copia**: la vista referencia el buffer
/// original.
#[derive(Debug)]
pub struct PacketView<'a> {
    header: PacketHeader,
    payload: &'a [u8],
}

impl<'a> PacketView<'a> {
    #[inline]
    #[must_use]
    pub const fn header(&self) -> &PacketHeader {
        &self.header
    }

    #[inline]
    #[must_use]
    pub const fn payload(&self) -> &'a [u8] {
        self.payload
    }

    #[inline]
    #[must_use]
    pub const fn msg_type(&self) -> u8 {
        self.header.msg_type
    }
}

/// Alias corto.
pub type Packet<'a> = PacketView<'a>;

impl<'a> PacketView<'a> {
    /// Decodifica un paquete desde un buffer del wire. Hace validación
    /// ordenada de barata a cara: longitud → magic → versión → length-match
    /// → checksum. Zero-copy.
    pub fn decode(buf: &'a [u8]) -> Result<Self, Error> {
        // Cheapest checks first.
        let total_len = buf.len();
        if total_len < HEADER_SIZE + 4 {
            return Err(Error::TooShort);
        }

        let header = PacketHeader::decode(buf)?;

        // payload_len y checksum al final.
        let payload_len_offset = total_len - PAYLOAD_LEN_TAIL_OFFSET;
        let checksum_offset = total_len - CHECKSUM_TAIL_OFFSET;
        let payload_len =
            u16::from_be_bytes([buf[payload_len_offset], buf[payload_len_offset + 1]]);
        let expected_len = HEADER_SIZE + payload_len as usize + 4;

        if expected_len != total_len {
            return Err(Error::LengthMismatch);
        }
        if payload_len as usize > MAX_PAYLOAD_SIZE {
            return Err(Error::PayloadTooLarge);
        }

        let payload = &buf[HEADER_SIZE..HEADER_SIZE + payload_len as usize];

        // Checksum: calcular sobre header (con checksum=0) + payload + payload_len,
        // comparar con el campo almacenado.
        let stored_checksum = u16::from_be_bytes([buf[checksum_offset], buf[checksum_offset + 1]]);
        let computed = compute_checksum(&buf[..HEADER_SIZE], payload, payload_len);

        if stored_checksum != computed {
            return Err(Error::BadChecksum);
        }

        Ok(Self { header, payload })
    }
}

/// Calcula el checksum canónico: header (24) + payload + payload_len (2),
/// en ese orden. `checksum` no entra al cálculo.
#[inline]
fn compute_checksum(header: &[u8], payload: &[u8], payload_len: u16) -> u16 {
    // Para evitar alojar, hacemos CRC en 3 pasadas (header, payload, len).
    let mut crc: u16 = 0xFFFF;
    crc = crc_step(crc, header);
    crc = crc_step(crc, payload);
    crc = crc_step(crc, &payload_len.to_be_bytes());
    crc
}

#[inline]
fn crc_step(mut crc: u16, data: &[u8]) -> u16 {
    use crate::checksum;
    // Reutiliza la tabla del módulo checksum llamando dos-segmentos con vacío.
    // Para evitar dependencia de internals, replicamos la lógica con la misma
    // tabla pública no expuesta — hacemos una pasada manual aquí.
    let _ = checksum::crc16_ccitt_false;
    for &byte in data {
        let idx = ((crc >> 8) as u8 ^ byte) as usize;
        crc = (crc << 8) ^ TABLE[idx];
    }
    crc
}

const POLY: u16 = 0x1021;
const TABLE: [u16; 256] = build_table();

const fn build_table() -> [u16; 256] {
    let mut table = [0u16; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = (i as u16) << 8;
        let mut bit = 0;
        while bit < 8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ POLY
            } else {
                crc << 1
            };
            bit += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

/// Builder que codifica un paquete en un buffer del caller.
#[derive(Debug, Clone, Copy)]
pub struct PacketBuilder<'a> {
    pub header: PacketHeader,
    pub payload: &'a [u8],
}

impl<'a> PacketBuilder<'a> {
    #[inline]
    #[must_use]
    pub const fn new(header: PacketHeader, payload: &'a [u8]) -> Self {
        Self { header, payload }
    }

    /// Codifica el paquete en `out` y devuelve el número de bytes escritos.
    /// Cero allocs.
    pub fn encode(&self, out: &mut [u8]) -> Result<usize, Error> {
        let payload_len = self.payload.len();
        if payload_len > MAX_PAYLOAD_SIZE {
            return Err(Error::PayloadTooLarge);
        }
        let total = HEADER_SIZE + payload_len + 4;
        if out.len() < total {
            return Err(Error::BufferTooSmall);
        }

        // Header base.
        self.header.encode(&mut out[..HEADER_SIZE])?;

        // Payload.
        out[HEADER_SIZE..HEADER_SIZE + payload_len].copy_from_slice(self.payload);

        // payload_len.
        let payload_len_u16 = payload_len as u16;
        let payload_len_offset = total - PAYLOAD_LEN_TAIL_OFFSET;
        out[payload_len_offset..payload_len_offset + 2]
            .copy_from_slice(&payload_len_u16.to_be_bytes());

        // Checksum: header + payload + payload_len.
        let checksum = crc16_two_segments(&out[..HEADER_SIZE], self.payload);
        let checksum = {
            let mut crc = checksum;
            for &b in &payload_len_u16.to_be_bytes() {
                let idx = ((crc >> 8) as u8 ^ b) as usize;
                crc = (crc << 8) ^ TABLE[idx];
            }
            crc
        };
        let checksum_offset = total - CHECKSUM_TAIL_OFFSET;
        out[checksum_offset..checksum_offset + 2].copy_from_slice(&checksum.to_be_bytes());

        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::constants::PROTOCOL_VERSION;
    use crate::header::Flags;

    fn sample_header() -> PacketHeader {
        PacketHeader {
            version: PROTOCOL_VERSION,
            flags: Flags::empty(),
            msg_type: 0x10,
            session_id: 0xAABB_CCDD,
            sequence: 42,
            timestamp: 1_000_000,
        }
    }

    #[test]
    fn roundtrip_empty_payload() {
        let header = sample_header();
        let payload = &[];
        let mut buf = [0u8; HEADER_SIZE + 4];
        let n = PacketBuilder::new(header, payload)
            .encode(&mut buf)
            .unwrap();
        assert_eq!(n, HEADER_SIZE + 4);

        let packet = PacketView::decode(&buf[..n]).unwrap();
        assert_eq!(packet.header(), &header);
        assert!(packet.payload().is_empty());
    }

    #[test]
    fn roundtrip_with_payload() {
        let header = sample_header();
        let payload: [u8; 128] = core::array::from_fn(|i| (i * 7) as u8);
        let mut buf = [0u8; 1200];
        let n = PacketBuilder::new(header, &payload)
            .encode(&mut buf)
            .unwrap();
        let packet = PacketView::decode(&buf[..n]).unwrap();
        assert_eq!(packet.header(), &header);
        assert_eq!(packet.payload(), &payload);
    }

    #[test]
    fn checksum_rejects_corruption() {
        let header = sample_header();
        let payload: [u8; 64] = [0xAB; 64];
        let mut buf = [0u8; 128];
        let n = PacketBuilder::new(header, &payload)
            .encode(&mut buf)
            .unwrap();
        buf[HEADER_SIZE + 5] ^= 0xFF;
        assert!(matches!(
            PacketView::decode(&buf[..n]),
            Err(Error::BadChecksum)
        ));
    }

    #[test]
    fn too_short_buffer() {
        let buf = [0u8; 10];
        assert!(matches!(PacketView::decode(&buf), Err(Error::TooShort)));
    }

    #[test]
    fn payload_too_large() {
        let header = sample_header();
        let huge = [0u8; MAX_PAYLOAD_SIZE + 1];
        let mut buf = [0u8; MAX_PAYLOAD_SIZE + HEADER_SIZE + 10];
        assert_eq!(
            PacketBuilder::new(header, &huge).encode(&mut buf),
            Err(Error::PayloadTooLarge),
        );
    }

    #[test]
    fn zero_copy_payload_points_into_buffer() {
        let header = sample_header();
        let payload: [u8; 32] = [0x5A; 32];
        let mut buf = [0u8; 128];
        let n = PacketBuilder::new(header, &payload)
            .encode(&mut buf)
            .unwrap();
        let packet = PacketView::decode(&buf[..n]).unwrap();
        // El payload devuelto debe ser una subslice del buffer original.
        let buf_ptr = buf.as_ptr();
        let pay_ptr = packet.payload().as_ptr();
        assert!(pay_ptr >= buf_ptr);
        assert!(unsafe { pay_ptr.offset_from(buf_ptr) } >= 0);
    }
}
