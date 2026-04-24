//! Errores del crate IO.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum IoError {
    #[error("no default audio host available")]
    NoHost,

    #[error("no matching {kind} device")]
    NoDevice { kind: &'static str },

    #[error("device does not support requested config: {0}")]
    UnsupportedConfig(String),

    #[error("cpal error: {0}")]
    Cpal(String),

    #[error("channel closed")]
    ChannelClosed,
}

impl From<cpal::DevicesError> for IoError {
    fn from(e: cpal::DevicesError) -> Self {
        Self::Cpal(format!("devices: {e}"))
    }
}

impl From<cpal::DeviceNameError> for IoError {
    fn from(e: cpal::DeviceNameError) -> Self {
        Self::Cpal(format!("device name: {e}"))
    }
}

impl From<cpal::SupportedStreamConfigsError> for IoError {
    fn from(e: cpal::SupportedStreamConfigsError) -> Self {
        Self::Cpal(format!("supported configs: {e}"))
    }
}

impl From<cpal::DefaultStreamConfigError> for IoError {
    fn from(e: cpal::DefaultStreamConfigError) -> Self {
        Self::Cpal(format!("default config: {e}"))
    }
}

impl From<cpal::BuildStreamError> for IoError {
    fn from(e: cpal::BuildStreamError) -> Self {
        Self::Cpal(format!("build stream: {e}"))
    }
}

impl From<cpal::PlayStreamError> for IoError {
    fn from(e: cpal::PlayStreamError) -> Self {
        Self::Cpal(format!("play stream: {e}"))
    }
}
