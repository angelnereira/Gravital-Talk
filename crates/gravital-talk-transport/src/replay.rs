//! Ventana anti-replay para secuencias de paquete (RFC 6479-style).
//!
//! Después del handshake todos los paquetes llevan `sequence` en el header
//! (texto claro, incluido como AAD del AEAD). Un atacante puede capturar un
//! datagrama válido y reinyectarlo; esta ventana descarta:
//!
//! - Duplicados exactos del `sequence` más reciente.
//! - Secuencias ya vistas dentro de la ventana.
//! - Paquetes demasiado viejos (fuera de la ventana).
//!
//! La ventana cubre `WINDOW` secuencias (4096) con un bitmap de `u64`.

/// Tamaño del bitmap en palabras de 64 bits (`WINDOW = 4096` secuencias).
const BITMAP_WORDS: usize = 64;
/// Número de secuencias cubiertas por la ventana.
const WINDOW: usize = BITMAP_WORDS * 64;

/// Mapa de bits deslizante sobre secuencias `u32` (con wrap-around).
///
/// Convención interna: el bit de la posición `0` corresponde a la secuencia
/// más reciente (`highest`); las anteriores ocupan posiciones crecientes.
/// Avanzar el `highest` desplaza el bitmap a la izquierda.
#[derive(Debug)]
pub struct ReplayWindow {
    highest: u32,
    /// `false` hasta el primer paquete: cualquier secuencia inicial es válida.
    initialized: bool,
    /// `bitmap[w]` cubre de `w*64` a `w*64+63` posiciones desde la más
    /// reciente (LSB = posición 0).
    bitmap: [u64; BITMAP_WORDS],
}

impl Default for ReplayWindow {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayWindow {
    pub const fn new() -> Self {
        Self {
            highest: 0,
            initialized: false,
            bitmap: [0; BITMAP_WORDS],
        }
    }

    /// Marca la secuencia como vista y devuelve `true` si debe aceptarse.
    ///
    /// Devuelve `false` (drop) para duplicados, secuencias viejas fuera de la
    /// ventana y paquetes del pasado ya vistos. Un salto muy grande hacia el
    /// futuro reinicia la ventana (nuevo stream tras un corte).
    pub fn check_and_mark(&mut self, seq: u32) -> bool {
        // Primer paquete de la ventana: la secuencia inicial es arbitraria.
        if !self.initialized {
            self.initialized = true;
            self.highest = seq;
            self.mark(0);
            return true;
        }

        let diff = seq.wrapping_sub(self.highest);

        if diff == 0 {
            // Duplicado exacto del más reciente.
            return false;
        }

        if diff < (1 << 31) {
            // `seq` es más nuevo que `highest` (menos de media vuelta).
            if u64::from(diff) >= WINDOW as u64 {
                // Salto abrupto: descartar el bitmap y fijar nuevo piso.
                self.bitmap = [0; BITMAP_WORDS];
                self.highest = seq;
                self.mark(0);
                return true;
            }
            self.shift_left(diff as usize);
            self.highest = seq;
            self.mark(0);
            return true;
        }

        // `seq` es más viejo que `highest`.
        let age = u64::from(self.highest.wrapping_sub(seq));
        if age >= WINDOW as u64 {
            return false;
        }
        !self.is_marked(age as usize)
    }

    const fn mark(&mut self, age: usize) {
        let word = age / 64;
        let bit = age % 64;
        self.bitmap[word] |= 1 << bit;
    }

    const fn is_marked(&self, age: usize) -> bool {
        let word = age / 64;
        let bit = age % 64;
        self.bitmap[word] & (1 << bit) != 0
    }

    /// Desplaza el bitmap `n` posiciones hacia la izquierda descartando lo
    /// que se salga de la ventana (equivale a envejecer las marcas).
    fn shift_left(&mut self, n: usize) {
        if n == 0 {
            return;
        }
        let words = n / 64;
        let bits = n % 64;

        if words >= BITMAP_WORDS {
            self.bitmap = [0; BITMAP_WORDS];
            return;
        }

        // De la palabra más significativa a la menos, leyendo siempre de
        // índices inferiores (aún no sobrescritos).
        for w in (0..BITMAP_WORDS).rev() {
            let mut val = if w >= words {
                self.bitmap[w - words]
            } else {
                0
            };
            if bits != 0 {
                val <<= bits;
                if w > words {
                    val |= self.bitmap[w - words - 1] >> (64 - bits);
                }
            }
            self.bitmap[w] = val;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_increasing_sequences() {
        let mut w = ReplayWindow::new();
        for seq in [1u32, 2, 3, 100, 500, 4096] {
            assert!(w.check_and_mark(seq), "seq {seq} debería aceptarse");
        }
    }

    #[test]
    fn rejects_exact_duplicate() {
        let mut w = ReplayWindow::new();
        assert!(w.check_and_mark(42));
        assert!(!w.check_and_mark(42), "duplicado debe descartarse");
    }

    #[test]
    fn rejects_old_seen_packet() {
        let mut w = ReplayWindow::new();
        for seq in [10u32, 20, 30] {
            assert!(w.check_and_mark(seq));
        }
        assert!(
            !w.check_and_mark(20),
            "paquete antiguo ya visto debe dropearse"
        );
        assert!(!w.check_and_mark(10));
    }

    #[test]
    fn rejects_very_old_packet() {
        let mut w = ReplayWindow::new();
        assert!(w.check_and_mark(10_000));
        // 5000 está por debajo de 10000 - 4096 → fuera de ventana.
        assert!(!w.check_and_mark(5000));
    }

    #[test]
    fn window_slides_and_rejects_old() {
        let mut w = ReplayWindow::new();
        for seq in 1..=100 {
            assert!(w.check_and_mark(seq), "seq {seq}");
        }
        // Dentro de la ventana sigue marcado.
        assert!(!w.check_and_mark(50));
        // Salto grande: lo anterior queda fuera de la ventana.
        assert!(w.check_and_mark(6000));
        assert!(!w.check_and_mark(50)); // ahora demasiado viejo
        assert!(!w.check_and_mark(6000)); // duplicado
        assert!(w.check_and_mark(6001)); // la ventana sigue avanzando
    }

    #[test]
    fn handles_wraparound() {
        let mut w = ReplayWindow::new();
        let near_max = u32::MAX - 5;
        assert!(w.check_and_mark(near_max));
        assert!(w.check_and_mark(u32::MAX));
        // Tras el wrap, 0 y 3 son "nuevos" (media vuelta).
        assert!(w.check_and_mark(0));
        assert!(w.check_and_mark(3));
        assert!(!w.check_and_mark(0));
        assert!(!w.check_and_mark(u32::MAX)); // viejo tras el wrap
    }

    #[test]
    fn huge_forward_jump_resets_window() {
        let mut w = ReplayWindow::new();
        assert!(w.check_and_mark(100));
        // Salto > ventana: se reinicia y acepta.
        assert!(w.check_and_mark(200_000));
        assert!(!w.check_and_mark(100)); // quedó fuera de la nueva ventana
    }
}
