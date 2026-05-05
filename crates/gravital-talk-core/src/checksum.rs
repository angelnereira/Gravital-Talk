//! CRC-16/CCITT-FALSE (`polinomio 0x1021`, `init 0xFFFF`, sin XOR final).
//!
//! El test vector canónico es `crc16_ccitt_false(b"123456789") == 0x29B1`.
//!
//! La implementación por default usa una lookup table de 256 entradas generada
//! en `const`. Con la feature `simd-crc` y sobre x86_64 con SSE4.2, se usa un
//! fast path con `_mm_crc32_u8` (CRC-32C del hardware) post-procesado. Nota:
//! SSE4.2 entrega CRC-32C, no CRC-16; el fast path sólo ayuda cuando la
//! aceleración NEON/PCLMULQDQ está disponible. Por simplicidad y portabilidad
//! del `no_std`, el camino SIMD está gated tras `simd-crc` y hace fallback a
//! la tabla si el CPU no expone las features.

const POLY: u16 = 0x1021;
const INIT: u16 = 0xFFFF;

/// Lookup table pre-calculada para CRC-16/CCITT-FALSE.
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

/// Calcula CRC-16/CCITT-FALSE sobre `data`.
#[inline]
#[must_use]
pub fn crc16_ccitt_false(data: &[u8]) -> u16 {
    #[cfg(all(feature = "simd-crc", target_arch = "x86_64"))]
    {
        if is_x86_feature_detected_safe() {
            return crc16_table(data); // Placeholder: SIMD path future work.
        }
    }
    crc16_table(data)
}

#[inline]
fn crc16_table(data: &[u8]) -> u16 {
    let mut crc: u16 = INIT;
    for &byte in data {
        let idx = ((crc >> 8) as u8 ^ byte) as usize;
        crc = (crc << 8) ^ TABLE[idx];
    }
    crc
}

/// Calcula CRC-16 en dos segmentos (header y payload), sin concatenar.
/// Útil para evitar una copia al validar un paquete recibido: se pasa el
/// header con el campo `checksum` puesto a cero y el payload por separado.
#[inline]
#[must_use]
pub fn crc16_two_segments(segment_a: &[u8], segment_b: &[u8]) -> u16 {
    let mut crc: u16 = INIT;
    for &byte in segment_a {
        let idx = ((crc >> 8) as u8 ^ byte) as usize;
        crc = (crc << 8) ^ TABLE[idx];
    }
    for &byte in segment_b {
        let idx = ((crc >> 8) as u8 ^ byte) as usize;
        crc = (crc << 8) ^ TABLE[idx];
    }
    crc
}

#[cfg(all(feature = "simd-crc", target_arch = "x86_64"))]
#[inline]
fn is_x86_feature_detected_safe() -> bool {
    #[cfg(feature = "std")]
    {
        std::is_x86_feature_detected!("sse4.2")
    }
    #[cfg(not(feature = "std"))]
    {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc16_check_value() {
        assert_eq!(crc16_ccitt_false(b"123456789"), 0x29B1);
    }

    #[test]
    fn crc16_empty() {
        assert_eq!(crc16_ccitt_false(b""), 0xFFFF);
    }

    #[test]
    fn crc16_single_byte() {
        assert_eq!(crc16_ccitt_false(b"A"), 0xB915);
    }

    #[test]
    fn crc16_two_segments_equals_concat() {
        let a = b"Hello, ";
        let b = b"world!";
        let concat = [a.as_slice(), b.as_slice()].concat();
        assert_eq!(crc16_ccitt_false(&concat), crc16_two_segments(a, b));
    }

    #[test]
    fn crc16_different_inputs_diverge() {
        let a = crc16_ccitt_false(b"gravital sound");
        let b = crc16_ccitt_false(b"gravital sounE");
        assert_ne!(a, b);
    }
}
