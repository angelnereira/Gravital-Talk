//! FASE 4 — tests property-based para el encoder/decoder FEC XOR.
//!
//! Valida las propiedades algebraicas del FEC:
//!   1. Sin pérdidas: el decoder no devuelve frame espurio.
//!   2. Una pérdida: el decoder recupera el frame exacto.
//!   3. Dos pérdidas: el decoder no puede recuperar, devuelve None.
//!   4. XOR es involución: a ⊕ b ⊕ b == a.

use bytes::Bytes;
use gravital_talk_transport::fec::{FecDecoder, FecEncoder};
use proptest::prelude::*;

// ── Estrategia: generar ventanas de frames arbitrarios ────────────────────────

/// Genera una ventana de `window` frames con payloads de longitud `frame_len`.
fn arb_frame_window(
    max_window: u8,
    max_frame: usize,
) -> impl Strategy<Value = (u8, Vec<Vec<u8>>)> {
    (2u8..=max_window, 1usize..=max_frame).prop_flat_map(|(window, frame_len)| {
        let frames = proptest::collection::vec(
            proptest::collection::vec(any::<u8>(), frame_len),
            window as usize,
        );
        (Just(window), frames)
    })
}

// ── 1. Sin pérdidas no produce recuperación espuria ───────────────────────────

proptest! {
    #[test]
    fn fec_no_recovery_without_loss(
        (window, frames) in arb_frame_window(8, 256),
    ) {
        let mut enc = FecEncoder::new(window);
        let mut parity = None;
        for (i, payload) in frames.iter().enumerate() {
            parity = enc.push(i as u32, payload);
        }
        let parity = parity.expect("encoder should produce parity after full window");

        let mut dec = FecDecoder::new(window);
        for (i, payload) in frames.iter().enumerate() {
            dec.push_data(i as u32, Bytes::from(payload.clone()));
        }

        let result = dec.push_parity(parity);
        prop_assert!(result.is_none(), "no frame lost → no spurious recovery");
    }
}

// ── 2. Una pérdida → recuperación exacta ─────────────────────────────────────

proptest! {
    #[test]
    fn fec_recovers_any_single_lost_frame(
        (window, frames) in arb_frame_window(8, 256),
        lost_seed: usize,
    ) {
        let window_u = window as usize;
        let lost_idx = lost_seed % window_u;

        let mut enc = FecEncoder::new(window);
        let mut parity = None;
        for (i, payload) in frames.iter().enumerate() {
            parity = enc.push(i as u32, payload);
        }
        let parity = parity.expect("encoder must emit parity");

        let mut dec = FecDecoder::new(window);
        for (i, payload) in frames.iter().enumerate() {
            if i != lost_idx {
                dec.push_data(i as u32, Bytes::from(payload.clone()));
            }
        }

        let result = dec.push_parity(parity);
        prop_assert!(result.is_some(), "should recover exactly one lost frame");
        let (rec_seq, rec_payload) = result.unwrap();
        prop_assert_eq!(rec_seq, lost_idx as u32);
        prop_assert_eq!(rec_payload.as_ref(), frames[lost_idx].as_slice(),
            "recovered payload must match original");
    }
}

// ── 3. Dos pérdidas → no se puede recuperar ──────────────────────────────────

proptest! {
    #[test]
    fn fec_cannot_recover_two_losses(
        (window, frames) in arb_frame_window(8, 256),
        seed_a: usize,
        seed_b: usize,
    ) {
        let window_u = window as usize;
        prop_assume!(window_u >= 2);

        let lost_a = seed_a % window_u;
        let lost_b_raw = seed_b % (window_u - 1);
        let lost_b = if lost_b_raw >= lost_a { lost_b_raw + 1 } else { lost_b_raw };
        prop_assume!(lost_a != lost_b);

        let mut enc = FecEncoder::new(window);
        let mut parity = None;
        for (i, payload) in frames.iter().enumerate() {
            parity = enc.push(i as u32, payload);
        }
        let parity = parity.expect("encoder must emit parity");

        let mut dec = FecDecoder::new(window);
        for (i, payload) in frames.iter().enumerate() {
            if i != lost_a && i != lost_b {
                dec.push_data(i as u32, Bytes::from(payload.clone()));
            }
        }

        let result = dec.push_parity(parity);
        prop_assert!(result.is_none(), "two losses cannot be recovered");
    }
}

// ── 4. XOR es involución ──────────────────────────────────────────────────────

proptest! {
    /// a ⊕ b ⊕ b == a para cualquier par de bytes.
    #[test]
    fn xor_involution(
        a in proptest::collection::vec(any::<u8>(), 1..256),
        b in proptest::collection::vec(any::<u8>(), 1..256),
    ) {
        // Usamos el encoder con ventana=2 como proxy de la operación XOR interna.
        let len = a.len().min(b.len());
        let a = &a[..len];
        let b = &b[..len];

        let mut enc = FecEncoder::new(2);
        enc.push(0, a);
        let parity = enc.push(1, b).expect("window=2, second push emits parity");

        // parity = a XOR b; parity XOR b should give a (mod len)
        let parity_bytes = parity.payload;
        let parity_len = parity_bytes.len().min(len);
        let parity_slice = &parity_bytes[..parity_len];
        let b_slice = &b[..parity_len];

        let mut recovered = parity_slice.to_vec();
        for i in 0..recovered.len() {
            recovered[i] ^= b_slice[i];
        }

        prop_assert_eq!(&recovered[..], &a[..parity_len],
            "XOR is self-inverse: (a XOR b) XOR b == a");
    }
}
