//! Routing table del relay: mapea `session_id → endpoints`.
//!
//! Cada session_id puede tener hasta 2 endpoints (peer A y peer B).
//! Cuando llega un datagrama de un endpoint conocido, se reenvía al otro.
//! Un endpoint puede ser una `SocketAddr` (UDP) o un sender de WebSocket.

use std::net::SocketAddr;
use std::time::Instant;

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::mpsc;

use crate::metrics::RelayMetrics;

/// Identifica un peer en una sesión.
#[derive(Debug, Clone)]
pub enum SessionEndpoint {
    /// Peer con UDP. Reenviar via `UdpTransport::send_to`.
    Udp(SocketAddr),
    /// Peer con WebSocket. Reenviar empujando bytes al `mpsc::Sender`.
    WebSocket(mpsc::UnboundedSender<Bytes>),
}

impl SessionEndpoint {
    pub fn is_udp(&self) -> bool {
        matches!(self, Self::Udp(_))
    }
    pub fn is_ws(&self) -> bool {
        matches!(self, Self::WebSocket(_))
    }
    /// Considera dos endpoints "iguales" si vienen del mismo origen físico.
    pub fn matches(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Udp(a), Self::Udp(b)) => a == b,
            (Self::WebSocket(a), Self::WebSocket(b)) => a.same_channel(b),
            _ => false,
        }
    }
}

#[derive(Debug)]
pub struct RouteEntry {
    pub a: Option<SessionEndpoint>,
    pub b: Option<SessionEndpoint>,
    pub last_activity: Instant,
}

impl RouteEntry {
    fn new() -> Self {
        Self {
            a: None,
            b: None,
            last_activity: Instant::now(),
        }
    }
}

#[derive(Debug)]
pub struct Router {
    routes: DashMap<u32, RouteEntry>,
    max_sessions: usize,
    metrics: RelayMetrics,
}

impl Router {
    pub fn new(max_sessions: usize, metrics: RelayMetrics) -> Self {
        Self {
            routes: DashMap::new(),
            max_sessions,
            metrics,
        }
    }

    pub fn metrics(&self) -> &RelayMetrics {
        &self.metrics
    }

    /// Resultado de procesar un datagrama entrante.
    /// `Forward(target)`: reenviar al endpoint indicado.
    /// `Registered`: registrado como nuevo peer pero sin destino aún.
    /// `Dropped`: rechazado (sesión llena, etc).
    pub fn route(&self, session_id: u32, from: SessionEndpoint) -> RouteDecision {
        if session_id == 0 {
            self.metrics
                .dropped
                .with_label_values(&["zero_session"])
                .inc();
            return RouteDecision::Dropped;
        }

        // Caso fast-path: la entrada ya existe.
        if let Some(mut entry) = self.routes.get_mut(&session_id) {
            entry.last_activity = Instant::now();

            // ¿Es el endpoint A?
            if let Some(ref a) = entry.a {
                if a.matches(&from) {
                    return match entry.b.clone() {
                        Some(b) => RouteDecision::Forward(b),
                        None => RouteDecision::Registered,
                    };
                }
            }
            // ¿Es el endpoint B?
            if let Some(ref b) = entry.b {
                if b.matches(&from) {
                    return match entry.a.clone() {
                        Some(a) => RouteDecision::Forward(a),
                        None => RouteDecision::Registered,
                    };
                }
            }
            // Es un tercer peer intentando intervenir — descartar.
            if entry.a.is_some() && entry.b.is_some() {
                self.metrics
                    .dropped
                    .with_label_values(&["session_full"])
                    .inc();
                return RouteDecision::Dropped;
            }
            // Slot libre, asignar como B.
            entry.b = Some(from);
            return RouteDecision::Registered;
        }

        // Sesión nueva.
        if self.routes.len() >= self.max_sessions {
            self.metrics
                .dropped
                .with_label_values(&["max_sessions"])
                .inc();
            return RouteDecision::Dropped;
        }
        let mut entry = RouteEntry::new();
        entry.a = Some(from);
        self.routes.insert(session_id, entry);
        self.metrics.active_sessions.set(self.routes.len() as i64);
        RouteDecision::Registered
    }

    /// Elimina entradas inactivas. Devuelve cuántas se removieron.
    pub fn evict_idle(&self, max_age_secs: u64) -> usize {
        let now = Instant::now();
        let max_age = std::time::Duration::from_secs(max_age_secs);
        let mut removed = 0usize;
        self.routes.retain(|_, entry| {
            let keep = now.saturating_duration_since(entry.last_activity) < max_age;
            if !keep {
                removed += 1;
            }
            keep
        });
        if removed > 0 {
            self.metrics.active_sessions.set(self.routes.len() as i64);
        }
        removed
    }

    pub fn active_sessions(&self) -> usize {
        self.routes.len()
    }
}

#[derive(Debug)]
pub enum RouteDecision {
    /// Reenviar al endpoint indicado.
    Forward(SessionEndpoint),
    /// Endpoint registrado, no hay destino todavía.
    Registered,
    /// Datagrama descartado.
    Dropped,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep_udp(s: &str) -> SessionEndpoint {
        SessionEndpoint::Udp(s.parse().unwrap())
    }

    #[test]
    fn first_packet_registers_endpoint_a() {
        let router = Router::new(100, RelayMetrics::new());
        let r = router.route(42, ep_udp("127.0.0.1:1000"));
        assert!(matches!(r, RouteDecision::Registered));
        assert_eq!(router.active_sessions(), 1);
    }

    #[test]
    fn second_peer_completes_session() {
        let router = Router::new(100, RelayMetrics::new());
        router.route(42, ep_udp("127.0.0.1:1000"));
        let r = router.route(42, ep_udp("127.0.0.1:2000"));
        assert!(matches!(r, RouteDecision::Registered));

        // Tercer datagrama de A debe reenviarse a B.
        let r = router.route(42, ep_udp("127.0.0.1:1000"));
        match r {
            RouteDecision::Forward(SessionEndpoint::Udp(addr)) => {
                assert_eq!(addr.port(), 2000);
            }
            other => panic!("expected Forward to B, got {other:?}"),
        }

        // Y de B debe reenviarse a A.
        let r = router.route(42, ep_udp("127.0.0.1:2000"));
        match r {
            RouteDecision::Forward(SessionEndpoint::Udp(addr)) => {
                assert_eq!(addr.port(), 1000);
            }
            other => panic!("expected Forward to A, got {other:?}"),
        }
    }

    #[test]
    fn third_peer_in_full_session_is_dropped() {
        let router = Router::new(100, RelayMetrics::new());
        router.route(42, ep_udp("127.0.0.1:1000"));
        router.route(42, ep_udp("127.0.0.1:2000"));
        let r = router.route(42, ep_udp("127.0.0.1:3000"));
        assert!(matches!(r, RouteDecision::Dropped));
    }

    #[test]
    fn zero_session_id_dropped() {
        let router = Router::new(100, RelayMetrics::new());
        let r = router.route(0, ep_udp("127.0.0.1:1000"));
        assert!(matches!(r, RouteDecision::Dropped));
    }

    #[test]
    fn max_sessions_enforced() {
        let router = Router::new(2, RelayMetrics::new());
        router.route(1, ep_udp("127.0.0.1:1000"));
        router.route(2, ep_udp("127.0.0.1:2000"));
        let r = router.route(3, ep_udp("127.0.0.1:3000"));
        assert!(matches!(r, RouteDecision::Dropped));
    }
}
