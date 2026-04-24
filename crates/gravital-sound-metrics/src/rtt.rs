//! Estimador de RTT con EWMA.

use core::sync::atomic::{AtomicU32, Ordering};

/// EWMA del RTT. α = 1/8 (factor 0.125), idéntico al estimador de TCP.
///
/// Almacena los microsegundos como `AtomicU32` para lectura lock-free desde
/// cualquier hilo. El productor es serial.
#[derive(Debug)]
pub struct RttEstimator {
    /// RTT actual en microsegundos. `u32::MAX` cuando no hay muestras.
    rtt_us: AtomicU32,
}

impl Default for RttEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl RttEstimator {
    const ALPHA_NUM: u64 = 1;
    const ALPHA_DEN: u64 = 8;

    #[must_use]
    pub const fn new() -> Self {
        Self {
            rtt_us: AtomicU32::new(u32::MAX),
        }
    }

    /// Añade una muestra nueva. `sample_us` es la diferencia entre el
    /// timestamp del heartbeat enviado y el heartbeat-ack recibido.
    pub fn record(&self, sample_us: u32) {
        // Un solo productor: load actual, calcular EWMA, guardar.
        let prev = self.rtt_us.load(Ordering::Relaxed);
        let new = if prev == u32::MAX {
            sample_us
        } else {
            // EWMA: new = prev + α·(sample − prev) = prev·(1−α) + sample·α
            let prev64 = prev as u64;
            let sample64 = sample_us as u64;
            let diff = if sample64 > prev64 {
                let d = sample64 - prev64;
                prev64 + d * Self::ALPHA_NUM / Self::ALPHA_DEN
            } else {
                let d = prev64 - sample64;
                prev64.saturating_sub(d * Self::ALPHA_NUM / Self::ALPHA_DEN)
            };
            diff.min(u32::MAX as u64) as u32
        };
        self.rtt_us.store(new, Ordering::Relaxed);
    }

    /// RTT actual en microsegundos. Devuelve `None` si no hay muestras.
    #[must_use]
    pub fn current_us(&self) -> Option<u32> {
        let v = self.rtt_us.load(Ordering::Relaxed);
        if v == u32::MAX {
            None
        } else {
            Some(v)
        }
    }

    /// RTT actual en milisegundos como `f32` (0 si no hay muestras).
    #[must_use]
    pub fn current_ms(&self) -> f32 {
        self.current_us().map_or(0.0, |us| us as f32 / 1000.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_is_none() {
        let r = RttEstimator::new();
        assert_eq!(r.current_us(), None);
        assert_eq!(r.current_ms(), 0.0);
    }

    #[test]
    fn first_sample_becomes_initial() {
        let r = RttEstimator::new();
        r.record(10_000);
        assert_eq!(r.current_us(), Some(10_000));
    }

    #[test]
    fn ewma_smooths_samples() {
        let r = RttEstimator::new();
        r.record(10_000);
        r.record(20_000);
        // EWMA: 10000 + (20000 - 10000) / 8 = 11250.
        assert_eq!(r.current_us(), Some(11_250));
    }

    #[test]
    fn ewma_smooths_downward() {
        let r = RttEstimator::new();
        r.record(100_000);
        r.record(20_000);
        // EWMA: 100000 - (100000 - 20000)/8 = 90000.
        assert_eq!(r.current_us(), Some(90_000));
    }

    #[test]
    fn current_ms_converts() {
        let r = RttEstimator::new();
        r.record(5_500);
        assert!((r.current_ms() - 5.5).abs() < 0.001);
    }
}
