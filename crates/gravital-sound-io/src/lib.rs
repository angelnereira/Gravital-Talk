//! Gravital Sound — captura y playback de audio con `cpal`.
//!
//! Proporciona:
//!
//! - [`AudioCapture`] — captura desde un input device (micrófono), entrega
//!   PCM i16 a un canal std::mpsc.
//! - [`AudioPlayback`] — reproduce PCM i16 recibido por un canal.
//! - [`devices`] — enumeración de devices input/output.
//!
//! El diseño desacopla la callback de cpal (que corre en un thread de
//! tiempo real) del pipeline Gravital Sound usando canales sin bloqueo.

#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(missing_debug_implementations)]

pub mod capture;
pub mod devices;
pub mod error;
pub mod playback;

pub use capture::AudioCapture;
pub use devices::{list_input_devices, list_output_devices, DeviceInfo};
pub use error::IoError;
pub use playback::AudioPlayback;

pub type Result<T> = core::result::Result<T, IoError>;

/// Configuración compartida de captura/playback.
#[derive(Debug, Clone, Copy)]
pub struct StreamConfig {
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_duration_ms: u8,
}

impl Default for StreamConfig {
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            channels: 1,
            frame_duration_ms: 10,
        }
    }
}

impl StreamConfig {
    pub fn samples_per_frame(&self) -> usize {
        (self.sample_rate as usize * self.frame_duration_ms as usize) / 1000
            * self.channels as usize
    }
}
