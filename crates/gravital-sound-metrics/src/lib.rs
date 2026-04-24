//! Métricas de sesión: RTT, jitter, pérdida, reordenamiento y MOS estimado.
//!
//! Todas las métricas se almacenan en `AtomicU64`/`AtomicU32` para permitir
//! lectura desde cualquier hilo sin bloqueo.

pub mod counters;
pub mod jitter;
pub mod loss;
pub mod quality;
pub mod rtt;
pub mod snapshot;

pub use counters::Counters;
pub use jitter::JitterEstimator;
pub use loss::LossTracker;
pub use quality::estimate_mos;
pub use rtt::RttEstimator;
pub use snapshot::MetricsSnapshot;

/// Agregador top-level que compone todos los estimadores.
#[derive(Debug)]
pub struct Metrics {
    pub rtt: RttEstimator,
    pub jitter: JitterEstimator,
    pub loss: LossTracker,
    pub counters: Counters,
}

impl Default for Metrics {
    fn default() -> Self {
        Self::new()
    }
}

impl Metrics {
    #[must_use]
    pub fn new() -> Self {
        Self {
            rtt: RttEstimator::new(),
            jitter: JitterEstimator::new(),
            loss: LossTracker::new(),
            counters: Counters::new(),
        }
    }

    /// Produce un snapshot atómico de todas las métricas. El buffer-fill y el
    /// MOS se pasan externamente (dependen de componentes fuera del crate).
    #[must_use]
    pub fn snapshot(&self, buffer_fill_percent: f32) -> MetricsSnapshot {
        let rtt_ms = self.rtt.current_ms();
        let jitter_ms = self.jitter.current_ms();
        let loss_percent = self.loss.loss_percent();
        let reorder_percent = self.loss.reorder_percent();
        let estimated_mos = estimate_mos(rtt_ms, loss_percent, jitter_ms);
        MetricsSnapshot {
            rtt_ms,
            jitter_ms,
            loss_percent,
            reorder_percent,
            buffer_fill_percent,
            estimated_mos,
            packets_sent: self.counters.packets_sent(),
            packets_received: self.counters.packets_received(),
            bytes_sent: self.counters.bytes_sent(),
            bytes_received: self.counters.bytes_received(),
        }
    }
}
