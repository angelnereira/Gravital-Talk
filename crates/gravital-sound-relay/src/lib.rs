//! Relay server productivo de Gravital Sound.
//!
//! Acepta paquetes UDP **y** conexiones WebSocket en el mismo proceso, y
//! reenvía datagramas entre peers que comparten un `session_id`. Expone
//! métricas Prometheus en `/metrics` y un health check en `/healthz`.
//!
//! Diseño:
//! - El [`Router`] mantiene un `DashMap<session_id, RouteEntry>` con la(s)
//!   dirección(es) UDP y/o conexión(es) WS de los peers de cada sesión.
//! - El primer datagrama válido de un session_id desconocido registra al
//!   peer como "endpoint A". El segundo registra "endpoint B" y a partir
//!   de ahí los datagramas se reenvían en bidireccional.
//! - El relay nunca decodifica el payload — sólo lee el header para extraer
//!   `session_id` y reenvía bytes raw.
//!
//! Esto mantiene latencia mínima y permite que el cifrado opcional (Track C
//! futuro) sea opaco al relay.

#![forbid(unsafe_op_in_unsafe_fn)]

pub mod config;
pub mod metrics;
pub mod observability;
pub mod router;
pub mod udp;
pub mod ws;

pub use config::RelayConfig;
pub use metrics::RelayMetrics;
pub use router::{Router, SessionEndpoint};
