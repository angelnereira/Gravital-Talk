//! `CodecSession` — wrapper sobre `Session` que aplica un codec.
//!
//! El `gravital-sound-transport::Session` transporta bytes. Este módulo
//! añade la capa de codec: el caller envía `&[i16]` samples y recibe frames
//! decodificados.
//!
//! Uso:
//!
//! ```no_run
//! use std::sync::Arc;
//! use gravital_sound::{CodecId, CodecSession, Config, SessionRole, UdpConfig, UdpTransport};
//!
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! let transport = Arc::new(
//!     UdpTransport::bind(UdpConfig::default()).await?,
//! );
//! let session = CodecSession::new(transport, Config::default(), CodecId::Pcm)?;
//! session.handshake(SessionRole::Client, "127.0.0.1:9000".parse()?).await?;
//! let silence = vec![0i16; 480];
//! session.send_samples(&silence).await?;
//! # Ok(()) }
//! ```

use std::net::SocketAddr;
use std::sync::Arc;

use gravital_sound_codec::{build_pair, CodecError, CodecId, Decoder, Encoder};
use gravital_sound_transport::{
    jitter_buffer::Frame, Config, Session, SessionRole, Transport, TransportError,
};
use tokio::sync::Mutex;

/// Error combinado de CodecSession.
#[derive(Debug, thiserror::Error)]
pub enum CodecSessionError {
    #[error(transparent)]
    Transport(#[from] TransportError),
    #[error(transparent)]
    Codec(#[from] CodecError),
}

pub struct CodecSession {
    inner: Arc<Session>,
    codec_id: CodecId,
    encoder: Mutex<Box<dyn Encoder>>,
    decoder: Mutex<Box<dyn Decoder>>,
    channels: u8,
    frame_samples: usize,
    mtu: usize,
}

impl core::fmt::Debug for CodecSession {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CodecSession")
            .field("codec", &self.codec_id)
            .field("channels", &self.channels)
            .field("frame_samples", &self.frame_samples)
            .finish()
    }
}

impl CodecSession {
    /// Crea un `CodecSession` adjuntando un codec a una sesión ya construida.
    pub fn new(
        transport: Arc<dyn Transport>,
        config: Config,
        codec: CodecId,
    ) -> Result<Self, CodecSessionError> {
        let (encoder, decoder) = build_pair(
            codec,
            config.sample_rate,
            config.channels,
            config.frame_duration_ms,
        )?;
        let frame_samples = encoder.frame_samples();
        let channels = config.channels;
        let mtu = config.mtu;
        Ok(Self {
            inner: Arc::new(Session::new(transport, config)),
            codec_id: codec,
            encoder: Mutex::new(encoder),
            decoder: Mutex::new(decoder),
            channels,
            frame_samples,
            mtu,
        })
    }

    #[must_use]
    pub fn codec(&self) -> CodecId {
        self.codec_id
    }

    #[must_use]
    pub fn session(&self) -> Arc<Session> {
        self.inner.clone()
    }

    pub async fn handshake(
        &self,
        role: SessionRole,
        peer: SocketAddr,
    ) -> Result<(), CodecSessionError> {
        self.inner.handshake(role, peer).await?;
        Ok(())
    }

    /// Envía `samples` (interleaved si `channels > 1`). `samples.len()` debe
    /// ser `frame_samples * channels`.
    pub async fn send_samples(&self, samples: &[i16]) -> Result<(), CodecSessionError> {
        let mut out = vec![0u8; self.mtu.max(1500)];
        let n = {
            let mut enc = self.encoder.lock().await;
            enc.encode(samples, &mut out)?
        };
        self.inner.send_audio(&out[..n]).await?;
        Ok(())
    }

    /// Recibe el próximo frame y lo decodifica a samples PCM i16.
    /// Devuelve el vector de samples (length = samples_per_channel * channels).
    pub async fn recv_samples(&self) -> Result<Vec<i16>, CodecSessionError> {
        let frame: Frame = self.inner.recv_audio().await?;
        let expected = self.frame_samples * self.channels as usize;
        let mut pcm = vec![0i16; expected.max(5760)]; // margin para PLC
        let mut dec = self.decoder.lock().await;
        let produced = dec.decode(&frame.payload, &mut pcm)?;
        let total = produced * self.channels as usize;
        pcm.truncate(total);
        Ok(pcm)
    }

    pub async fn close(&self) -> Result<(), CodecSessionError> {
        self.inner.close().await?;
        Ok(())
    }
}
