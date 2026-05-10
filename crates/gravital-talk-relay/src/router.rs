//! Routing table del relay: mapea `session_id → peers`.
//!
//! Soporta grupos de hasta `max_peers_per_session` participantes.
//! Cuando llega un datagrama de un peer conocido, se retorna la lista
//! de todos los demás peers del grupo para broadcast.
//!
//! ## Floor Arbitration
//!
//! El relay actúa como árbitro de floor control. Cuando recibe un
//! `FloorRequest` (0x40), evalúa si el floor está libre y retorna la
//! decisión apropiada. El loop UDP actúa sobre esa decisión enviando
//! `FloorGrant` (0x41) o `FloorDeny` (0x42) de vuelta al solicitante,
//! y `FloorTaken` (0x44) a los demás peers.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::mpsc;

use crate::metrics::RelayMetrics;

/// Timeout de transmisión máxima: si el holder no libera en 30s, el relay
/// auto-libera el floor. Estándar 3GPP MCX.
const FLOOR_MAX_HOLD_SECS: u64 = 30;

/// Identifica un peer dentro de un grupo.
#[derive(Debug, Clone)]
pub enum SessionEndpoint {
    Udp(SocketAddr),
    WebSocket(mpsc::UnboundedSender<Bytes>),
}

impl SessionEndpoint {
    pub fn is_udp(&self) -> bool {
        matches!(self, Self::Udp(_))
    }
    pub fn is_ws(&self) -> bool {
        matches!(self, Self::WebSocket(_))
    }

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
    /// Todos los peers activos de esta sesión/grupo.
    pub peers: Vec<SessionEndpoint>,
    pub last_activity: Instant,
    /// Quien tiene el floor actualmente (None = libre).
    pub floor_holder: Option<SocketAddr>,
    /// Momento en que se concedió el floor.
    pub floor_granted_at: Option<Instant>,
}

impl RouteEntry {
    fn new() -> Self {
        Self {
            peers: Vec::new(),
            last_activity: Instant::now(),
            floor_holder: None,
            floor_granted_at: None,
        }
    }

    /// `true` si nadie tiene el floor (o el holder superó el timeout).
    fn floor_is_free(&self) -> bool {
        match (self.floor_holder, self.floor_granted_at) {
            (None, _) => true,
            (Some(_), Some(granted)) => {
                granted.elapsed() > Duration::from_secs(FLOOR_MAX_HOLD_SECS)
            }
            (Some(_), None) => false,
        }
    }
}

#[derive(Debug)]
pub struct Router {
    routes: DashMap<u32, RouteEntry>,
    rooms: DashMap<String, u32>,
    max_sessions: usize,
    max_peers_per_session: usize,
    metrics: RelayMetrics,
}

impl Router {
    pub fn new(max_sessions: usize, max_peers_per_session: usize, metrics: RelayMetrics) -> Self {
        Self {
            routes: DashMap::new(),
            rooms: DashMap::new(),
            max_sessions,
            max_peers_per_session,
            metrics,
        }
    }

    pub fn metrics(&self) -> &RelayMetrics {
        &self.metrics
    }

    /// Registra el datagrama entrante y devuelve la lista de peers destino.
    ///
    /// - `Broadcast(targets)` — reenviar a todos los targets de la lista.
    /// - `Registered` — primer o nuevo peer; sin destinos todavía.
    /// - `Dropped` — rechazado (sesión llena, session_id=0, max_sessions).
    pub fn route(&self, session_id: u32, from: SessionEndpoint) -> RouteDecision {
        if session_id == 0 {
            self.metrics.dropped.with_label_values(&["zero_session"]).inc();
            return RouteDecision::Dropped;
        }

        if let Some(mut entry) = self.routes.get_mut(&session_id) {
            entry.last_activity = Instant::now();

            // ¿Ya conocemos a este peer?
            let known = entry.peers.iter().any(|p| p.matches(&from));
            if known {
                // Devolver todos los demás como destinos de broadcast.
                let targets: Vec<SessionEndpoint> = entry
                    .peers
                    .iter()
                    .filter(|p| !p.matches(&from))
                    .cloned()
                    .collect();
                return if targets.is_empty() {
                    RouteDecision::Registered
                } else {
                    RouteDecision::Broadcast(targets)
                };
            }

            // Peer nuevo — ¿hay espacio?
            if entry.peers.len() >= self.max_peers_per_session {
                self.metrics.dropped.with_label_values(&["session_full"]).inc();
                return RouteDecision::Dropped;
            }

            // Registrar el peer y devolver los peers ya existentes para broadcast.
            let existing: Vec<SessionEndpoint> = entry.peers.iter().cloned().collect();
            entry.peers.push(from);
            return if existing.is_empty() {
                RouteDecision::Registered
            } else {
                RouteDecision::Broadcast(existing)
            };
        }

        // Sesión nueva.
        if self.routes.len() >= self.max_sessions {
            self.metrics.dropped.with_label_values(&["max_sessions"]).inc();
            return RouteDecision::Dropped;
        }
        let mut entry = RouteEntry::new();
        entry.peers.push(from);
        self.routes.insert(session_id, entry);
        self.metrics.active_sessions.set(self.routes.len() as i64);
        RouteDecision::Registered
    }

