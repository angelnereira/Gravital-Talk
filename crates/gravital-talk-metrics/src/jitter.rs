//! Estimador de jitter según RFC 3550 §A.8.
//!
//! J = J + (|D(i-1,i)| - J) / 16
//! donde D(i-1, i) = (Ri - Si) - (Ri-1 - Si-1).

use core::sync::atomic::{AtomicI64, AtomicU32, Ordering};

#[derive(Debug)]
pub struct JitterEstimator {
    /// Jitter acumulado en microsegundos (shift izquierdo 4 bits para fixed-point).
    jitter_us_q4: AtomicU32,
    /// (Ri − Si) de la muestra previa, en microsegundos. `i64::MIN` = sin muestra.
    prev_diff_us: AtomicI64,
}

impl Default for JitterEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl JitterEstimator {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            jitter_us_q4: AtomicU32::new(0),
            prev_diff_us: AtomicI64::new(i64::MIN),
        }
    }

    /// Registra una muestra. `send_ts_us` es el timestamp del emisor extraído
    /// del header, `recv_ts_us` es el instante local de llegada.
    pub fn record(&self, send_ts_us: u64, recv_ts_us: u64) {
        let current_diff = recv_ts_us as i64 - send_ts_us as i64;
        let prev = self.prev_diff_us.load(Ordering::Relaxed);
        if prev == i64::MIN {
            self.prev_diff_us.store(current_diff, Ordering::Relaxed);
            return;
        }
        let d = (current_diff - prev).unsigned_abs();
        // jitter = jitter + (|d| − jitter) / 16.
        let j = self.jitter_us_q4.load(Ordering::Relaxed) as u64;
        // Representación: j ya está sin shift. Aplicamos filtro simple.
        let new_j = if d > j {
            j + (d - j) / 16
        } else {
            j - (j - d) / 16
        };
        let clamped = new_j.min(u32::MAX as u64) as u32;
        self.jitter_us_q4.store(clamped, Ordering::Relaxed);
        self.prev_diff_us.store(current_diff, Ordering::Relaxed);
    }

    #[must_use]
    pub fn current_us(&self) -> u32 {
        self.jitter_us_q4.load(Ordering::Relaxed)
    }

    #[must_use]
    pub fn current_ms(&self) -> f32 {
        self.current_us() as f32 / 1000.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_jitter_zero() {
        let j = JitterEstimator::new();
        assert_eq!(j.current_us(), 0);
    }

    #[test]
    fn stable_stream_no_jitter() {
        let j = JitterEstimator::new();
        // Paquetes espaciados exactamente 20 ms, llegan espaciados 20 ms.
        for i in 0..10 {
            let send = (i * 20_000) as u64;
            let recv = (1_000_000 + i * 20_000) as u64;
            j.record(send, recv);
        }
        assert_eq!(j.current_us(), 0);
    }

    #[test]
    fn jittery_stream_detected() {
        let j = JitterEstimator::new();
        // Paquetes emitidos cada 20 ms pero recibidos con jitter de ±5 ms.
        let arrivals = [0, 25_000, 40_000, 67_000, 80_000, 102_000];
        let sends = [0, 20_000, 40_000, 60_000, 80_000, 100_000];
        for (s, r) in sends.iter().zip(arrivals.iter()) {
            j.record(*s, *r);
        }
        assert!(j.current_us() > 0);
    }
}
