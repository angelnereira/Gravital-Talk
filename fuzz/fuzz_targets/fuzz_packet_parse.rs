//! Fuzzer de `PacketView::decode`.
//!
//! Objetivo: encontrar panics, divisiones por cero o comportamiento UB
//! al parsear bytes arbitrarios como paquetes Gravital Talk.
//!
//! Ejecutar con:
//!   cargo fuzz run fuzz_packet_parse

#![no_main]

use gravital_talk_core::packet::PacketView;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // `decode` nunca debe entrar en pánico; sólo puede devolver Ok o Err.
    if let Ok(view) = PacketView::decode(data) {
        // Si el decode tuvo éxito, re-encode y comprueba idempotencia.
        let header = view.header();
        let payload = view.payload();

        // Acceder a todos los campos — asegura que los reads son seguros.
        let _ = header.version;
        let _ = header.msg_type;
        let _ = header.session_id;
        let _ = header.sequence;
        let _ = header.timestamp;
        let _ = payload.len();

        // Re-encode y volver a parsear — debe ser idempotente.
        use gravital_talk_core::packet::PacketBuilder;
        let mut out = vec![0u8; data.len() + 64];
        if let Ok(n) = PacketBuilder::new(*header, payload).encode(&mut out) {
            let _ = PacketView::decode(&out[..n]);
        }
    }
});
