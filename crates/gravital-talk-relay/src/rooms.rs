//! Generación y validación de códigos de sala humano-legibles.
//!
//! Formato: `XXXX-NNNN` donde X = letra mayúscula sin ambigüedad (sin I, O)
//! y N = dígito (sin 0, 1). Ejemplo: "GRVT-2847".
//!
//! El código es fácil de leer en voz alta y transmitir por radio.

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// Letras no ambiguas (sin I, O, Q).
const LETTERS: &[u8] = b"ABCDEFGHJKLMNPRSTUVWXYZ";
/// Dígitos no ambiguos (sin 0, 1).
const DIGITS: &[u8] = b"23456789";

/// Genera un código único de 9 caracteres en formato `XXXX-NNNN`.
pub fn generate_code() -> String {
    let seed = {
        let t = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let c = COUNTER.fetch_add(1, Ordering::Relaxed);
        t.wrapping_add(c.wrapping_mul(6364136223846793005))
    };

    let mut rng = seed;
    let mut next = || -> u8 {
        rng = rng.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (rng >> 33) as u8
    };

    let l = LETTERS.len() as u8;
    let d = DIGITS.len() as u8;

    let code = format!(
        "{}{}{}{}-{}{}{}{}",
        LETTERS[(next() % l) as usize] as char,
        LETTERS[(next() % l) as usize] as char,
        LETTERS[(next() % l) as usize] as char,
        LETTERS[(next() % l) as usize] as char,
        DIGITS[(next() % d) as usize] as char,
        DIGITS[(next() % d) as usize] as char,
        DIGITS[(next() % d) as usize] as char,
        DIGITS[(next() % d) as usize] as char,
    );
    code
}

/// Valida que un string sea un código de sala válido.
pub fn is_valid_code(code: &str) -> bool {
    let bytes = code.as_bytes();
    if bytes.len() != 9 || bytes[4] != b'-' {
        return false;
    }
    bytes[..4].iter().all(|b| LETTERS.contains(b))
        && bytes[5..].iter().all(|b| DIGITS.contains(b))
}

/// Genera un session_id aleatorio no-cero para una sala nueva.
pub fn generate_session_id() -> u32 {
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(1);
    let c = COUNTER.fetch_add(1, Ordering::Relaxed);
    let h = t.wrapping_add(c.wrapping_mul(2654435761));
    let id = (h ^ (h >> 32)) as u32;
    if id == 0 { 0xDEAD_BEEF } else { id }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn code_format_valid() {
        for _ in 0..50 {
            let code = generate_code();
            assert!(is_valid_code(&code), "invalid code: {code}");
            assert_eq!(code.len(), 9);
            assert_eq!(code.chars().nth(4), Some('-'));
        }
    }

    #[test]
    fn codes_are_unique() {
        let codes: Vec<_> = (0..20).map(|_| generate_code()).collect();
        let unique: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(codes.len(), unique.len());
    }

    #[test]
    fn session_id_nonzero() {
        for _ in 0..10 {
            assert_ne!(generate_session_id(), 0);
        }
    }
}
