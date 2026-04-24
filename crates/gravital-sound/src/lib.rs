//! # Gravital Sound
//!
//! Facade crate. Re-exporta los tipos públicos más usados de los crates
//! `gravital-sound-core`, `gravital-sound-metrics` y `gravital-sound-transport`
//! para facilitar el uso desde aplicaciones.
//!
//! ```no_run
//! use gravital_sound::{Config, Session, SessionRole, UdpTransport, UdpConfig};
//! use std::sync::Arc;
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let transport = Arc::new(
//!     UdpTransport::bind(UdpConfig {
//!         bind_addr: "0.0.0.0:0".parse()?,
//!         ..Default::default()
//!     })
//!     .await?,
//! );
//! let session = Session::new(transport, Config::default());
//! session.handshake(SessionRole::Client, "127.0.0.1:9000".parse()?).await?;
//! session.send_audio(&[0u8; 960]).await?;
//! # Ok(()) }
//! ```

#![forbid(unsafe_op_in_unsafe_fn)]

pub mod codec_session;
pub use codec_session::CodecSession;

#[cfg(feature = "opus")]
pub use gravital_sound_codec::OpusCodec;
pub use gravital_sound_codec::{
    build_pair as build_codec_pair, CodecError, CodecId, Decoder, Encoder, PcmCodec,
};

pub use gravital_sound_core::{
    checksum, constants,
    error::Error as CoreError,
    fragment::{FragmentHeader, FragmentReassembler},
    header::{Flags, PacketHeader},
    message::{ErrorCode, HandshakeAccept, HandshakeConfirm, HandshakeInit, MessageType},
    packet::{PacketBuilder, PacketView},
    session::{SessionEvent, SessionId, SessionState, SessionStateMachine, StateTransitionError},
    PROTOCOL_VERSION,
};

pub use gravital_sound_metrics::{
    estimate_mos, Counters, JitterEstimator, LossTracker, Metrics, MetricsSnapshot, RttEstimator,
};

pub use gravital_sound_transport::{
    jitter_buffer::{Frame, JitterBuffer},
    udp::{UdpConfig, UdpTransport, DEFAULT_SOCKET_BUFFER, DSCP_EF},
    Config, LatencyClass, Session, SessionRole, Transport, TransportError,
};

/// Version del crate facade.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
