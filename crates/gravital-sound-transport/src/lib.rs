//! Gravital Sound — capa de transporte.
//!
//! Contiene:
//! - `Transport` trait (async) sobre el que se construye el transporte real.
//! - `UdpTransport` con tuning de socket agresivo para baja latencia.
//! - `JitterBuffer` lock-free.
//! - `SessionManager` que orquesta el handshake 3-way y el ciclo de vida.

#![forbid(unsafe_op_in_unsafe_fn)]

pub mod error;
pub mod jitter_buffer;
pub mod session;
pub mod traits;
pub mod udp;

pub use error::TransportError;
pub use jitter_buffer::JitterBuffer;
pub use session::{Config, Session, SessionRole};
pub use traits::{LatencyClass, Transport};
pub use udp::UdpTransport;

pub type Result<T> = core::result::Result<T, TransportError>;
