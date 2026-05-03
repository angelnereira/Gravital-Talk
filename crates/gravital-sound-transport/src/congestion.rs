//! Control de congestión AIMD para Gravital Sound.
//!
//! Algoritmo: AIMD (Additive Increase, Multiplicative Decrease).
//! - Pérdida alta (> 5 %): reduce bitrate × 0.85
//! - RTT alto (> 150 ms): reduce bitrate × 0.95
//! - Sin pérdida ni RTT alto: incremento aditivo de 2 kbps por ciclo
//!
//! El estado es thread-safe via `AtomicU32`; no requiere lock en el hot path.

use std::sync::atomic::{AtomicU32, Ordering};

use gravital_sound_core::constants::{CONGESTION_AIMD_INCREMENT, CONGESTION_MIN_BITRATE};

/// Umbral de pérdida (5 %) por encima del cual se aplica reducción agresiva.
const LOSS_THRESHOLD: f32 = 0.05;

/// Umbral de RTT (150 ms en microsegundos) para reducción suave.
const RTT_THRESHOLD_US: u64 = 150_000;

/// Controller de congestión para una sesión Gravital Sound.
#[derive(Debug)]
pub struct CongestionController {
    current_bitrate: AtomicU32,
    min_bitrate: u32,
    max_bitrate: u32,
}

impl CongestionController {
    /// Crea un controller con `initial_bitrate` clampado al rango `[min, max]`.
    #[must_use]
    pub fn new(initial_bitrate: u32, min_bitrate: u32, max_bitrate: u32) -> Self {
        let min_bitrate = min_bitrate.max(CONGESTION_MIN_BITRATE);
        let clamped = initial_bitrate.clamp(min_bitrate, max_bitrate);
        Self {
            current_bitrate: AtomicU32::new(clamped),
            min_bitrate,
            max_bitrate,
        }
    }

    /// Bitrate actual estimado (bps).
    #[inline]
    #[must_use]
    pub fn current_bitrate(&self) -> u32 {
        self.current_bitrate.load(Ordering::Relaxed)
    }

    /// Actualiza el bitrate en base a las métricas del último intervalo.
    ///
    /// - `loss_rate`: fracción de paquetes perdidos en `[0.0, 1.0]`.
    /// - `rtt_us`: RTT estimado en microsegundos.
    /// - `jitter_us`: jitter estimado en microsegundos (reservado para uso futuro).
    pub fn update(&self, loss_rate: f32, rtt_us: u64, _jitter_us: u64) {
        let current = self.current_bitrate.load(Ordering::Relaxed) as f64;

        let new_bitrate = if loss_rate > LOSS_THRESHOLD {
            // Pérdida significativa: reducción multiplicativa agresiva.
            (current * 0.85) as u32
        } else if rtt_us > RTT_THRESHOLD_US {
            // RTT alto: reducción multiplicativa suave.
            (current * 0.95) as u32
        } else {
            // Sin congestión: incremento aditivo.
            (current as u32).saturating_add(CONGESTION_AIMD_INCREMENT)
        };

        let clamped = new_bitrate.clamp(self.min_bitrate, self.max_bitrate);
        self.current_bitrate.store(clamped, Ordering::Relaxed);
    }

    /// Fuerza el bitrate a un valor específico (ej: recibido via ControlBitrate).
    pub fn set_bitrate(&self, bitrate: u32) {
        let clamped = bitrate.clamp(self.min_bitrate, self.max_bitrate);
        self.current_bitrate.store(clamped, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn high_loss_decreases_bitrate() {
        let cc = CongestionController::new(64_000, 8_000, 128_000);
        cc.update(0.10, 50_000, 0); // 10% loss
        assert!(cc.current_bitrate() < 64_000);
    }

    #[test]
    fn high_rtt_decreases_bitrate() {
        let cc = CongestionController::new(64_000, 8_000, 128_000);
        cc.update(0.0, 200_000, 0); // RTT = 200ms
        assert!(cc.current_bitrate() < 64_000);
    }

    #[test]
    fn no_congestion_increases_bitrate() {
        let cc = CongestionController::new(32_000, 8_000, 128_000);
        cc.update(0.0, 50_000, 0); // sin pérdida, RTT bajo
        assert!(cc.current_bitrate() > 32_000);
    }

    #[test]
    fn clamps_to_min() {
        let cc = CongestionController::new(8_000, 8_000, 128_000);
        // Muchas reducciones no bajan del mínimo.
        for _ in 0..100 {
            cc.update(0.99, 999_999, 0);
        }
        assert_eq!(cc.current_bitrate(), 8_000);
    }

    #[test]
    fn clamps_to_max() {
        let cc = CongestionController::new(127_000, 8_000, 128_000);
        // Muchos incrementos no superan el máximo.
        for _ in 0..100 {
            cc.update(0.0, 0, 0);
        }
        assert_eq!(cc.current_bitrate(), 128_000);
    }

    #[test]
    fn set_bitrate_respects_bounds() {
        let cc = CongestionController::new(64_000, 8_000, 128_000);
        cc.set_bitrate(200_000);
        assert_eq!(cc.current_bitrate(), 128_000);
        cc.set_bitrate(1_000);
        assert_eq!(cc.current_bitrate(), 8_000);
    }
}
