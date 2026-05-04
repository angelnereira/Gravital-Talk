//! Snapshot inmutable de métricas.

/// Snapshot atómico de todas las métricas de una sesión.
///
/// Layout `#[repr(C)]` para compatibilidad directa con `GsMetrics` en la FFI.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct MetricsSnapshot {
    pub rtt_ms: f32,
    pub jitter_ms: f32,
    pub loss_percent: f32,
    pub reorder_percent: f32,
    pub buffer_fill_percent: f32,
    pub estimated_mos: f32,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        let s = MetricsSnapshot::default();
        assert_eq!(s.rtt_ms, 0.0);
        assert_eq!(s.packets_sent, 0);
    }

    #[test]
    fn size_is_reasonable() {
        // 6 f32 (24) + 4 u64 (32) + padding para alineación u64 = 64 bytes.
        let size = core::mem::size_of::<MetricsSnapshot>();
        assert!(
            size <= 64,
            "MetricsSnapshot grew unexpectedly: {size} bytes"
        );
    }
}
