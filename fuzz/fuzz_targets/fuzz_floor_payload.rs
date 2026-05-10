//! Fuzzer de `FloorPayload::decode` y el FSM `FloorController`.
//!
//! Objetivo: verificar que el parsing de payloads floor control y todas
//! las transiciones del FSM son seguras con entradas arbitrarias.
//!
//! Ejecutar con:
//!   cargo fuzz run fuzz_floor_payload

#![no_main]

use gravital_talk_core::floor::{FloorController, FloorEvent, FloorPayload};
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // ── Test 1: FloorPayload decode + re-encode round-trip ──────────────────
    if let Ok(fp) = FloorPayload::decode(data) {
        let mut buf = [0u8; FloorPayload::SIZE];
        // encode nunca debe panicar.
        let _ = fp.encode(&mut buf);
        // Re-decode debe producir el mismo SSRC.
        if let Ok(fp2) = FloorPayload::decode(&buf) {
            assert_eq!(fp.ssrc, fp2.ssrc);
        }
    }

    // ── Test 2: FSM con secuencia de eventos derivada de los bytes ──────────
    if data.is_empty() {
        return;
    }
    let mut ctrl = FloorController::default();
    // Usar pares (event_byte, ssrc_byte) para más variedad.
    for chunk in data.chunks(2).take(32) {
        let event_byte = chunk[0];
        let ssrc = if chunk.len() >= 2 { chunk[1] as u32 } else { 0 };
        let event = match event_byte % 7 {
            0 => FloorEvent::Request,
            1 => FloorEvent::Grant,
            2 => FloorEvent::Deny,
            3 => FloorEvent::Release,
            4 => FloorEvent::Taken,
            5 => FloorEvent::Timeout,
            _ => FloorEvent::Reset,
        };
        // El FSM nunca debe panicar; las transiciones inválidas retornan Err.
        let _ = ctrl.transition(event, ssrc);
        // Acceder al estado tampoco debe panicar.
        let _ = ctrl.state();
        let _ = ctrl.is_available();
        let _ = ctrl.is_transmitting();
    }
});
