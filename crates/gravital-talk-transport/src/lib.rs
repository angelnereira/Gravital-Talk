//! Gravital Talk — capa de transporte.
//!
//! Contiene:
//! - `Transport` trait (async) sobre el que se construye el transporte real.
//! - `UdpTransport` con tuning de socket agresivo para baja latencia.
//! - `JitterBuffer` lock-free.
//! - `SessionManager` que orquesta el handshake 3-way y el ciclo de vida.

#![forbid(unsafe_op_in_unsafe_fn)]

pub mod congestion;
pub mod discovery;
pub mod error;
pub mod fec;
pub mod jitter_buffer;
pub mod session;
pub mod tone;
pub mod traits;
pub mod udp;

pub use congestion::CongestionController;
pub use discovery::{announce_lan, discover_lan, PeerInfo, DISCOVERY_PORT};
pub use error::TransportError;
pub use fec::{FecDecoder, FecEncoder, FecParity};
pub use jitter_buffer::JitterBuffer;
pub use session::{Config, Session, SessionRole};
pub use tone::{generate_pcm_tone, pcm_to_bytes, ptt_press_tone, ptt_release_tone};
pub use traits::{LatencyClass, Transport};
pub use udp::UdpTransport;

pub type Result<T> = core::result::Result<T, TransportError>;
