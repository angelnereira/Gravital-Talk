//! Fuzzer de `MessageType::from_code`.
//!
//! Verifica que todos los valores u8 (0x00..=0xFF) se parsean sin pánico
//! y que los códigos conocidos hacen round-trip exacto.
//!
//! Ejecutar con:
//!   cargo fuzz run fuzz_message_type

#![no_main]

use gravital_talk_core::message::MessageType;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    for &byte in data {
        match MessageType::from_code(byte) {
            Ok(mt) => {
                // Los tipos conocidos deben hacer round-trip exacto.
                assert_eq!(mt.code(), byte);

                // Los helpers categóricos no deben panicar.
                let _ = mt.is_floor_control();
                let _ = mt.is_auth();
            }
            Err(_) => {
                // Código desconocido — no debe panicar.
            }
        }
    }
});
