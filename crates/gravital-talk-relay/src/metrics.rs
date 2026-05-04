//! Métricas Prometheus expuestas por el relay.

use prometheus::{IntCounter, IntCounterVec, IntGauge, Opts, Registry};

#[derive(Debug, Clone)]
pub struct RelayMetrics {
    pub registry: Registry,
    pub packets_in: IntCounter,
    pub packets_out: IntCounter,
    pub bytes_in: IntCounter,
    pub bytes_out: IntCounter,
    pub active_sessions: IntGauge,
    pub dropped: IntCounterVec,
    pub ws_connections: IntGauge,
}

impl Default for RelayMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl RelayMetrics {
    pub fn new() -> Self {
        let registry = Registry::new();
        let packets_in = IntCounter::with_opts(Opts::new(
            "gs_relay_packets_in_total",
            "Datagramas UDP/WS recibidos por el relay.",
        ))
        .unwrap();
        let packets_out = IntCounter::with_opts(Opts::new(
            "gs_relay_packets_out_total",
            "Datagramas reenviados al peer destino.",
        ))
        .unwrap();
        let bytes_in =
            IntCounter::with_opts(Opts::new("gs_relay_bytes_in_total", "Bytes recibidos."))
                .unwrap();
        let bytes_out =
            IntCounter::with_opts(Opts::new("gs_relay_bytes_out_total", "Bytes reenviados."))
                .unwrap();
        let active_sessions = IntGauge::with_opts(Opts::new(
            "gs_relay_active_sessions",
            "Número de session_id con al menos un peer activo.",
        ))
        .unwrap();
        let dropped = IntCounterVec::new(
            Opts::new(
                "gs_relay_dropped_total",
                "Datagramas descartados por motivo.",
            ),
            &["reason"],
        )
        .unwrap();
        let ws_connections = IntGauge::with_opts(Opts::new(
            "gs_relay_ws_connections",
            "Conexiones WebSocket abiertas.",
        ))
        .unwrap();

        registry.register(Box::new(packets_in.clone())).unwrap();
        registry.register(Box::new(packets_out.clone())).unwrap();
        registry.register(Box::new(bytes_in.clone())).unwrap();
        registry.register(Box::new(bytes_out.clone())).unwrap();
        registry
            .register(Box::new(active_sessions.clone()))
            .unwrap();
        registry.register(Box::new(dropped.clone())).unwrap();
        registry.register(Box::new(ws_connections.clone())).unwrap();

        Self {
            registry,
            packets_in,
            packets_out,
            bytes_in,
            bytes_out,
            active_sessions,
            dropped,
            ws_connections,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_registry_is_populated() {
        let m = RelayMetrics::new();
        m.packets_in.inc();
        m.bytes_in.inc_by(1500);
        m.dropped.with_label_values(&["malformed"]).inc();

        let metric_families = m.registry.gather();
        let names: Vec<_> = metric_families.iter().map(|mf| mf.get_name()).collect();
        assert!(names.contains(&"gs_relay_packets_in_total"));
        assert!(names.contains(&"gs_relay_dropped_total"));
    }
}