    /// Elimina peers cuyo canal WS está cerrado y sesiones inactivas.
    pub fn evict_idle(&self, max_age_secs: u64) -> usize {
        let now = Instant::now();
        let max_age = std::time::Duration::from_secs(max_age_secs);
        let mut removed = 0usize;
        self.routes.retain(|_, entry| {
            // Limpiar peers WS desconectados.
            entry.peers.retain(|p| match p {
                SessionEndpoint::WebSocket(tx) => !tx.is_closed(),
                SessionEndpoint::Udp(_) => true,
            });
            let keep = !entry.peers.is_empty()
                && now.saturating_duration_since(entry.last_activity) < max_age;
            if !keep { removed += 1; }
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

    pub fn peer_count(&self, session_id: u32) -> usize {
        self.routes.get(&session_id).map(|e| e.peers.len()).unwrap_or(0)
    }

    // ── Floor Arbitration ────────────────────────────────────────────────────

    /// Procesa una solicitud de floor.
    ///
    /// Si el floor está libre, lo concede al solicitante y retorna `Granted`
    /// con la lista de peers a notificar. Si está ocupado, retorna `Denied`.
    pub fn floor_request(&self, session_id: u32, from: SocketAddr) -> FloorDecision {
        let Some(mut entry) = self.routes.get_mut(&session_id) else {
            return FloorDecision::Unknown;
        };
        let is_known = entry.peers.iter().any(|p| p.matches(&SessionEndpoint::Udp(from)));
        if !is_known {
            return FloorDecision::Unknown;
        }

        // Auto-liberar si el holder superó el timeout.
        if !entry.floor_is_free() {
            if let (Some(holder), Some(granted)) = (entry.floor_holder, entry.floor_granted_at) {
                if granted.elapsed() > Duration::from_secs(FLOOR_MAX_HOLD_SECS) {
                    tracing::info!(session_id, ?holder, "floor auto-released (timeout)");
                    entry.floor_holder = None;
                    entry.floor_granted_at = None;
                }
            }
        }

        if !entry.floor_is_free() {
            return FloorDecision::Denied;
        }

        entry.floor_holder = Some(from);
        entry.floor_granted_at = Some(Instant::now());

        let others: Vec<SessionEndpoint> = entry
            .peers
            .iter()
            .filter(|p| !p.matches(&SessionEndpoint::Udp(from)))
            .cloned()
            .collect();
        FloorDecision::Granted { others }
    }

    /// Procesa una liberación de floor.
    pub fn floor_release(&self, session_id: u32, from: SocketAddr) -> FloorDecision {
        let Some(mut entry) = self.routes.get_mut(&session_id) else {
            return FloorDecision::Unknown;
        };
        // Solo el holder puede liberar (o se permite si el timeout expiró).
        let is_holder = entry.floor_holder == Some(from) || entry.floor_is_free();
        if !is_holder {
            return FloorDecision::Denied;
        }
        entry.floor_holder = None;
        entry.floor_granted_at = None;

        let others: Vec<SessionEndpoint> = entry
            .peers
            .iter()
            .filter(|p| !p.matches(&SessionEndpoint::Udp(from)))
            .cloned()
            .collect();
        FloorDecision::Released { others }
    }

    /// Devuelve el holder actual del floor para una sesión.
    pub fn floor_holder(&self, session_id: u32) -> Option<SocketAddr> {
        self.routes.get(&session_id)?.floor_holder
    }

    // ── Rooms API ────────────────────────────────────────────────────────────

    /// Registra un código de sala → session_id. Devuelve false si el código ya existe.
    pub fn register_room(&self, code: String, session_id: u32) -> bool {
        if self.rooms.contains_key(&code) {
            return false;
        }
        self.rooms.insert(code, session_id);
        true
    }

    /// Resuelve un código de sala a session_id.
    pub fn resolve_room(&self, code: &str) -> Option<u32> {
        self.rooms.get(code).map(|v| *v)
    }

    /// Elimina un código de sala.
    pub fn remove_room(&self, code: &str) -> bool {
        self.rooms.remove(code).is_some()
    }

    /// Lista todos los códigos activos con su session_id y peer_count.
    pub fn list_rooms(&self) -> Vec<(String, u32, usize)> {
        self.rooms
            .iter()
            .map(|r| {
                let sid = *r.value();
                let peers = self.peer_count(sid);
                (r.key().clone(), sid, peers)
            })
            .collect()
    }
}

#[derive(Debug)]
pub enum RouteDecision {
    /// Reenviar (broadcast) al conjunto de peers indicado.
    Broadcast(Vec<SessionEndpoint>),
    /// Peer registrado; no hay destinos todavía (primer peer del grupo).
    Registered,
    /// Datagrama descartado.
    Dropped,
}

/// Decisión del árbitro de floor control.
#[derive(Debug)]
pub enum FloorDecision {
    /// Floor concedido al solicitante. Notificar a los demás con FloorTaken.
    Granted {
        /// Peers a los que notificar (todos excepto el solicitante).
        others: Vec<SessionEndpoint>,
    },
    /// Floor denegado (otra persona ya lo tiene).
    Denied,
    /// El floor fue liberado. Notificar a todos los peers.
    Released {
        others: Vec<SessionEndpoint>,
    },
    /// Sesión desconocida o peer no registrado.
    Unknown,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ep(s: &str) -> SessionEndpoint {
        SessionEndpoint::Udp(s.parse().unwrap())
    }

    fn router() -> Router {
        Router::new(100, 50, RelayMetrics::new())
    }

    #[test]
    fn first_packet_registers() {
        let r = router();
        assert!(matches!(r.route(1, ep("127.0.0.1:1000")), RouteDecision::Registered));
        assert_eq!(r.active_sessions(), 1);
        assert_eq!(r.peer_count(1), 1);
    }

    #[test]
    fn second_peer_gets_broadcast_to_first() {
        let r = router();
        r.route(1, ep("127.0.0.1:1000"));
        let d = r.route(1, ep("127.0.0.1:2000"));
        match d {
            RouteDecision::Broadcast(targets) => {
                assert_eq!(targets.len(), 1);
                match &targets[0] {
                    SessionEndpoint::Udp(a) => assert_eq!(a.port(), 1000),
                    _ => panic!("expected UDP endpoint"),
                }
            }
            other => panic!("expected Broadcast, got {other:?}"),
        }
        assert_eq!(r.peer_count(1), 2);
    }

    #[test]
    fn existing_peer_broadcasts_to_all_others() {
        let r = router();
        r.route(1, ep("127.0.0.1:1000"));
        r.route(1, ep("127.0.0.1:2000"));
        r.route(1, ep("127.0.0.1:3000"));
        assert_eq!(r.peer_count(1), 3);

        let d = r.route(1, ep("127.0.0.1:1000"));
        match d {
            RouteDecision::Broadcast(targets) => {
                assert_eq!(targets.len(), 2);
                let ports: Vec<u16> = targets
                    .iter()
                    .map(|t| match t { SessionEndpoint::Udp(a) => a.port(), _ => 0 })
                    .collect();
                assert!(ports.contains(&2000));
                assert!(ports.contains(&3000));
            }
            other => panic!("expected Broadcast, got {other:?}"),
        }
    }

    #[test]
    fn session_full_drops_extra_peer() {
        let r = Router::new(100, 2, RelayMetrics::new());
        r.route(1, ep("127.0.0.1:1000"));
        r.route(1, ep("127.0.0.1:2000"));
        let d = r.route(1, ep("127.0.0.1:3000"));
        assert!(matches!(d, RouteDecision::Dropped));
        assert_eq!(r.peer_count(1), 2);
    }

    #[test]
    fn zero_session_id_dropped() {
        let r = router();
        assert!(matches!(r.route(0, ep("127.0.0.1:1000")), RouteDecision::Dropped));
    }

    #[test]
    fn max_sessions_enforced() {
        let r = Router::new(2, 50, RelayMetrics::new());
        r.route(1, ep("127.0.0.1:1000"));
        r.route(2, ep("127.0.0.1:2000"));
        assert!(matches!(r.route(3, ep("127.0.0.1:3000")), RouteDecision::Dropped));
    }

    #[test]
    fn room_registry_roundtrip() {
        let r = router();
        assert!(r.register_room("ABCD-1234".into(), 42));
        assert_eq!(r.resolve_room("ABCD-1234"), Some(42));
        assert!(!r.register_room("ABCD-1234".into(), 99)); // duplicate
        assert!(r.remove_room("ABCD-1234"));
        assert_eq!(r.resolve_room("ABCD-1234"), None);
    }
}
