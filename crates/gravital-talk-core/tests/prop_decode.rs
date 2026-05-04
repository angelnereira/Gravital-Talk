//! FASE 4 — tests property-based para decode de paquetes y ensamblado de fragmentos.
//!
//! Estos tests sirven como harness de fuzzing basado en proptest: cubren los
//! mismos vectores que un fuzzer libFuzzer cubriría (inputs arbitrarios, bit
//! flips, payloads malformados) sin requerir toolchain nightly.

use gravital_talk_core::constants::{HEADER_SIZE, MAX_FRAGMENTS};
use gravital_talk_core::fragment::{FragmentHeader, FragmentReassembler};
use gravital_talk_core::header::{Flags, PacketHeader};
use gravital_talk_core::packet::{PacketBuilder, PacketView};
use proptest::prelude::*;

// ── 1. Decode nunca hace panic ────────────────────────────────────────────────

proptest! {
    /// Ningún slice de bytes arbitrario debe provocar un panic en PacketView::decode.
    #[test]
    fn packet_decode_never_panics(buf in proptest::collection::vec(any::<u8>(), 0..2048)) {
        // La única invariante es: no panic. Los errores son aceptables.
        let _ = PacketView::decode(&buf);
    }

    /// Inputs de tamaño exactamente mínimo (HEADER_SIZE + 4 bytes para len+crc)
    /// tampoco deben provocar panic.
    #[test]
    fn packet_decode_boundary_sizes(
        buf in proptest::collection::vec(any::<u8>(), 0..=(HEADER_SIZE + 4))
    ) {
        let _ = PacketView::decode(&buf);
    }

    /// Un byte reservado no-cero debe ser rechazado en cualquier posición (5, 6, 7).
    #[test]
    fn reserved_byte_nonzero_rejected(
        // Generar un valor no-cero para el byte reservado
        reserved_val in 1u8..=255u8,
        reserved_pos in 0usize..3,
    ) {
        // Construir un buffer del tamaño de header + 4 bytes extra (len + crc)
        // con los magic bytes y versión correctos.
        let mut buf = vec![0u8; HEADER_SIZE + 4];
        buf[0] = b'G';
        buf[1] = b'S';
        buf[2] = 0x01; // version
        // offsets 5-7 son los reservados
        buf[5 + reserved_pos] = reserved_val;
        let result = PacketView::decode(&buf);
        prop_assert!(result.is_err(), "reserved byte non-zero should be rejected");
    }
}

// ── 2. Roundtrip de paquetes válidos ──────────────────────────────────────────

proptest! {
    /// Un paquete correctamente codificado debe decodificarse con los mismos
    /// valores de campo que tenía el header original.
    #[test]
    fn valid_packet_encode_decode_roundtrip(
        msg_type in 1u8..=0x40u8,
        session_id: u32,
        sequence: u32,
        timestamp: u64,
        payload in proptest::collection::vec(any::<u8>(), 0..512),
    ) {
        let header = PacketHeader {
            version: 1,
            flags: Flags::empty(),
            msg_type,
            session_id,
            sequence,
            timestamp,
        };

        let needed = HEADER_SIZE + payload.len() + 4; // header + payload + len(2) + crc(2)
        let mut buf = vec![0u8; needed + 16];
        let n = PacketBuilder::new(header, &payload)
            .encode(&mut buf)
            .expect("valid header and payload should encode");

        let view = PacketView::decode(&buf[..n])
            .expect("freshly encoded packet should decode without error");

        prop_assert_eq!(view.header().session_id, session_id);
        prop_assert_eq!(view.header().sequence, sequence);
        prop_assert_eq!(view.header().msg_type, msg_type);
        prop_assert_eq!(view.payload(), payload.as_slice());
    }
}

// ── 3. Detección de corrupción por checksum ───────────────────────────────────

proptest! {
    /// Corromper el checksum de un paquete válido debe provocar error de decode.
    #[test]
    fn checksum_corruption_detected(
        msg_type in 1u8..=0x40u8,
        session_id: u32,
        sequence: u32,
        payload in proptest::collection::vec(any::<u8>(), 1..256),
        // Valor de corrupción garantizado no-cero para que cambie el checksum.
        flip in 1u8..=255u8,
    ) {
        let header = PacketHeader {
            version: 1,
            flags: Flags::empty(),
            msg_type,
            session_id,
            sequence,
            timestamp: 0,
        };
        let needed = HEADER_SIZE + payload.len() + 4;
        let mut buf = vec![0u8; needed + 16];
        let n = PacketBuilder::new(header, &payload)
            .encode(&mut buf)
            .unwrap();

        // XOR el byte menos significativo del checksum (último byte del paquete).
        buf[n - 1] ^= flip;

        let result = PacketView::decode(&buf[..n]);
        prop_assert!(result.is_err(), "checksum corruption should be detected");
    }
}

// ── 4. FragmentHeader — decode nunca hace panic ───────────────────────────────

proptest! {
    /// 4 bytes arbitrarios nunca deben provocar panic en FragmentHeader::decode.
    #[test]
    fn fragment_header_decode_never_panics(b0: u8, b1: u8, b2: u8, b3: u8) {
        let buf = [b0, b1, b2, b3];
        let _ = FragmentHeader::decode(&buf);
    }

    /// Um FragmentHeader com campos válidos codifica e decodifica corretamente.
    #[test]
    fn fragment_header_valid_roundtrip(
        total in 1u16..=(MAX_FRAGMENTS),
        index in 0u16..MAX_FRAGMENTS,
    ) {
        prop_assume!(index < total);
        let fh = FragmentHeader { index, total };
        let mut buf = [0u8; 4];
        fh.encode(&mut buf).expect("valid fragment header should encode");
        let decoded = FragmentHeader::decode(&buf).expect("freshly encoded header should decode");
        prop_assert_eq!(decoded.index, index);
        prop_assert_eq!(decoded.total, total);
    }
}

// ── 5. FragmentReassembler — nunca hace panic con entradas arbitrarias ────────

proptest! {
    /// El reassembler no debe hacer panic al recibir secuencias arbitrarias de
    /// fragmentos, incluyendo índices inválidos o duplicados.
    #[test]
    fn fragment_reassembler_arbitrary_input_no_panic(
        total in 1u16..=16u16,
        pushes in proptest::collection::vec(
            (0u16..=20u16, proptest::collection::vec(any::<u8>(), 0..64)),
            0..10,
        )
    ) {
        // new() can fail if total == 0 or > MAX_FRAGMENTS; we guard above.
        if let Ok(mut ra) = FragmentReassembler::new(total) {
            for (index, payload) in pushes {
                // insert() may fail for various reasons; the invariant is no panic.
                let _ = ra.insert(index, &payload);
            }
        }
    }
}
