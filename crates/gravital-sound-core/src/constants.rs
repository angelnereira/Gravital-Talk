//! Constantes del protocolo.

/// Magic bytes `"GS"` en ASCII, big-endian.
pub const MAGIC_BYTES: [u8; 2] = [b'G', b'S'];

/// Versión del protocolo implementada.
pub const PROTOCOL_VERSION: u8 = 0x01;

/// Tamaño del header en bytes.
pub const HEADER_SIZE: usize = 24;

/// MTU efectivo por default (deja espacio para IP 40 + UDP 8 sobre 1280 PMTU IPv6).
pub const DEFAULT_MTU: usize = 1200;

/// Tamaño máximo del payload por default.
pub const MAX_PAYLOAD_SIZE: usize = DEFAULT_MTU - HEADER_SIZE;

/// Número máximo de fragmentos por frame grande.
pub const MAX_FRAGMENTS: u16 = 16;

/// Tamaño del sub-header de fragmentación al inicio del payload.
pub const FRAGMENT_SUBHEADER_SIZE: usize = 4;

/// Sample rate por default (Hz).
pub const DEFAULT_SAMPLE_RATE: u32 = 48_000;

/// Duración por default de un frame de audio (ms).
pub const DEFAULT_FRAME_DURATION_MS: u8 = 20;

/// Duración del jitter buffer por default (ms).
pub const DEFAULT_JITTER_BUFFER_MS: u16 = 40;

/// Bitrate por default para Opus cuando esté disponible (bps).
pub const DEFAULT_MAX_BITRATE: u32 = 64_000;

/// Intervalo entre heartbeats (ms) cuando no hay otro tráfico.
pub const HEARTBEAT_INTERVAL_MS: u64 = 1_000;

/// Tiempo sin tráfico antes de considerar al peer muerto (ms).
pub const HEARTBEAT_TIMEOUT_MS: u64 = 10_000;

/// Tiempo máximo del handshake antes de abort (ms).
pub const HANDSHAKE_TIMEOUT_MS: u64 = 10_000;

/// Backoff inicial del cliente en reintento de handshake (ms).
pub const HANDSHAKE_RETRY_BASE_MS: u64 = 200;

/// Grace period para responder a CLOSE antes de forzar Closed (ms).
pub const CLOSE_GRACE_MS: u64 = 500;

/// Tamaño de la ventana de bitmap de pérdida.
pub const LOSS_WINDOW_SIZE: u32 = 64;

/// Offsets de campos del header (útil para bindings externos).
pub mod offsets {
    pub const MAGIC: usize = 0;
    pub const VERSION: usize = 2;
    pub const FLAGS: usize = 3;
    pub const MSG_TYPE: usize = 4;
    pub const RESERVED: usize = 5;
    pub const SESSION_ID: usize = 8;
    pub const SEQUENCE: usize = 12;
    pub const TIMESTAMP: usize = 16;
    pub const HEADER_END: usize = 24;
}

/// Offset desde el **fin** del paquete para `payload_len` (u16 BE).
pub const PAYLOAD_LEN_TAIL_OFFSET: usize = 4;

/// Offset desde el fin del paquete para `checksum` (u16 BE).
pub const CHECKSUM_TAIL_OFFSET: usize = 2;
