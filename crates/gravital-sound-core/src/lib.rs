//! Gravital Sound — core del protocolo.
//!
//! Este crate contiene toda la lógica de framing, máquina de estados y
//! primitivas de integridad. Es `#![no_std]` por default con `alloc`
//! disponible; activa la feature `std` para integrar con la stdlib.
//!
//! El formato binario está documentado en `docs/packet-format.md` y la
//! especificación completa en `docs/protocol-spec.md`.

#![cfg_attr(not(feature = "std"), no_std)]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(missing_debug_implementations)]

#[cfg(feature = "alloc")]
extern crate alloc;

pub mod checksum;
pub mod constants;
pub mod error;
pub mod fragment;
pub mod header;
pub mod message;
pub mod packet;
pub mod session;

pub use checksum::crc16_ccitt_false;
pub use constants::{
    DEFAULT_MTU, HEADER_SIZE, MAGIC_BYTES, MAX_FRAGMENTS, MAX_PAYLOAD_SIZE, PROTOCOL_VERSION,
};
pub use error::Error;
pub use fragment::{FragmentHeader, FragmentReassembler};
pub use header::{Flags, PacketHeader};
pub use message::{ErrorCode, HandshakeAccept, HandshakeConfirm, HandshakeInit, MessageType};
pub use packet::{Packet, PacketView};
pub use session::{SessionId, SessionState, SessionStateMachine, StateTransitionError};

/// Resultado estándar del crate.
pub type Result<T> = core::result::Result<T, Error>;

/// Versión del crate, exportada para que la FFI la devuelva.
pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");
