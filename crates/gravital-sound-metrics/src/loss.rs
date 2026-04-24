//! Tracking de pérdida y reordenamiento con bitmap de ventana fija.

use core::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use gravital_sound_core::constants::LOSS_WINDOW_SIZE;

/// Rastrea pérdidas dentro de una ventana deslizante de 64 paquetes.
///
/// La ventana se representa como `u64` (64 bits = 64 paquetes). Los bits a 1
/// marcan paquetes recibidos; los bits a 0 marcan huecos (pérdidas).
#[derive(Debug)]
pub struct LossTracker {
    /// Bitmap de los últimos 64 paquetes.
    bitmap: AtomicU64,
    /// Sequence del paquete más nuevo visto.
    high_seq: AtomicU32,
    /// Total de paquetes únicos contados.
    total_packets: AtomicU32,
    /// Total de reordering events.
    reorder_count: AtomicU32,
    /// Si aún no hay muestras.
    initialised: AtomicU32,
}

impl Default for LossTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl LossTracker {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bitmap: AtomicU64::new(0),
            high_seq: AtomicU32::new(0),
            total_packets: AtomicU32::new(0),
            reorder_count: AtomicU32::new(0),
            initialised: AtomicU32::new(0),
        }
    }

    /// Registra la recepción de un paquete con `sequence`.
    pub fn record(&self, sequence: u32) {
        let initialised = self.initialised.load(Ordering::Relaxed) != 0;
        if !initialised {
            self.high_seq.store(sequence, Ordering::Relaxed);
            self.bitmap.store(1, Ordering::Relaxed);
            self.total_packets.store(1, Ordering::Relaxed);
            self.initialised.store(1, Ordering::Relaxed);
            return;
        }

        let high = self.high_seq.load(Ordering::Relaxed);
        let diff = sequence.wrapping_sub(high) as i32;

        if diff > 0 {
            // Paquete más nuevo. Desplaza bitmap.
            let shift = (diff as u32).min(LOSS_WINDOW_SIZE);
            let mut bm = self.bitmap.load(Ordering::Relaxed);
            bm <<= shift;
            bm |= 1; // el paquete nuevo entra en la posición menos significativa.
            self.bitmap.store(bm, Ordering::Relaxed);
            self.high_seq.store(sequence, Ordering::Relaxed);
        } else if diff < 0 {
            // Paquete antiguo (reorder). Intenta colocar el bit correspondiente.
            let gap = -diff as u32;
            if gap < LOSS_WINDOW_SIZE {
                let mut bm = self.bitmap.load(Ordering::Relaxed);
                let bit = 1u64 << gap;
                if bm & bit == 0 {
                    bm |= bit;
                    self.bitmap.store(bm, Ordering::Relaxed);
                }
            }
            self.reorder_count.fetch_add(1, Ordering::Relaxed);
        }
        self.total_packets.fetch_add(1, Ordering::Relaxed);
    }

    /// Porcentaje de pérdida en la ventana actual (0.0..=100.0).
    #[must_use]
    pub fn loss_percent(&self) -> f32 {
        let bm = self.bitmap.load(Ordering::Relaxed);
        let initialised = self.initialised.load(Ordering::Relaxed) != 0;
        if !initialised {
            return 0.0;
        }
        let received = bm.count_ones();
        let window = LOSS_WINDOW_SIZE;
        let lost = window.saturating_sub(received);
        (lost as f32 / window as f32) * 100.0
    }

    /// Porcentaje aproximado de reordering (sobre total).
    #[must_use]
    pub fn reorder_percent(&self) -> f32 {
        let total = self.total_packets.load(Ordering::Relaxed);
        if total == 0 {
            return 0.0;
        }
        let reorder = self.reorder_count.load(Ordering::Relaxed);
        (reorder as f32 / total as f32) * 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_has_no_loss_reported() {
        let t = LossTracker::new();
        assert_eq!(t.loss_percent(), 0.0);
        assert_eq!(t.reorder_percent(), 0.0);
    }

    #[test]
    fn perfect_stream_no_loss() {
        let t = LossTracker::new();
        for i in 0..LOSS_WINDOW_SIZE {
            t.record(i);
        }
        assert_eq!(t.loss_percent(), 0.0);
    }

    #[test]
    fn detects_gap() {
        let t = LossTracker::new();
        for i in 0..10 {
            if i == 3 {
                continue; // perdido
            }
            t.record(i);
        }
        assert!(t.loss_percent() > 0.0);
    }

    #[test]
    fn reorder_counted() {
        let t = LossTracker::new();
        t.record(0);
        t.record(1);
        t.record(3);
        t.record(2); // fuera de orden
        assert!(t.reorder_percent() > 0.0);
    }
}
