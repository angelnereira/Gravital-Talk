//! Orquestación de sesión: handshake 3-way, heartbeat, envío/recepción.
//!
//! Una `Session` envuelve un `Transport` y el `SessionStateMachine` del
//! core, llevando el ciclo de vida y las métricas.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bytes::{Bytes, BytesMut};
use gravital_sound_core::constants::{
    DEFAULT_FRAME_DURATION_MS, DEFAULT_JITTER_BUFFER_MS, DEFAULT_MAX_BITRATE, DEFAULT_MTU,
    DEFAULT_SAMPLE_RATE, HANDSHAKE_RETRY_BASE_MS, HANDSHAKE_TIMEOUT_MS, HEARTBEAT_INTERVAL_MS,
    HEARTBEAT_TIMEOUT_MS,
};
use gravital_sound_core::header::{Flags, PacketHeader};
use gravital_sound_core::message::{HandshakeAccept, HandshakeConfirm, HandshakeInit, MessageType};
use gravital_sound_core::packet::{PacketBuilder, PacketView};
use gravital_sound_core::session::{SessionEvent, SessionState, SessionStateMachine};
use gravital_sound_metrics::Metrics;
use tokio::sync::Mutex;
use tokio::time::{timeout, Instant};

use crate::error::TransportError;
use crate::jitter_buffer::{Frame, JitterBuffer};
use crate::traits::Transport;

/// Rol de la sesión en el handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRole {
    /// Inicia el handshake (manda `HANDSHAKE_INIT`).
    Client,
    /// Acepta el handshake (manda `HANDSHAKE_ACCEPT`).
    Server,
}

/// Parámetros negociables de sesión.
#[derive(Debug, Clone)]
pub struct Config {
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_duration_ms: u8,
    pub max_bitrate: u32,
    /// Codec preferido (1 = PCM, 2 = Opus reservado).
    pub codec_preferred: u8,
    /// Codecs aceptables del lado server (en orden de preferencia local).
    /// El server elige el `codec_preferred` del cliente si está en esta lista;
    /// si no, hace fallback al primer codec local. El cliente valida que el
    /// codec aceptado por el server esté en su propia lista soportada.
    pub supported_codecs: Vec<u8>,
    /// Flags de capacidad (bitfield definido por la aplicación).
    pub capability_flags: u32,
    /// Profundidad del jitter buffer en ms.
    pub jitter_buffer_ms: u16,
    /// MTU efectivo en bytes.
    pub mtu: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            sample_rate: DEFAULT_SAMPLE_RATE,
            channels: 1,
            frame_duration_ms: DEFAULT_FRAME_DURATION_MS,
            max_bitrate: DEFAULT_MAX_BITRATE,
            codec_preferred: 0x01,
            supported_codecs: vec![0x01, 0x02],
            capability_flags: 0,
            jitter_buffer_ms: DEFAULT_JITTER_BUFFER_MS,
            mtu: DEFAULT_MTU,
        }
    }
}

/// Una sesión activa.
pub struct Session {
    transport: Arc<dyn Transport>,
    state: Mutex<SessionStateMachine>,
    metrics: Arc<Metrics>,
    jitter: Arc<JitterBuffer>,
    config: Config,
    peer: Mutex<Option<SocketAddr>>,
    session_id: AtomicU32,
    tx_sequence: AtomicU32,
    last_rx: AtomicU64,
    /// Codec acordado tras el handshake (0 antes de negociar).
    negotiated_codec: AtomicU8,
    epoch: Instant,
}

impl core::fmt::Debug for Session {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Session")
            .field("session_id", &self.session_id.load(Ordering::Relaxed))
            .field("tx_sequence", &self.tx_sequence.load(Ordering::Relaxed))
            .finish()
    }
}

impl Session {
    /// Construye una sesión con transporte ya conectado.
    pub fn new(transport: Arc<dyn Transport>, config: Config) -> Self {
        let jitter_depth = jitter_slots(config.jitter_buffer_ms, config.frame_duration_ms);
        Self {
            transport,
            state: Mutex::new(SessionStateMachine::new()),
            metrics: Arc::new(Metrics::new()),
            jitter: Arc::new(JitterBuffer::new(jitter_depth)),
            config,
            peer: Mutex::new(None),
            session_id: AtomicU32::new(0),
            tx_sequence: AtomicU32::new(0),
            last_rx: AtomicU64::new(0),
            negotiated_codec: AtomicU8::new(0),
            epoch: Instant::now(),
        }
    }

    /// Codec acordado tras el handshake. Devuelve `0` si aún no se completó.
    #[must_use]
    pub fn negotiated_codec(&self) -> u8 {
        self.negotiated_codec.load(Ordering::Acquire)
    }

    /// Configuración inmutable de esta sesión.
    #[must_use]
    pub fn config(&self) -> &Config {
        &self.config
    }

    #[must_use]
    pub fn metrics(&self) -> Arc<Metrics> {
        self.metrics.clone()
    }

    #[must_use]
    pub fn jitter_buffer(&self) -> Arc<JitterBuffer> {
        self.jitter.clone()
    }

    /// Estado actual (snapshot).
    pub async fn state(&self) -> SessionState {
        self.state.lock().await.state()
    }

    /// ID de sesión negociado (0 si aún no hay handshake).
    #[must_use]
    pub fn session_id(&self) -> u32 {
        self.session_id.load(Ordering::Acquire)
    }

    /// Ejecuta el handshake 3-way como cliente o servidor.
    pub async fn handshake(
        &self,
        role: SessionRole,
        peer: SocketAddr,
    ) -> Result<(), TransportError> {
        *self.peer.lock().await = Some(peer);

        {
            let event = match role {
                SessionRole::Client => SessionEvent::StartConnect,
                SessionRole::Server => SessionEvent::StartAccept,
            };
            self.state
                .lock()
                .await
                .transition(event)
                .map_err(|_| TransportError::InvalidState("cannot start handshake"))?;
        }

        let deadline = Duration::from_millis(HANDSHAKE_TIMEOUT_MS);
        let result = match role {
            SessionRole::Client => timeout(deadline, self.handshake_client(peer)).await,
            SessionRole::Server => timeout(deadline, self.handshake_server(peer)).await,
        };

        match result {
            Ok(Ok(())) => {
                self.state
                    .lock()
                    .await
                    .transition(SessionEvent::HandshakeOk)
                    .map_err(|_| TransportError::InvalidState("handshake_ok"))?;
                Ok(())
            }
            Ok(Err(e)) => {
                let _ = self
                    .state
                    .lock()
                    .await
                    .transition(SessionEvent::HandshakeTimeout);
                Err(e)
            }
            Err(_) => {
                let _ = self
                    .state
                    .lock()
                    .await
                    .transition(SessionEvent::HandshakeTimeout);
                Err(TransportError::Timeout)
            }
        }
    }

    async fn handshake_client(&self, peer: SocketAddr) -> Result<(), TransportError> {
        let nonce: u32 = rand_u32();
        let init = HandshakeInit {
            protocol_version: 1,
            codec_preferred: self.config.codec_preferred,
            sample_rate: self.config.sample_rate,
            channels: self.config.channels,
            frame_duration_ms: self.config.frame_duration_ms,
            max_bitrate: self.config.max_bitrate,
            capability_flags: self.config.capability_flags,
            nonce,
        };

        let mut payload = [0u8; HandshakeInit::SIZE];
        init.encode(&mut payload)
            .map_err(TransportError::Protocol)?;

        // Reintento con backoff exponencial hasta el timeout del caller.
        let mut attempt: u32 = 0;
        loop {
            self.send_control(MessageType::HandshakeInit, 0, &payload, peer)
                .await?;

            let mut buf = vec![0u8; self.config.mtu];
            let backoff = Duration::from_millis(HANDSHAKE_RETRY_BASE_MS << attempt.min(4));
            let res = timeout(backoff, self.transport.recv(&mut buf)).await;
            if let Ok(Ok((n, _))) = res {
                let view = PacketView::decode(&buf[..n]).map_err(TransportError::Protocol)?;
                if view.header().msg_type == MessageType::HandshakeAccept.code() {
                    let accept = HandshakeAccept::decode(view.payload())
                        .map_err(TransportError::Protocol)?;
                    if accept.nonce != nonce {
                        return Err(TransportError::Handshake("nonce mismatch"));
                    }
                    if !self
                        .config
                        .supported_codecs
                        .contains(&accept.codec_accepted)
                    {
                        return Err(TransportError::Handshake(
                            "server selected unsupported codec",
                        ));
                    }
                    self.negotiated_codec
                        .store(accept.codec_accepted, Ordering::Release);
                    self.session_id.store(accept.session_id, Ordering::Release);
                    let confirm = HandshakeConfirm {
                        session_id: accept.session_id,
                    };
                    let mut pc = [0u8; HandshakeConfirm::SIZE];
                    confirm.encode(&mut pc).map_err(TransportError::Protocol)?;
                    self.send_control(MessageType::HandshakeConfirm, accept.session_id, &pc, peer)
                        .await?;
                    return Ok(());
                }
            }
            attempt = attempt.saturating_add(1);
            if attempt > 6 {
                return Err(TransportError::Handshake("client retries exhausted"));
            }
        }
    }

    async fn handshake_server(&self, peer: SocketAddr) -> Result<(), TransportError> {
        let mut buf = vec![0u8; self.config.mtu];
        // Descarta datagramas de otros peers (o malformados) hasta encontrar
        // un HANDSHAKE_INIT válido del peer esperado. Evita que un tercer
        // host interrumpa el handshake.
        let init: HandshakeInit = loop {
            let (n, from) = self.transport.recv(&mut buf).await?;
            if from != peer {
                tracing::debug!(?from, expected = ?peer, "dropping datagram from wrong peer");
                continue;
            }
            let view = match PacketView::decode(&buf[..n]) {
                Ok(v) => v,
                Err(e) => {
                    tracing::debug!(?e, "dropping malformed packet during handshake");
                    continue;
                }
            };
            if view.header().msg_type != MessageType::HandshakeInit.code() {
                tracing::debug!(
                    msg_type = view.header().msg_type,
                    "dropping non-INIT packet during handshake"
                );
                continue;
            }
            match HandshakeInit::decode(view.payload()) {
                Ok(i) => break i,
                Err(e) => return Err(TransportError::Protocol(e)),
            }
        };
        if init.protocol_version != 1 {
            return Err(TransportError::Handshake("protocol version mismatch"));
        }

        let session_id = rand_u32();
        self.session_id.store(session_id, Ordering::Release);

        // Codec negotiation: prefer client's choice if locally supported,
        // else fall back to first locally-supported codec; reject if list empty.
        let chosen_codec = if self.config.supported_codecs.contains(&init.codec_preferred) {
            init.codec_preferred
        } else {
            *self
                .config
                .supported_codecs
                .first()
                .ok_or(TransportError::Handshake("no supported codecs configured"))?
        };
        self.negotiated_codec.store(chosen_codec, Ordering::Release);

        let accept = HandshakeAccept {
            protocol_version: 1,
            codec_accepted: chosen_codec,
            sample_rate: init.sample_rate,
            channels: init.channels,
            frame_duration_ms: init.frame_duration_ms,
            max_bitrate: init.max_bitrate.min(self.config.max_bitrate),
            capability_flags: init.capability_flags & self.config.capability_flags,
            nonce: init.nonce,
            session_id,
        };

        let mut payload = [0u8; HandshakeAccept::SIZE];
        accept
            .encode(&mut payload)
            .map_err(TransportError::Protocol)?;
        self.send_control(MessageType::HandshakeAccept, session_id, &payload, peer)
            .await?;

        // Espera CONFIRM, ignorando tráfico de otros peers.
        let confirm: HandshakeConfirm = loop {
            let (n, from) = self.transport.recv(&mut buf).await?;
            if from != peer {
                tracing::debug!(
                    ?from,
                    "dropping datagram from wrong peer during CONFIRM wait"
                );
                continue;
            }
            let view = match PacketView::decode(&buf[..n]) {
                Ok(v) => v,
                Err(e) => {
                    tracing::debug!(?e, "dropping malformed packet during CONFIRM wait");
                    continue;
                }
            };
            if view.header().msg_type != MessageType::HandshakeConfirm.code() {
                continue;
            }
            match HandshakeConfirm::decode(view.payload()) {
                Ok(c) => break c,
                Err(e) => return Err(TransportError::Protocol(e)),
            }
        };
        if confirm.session_id != session_id {
            return Err(TransportError::Handshake("session_id mismatch"));
        }
        Ok(())
    }

    /// Envía un frame de audio. Requiere `Active`.
    pub async fn send_audio(&self, payload: &[u8]) -> Result<(), TransportError> {
        {
            let st = self.state.lock().await.state();
            if st != SessionState::Active {
                return Err(TransportError::InvalidState("not active"));
            }
        }
        let peer = self
            .peer
            .lock()
            .await
            .ok_or(TransportError::InvalidState("no peer"))?;
        let seq = self.tx_sequence.fetch_add(1, Ordering::Relaxed);
        let ts = self.micros_since_epoch();
        let header = PacketHeader {
            version: 1,
            flags: Flags::empty(),
            msg_type: MessageType::AudioFrame.code(),
            session_id: self.session_id.load(Ordering::Acquire),
            sequence: seq,
            timestamp: ts,
        };
        let mut buf = BytesMut::with_capacity(self.config.mtu);
        buf.resize(self.config.mtu, 0);
        let n = PacketBuilder::new(header, payload)
            .encode(&mut buf)
            .map_err(TransportError::Protocol)?;
        let sent = self.transport.send_to(&buf[..n], peer).await?;
        self.metrics.counters.record_sent(sent as u64);
        Ok(())
    }

    /// Recibe el próximo frame de audio ya desjitterizado.
    /// Bloquea hasta que haya al menos un frame disponible.
    pub async fn recv_audio(&self) -> Result<Frame, TransportError> {
        loop {
            if let Some(frame) = self.jitter.pop() {
                return Ok(frame);
            }
            self.poll_once().await?;
        }
    }

    /// Procesa un único datagrama entrante (util para event loops custom).
    pub async fn poll_once(&self) -> Result<(), TransportError> {
        let mut buf = vec![0u8; self.config.mtu];
        let recv_res = timeout(
            Duration::from_millis(HEARTBEAT_INTERVAL_MS),
            self.transport.recv(&mut buf),
        )
        .await;

        let (n, _from) = match recv_res {
            Ok(r) => r?,
            Err(_) => {
                // Timeout: emite heartbeat si estamos activos.
                let st = self.state.lock().await.state();
                if matches!(st, SessionState::Active | SessionState::Paused) {
                    self.send_heartbeat().await?;
                    self.check_liveness().await?;
                }
                return Ok(());
            }
        };

        self.metrics.counters.record_received(n as u64);
        let view = match PacketView::decode(&buf[..n]) {
            Ok(v) => v,
            Err(e) => {
                self.metrics.counters.record_integrity_error();
                tracing::debug!(?e, "dropping malformed packet");
                return Ok(());
            }
        };
        self.last_rx
            .store(self.micros_since_epoch(), Ordering::Release);

        let mt = view.header().msg_type;
        match MessageType::from_code(mt) {
            Ok(MessageType::AudioFrame) => {
                let frame = Frame {
                    sequence: view.header().sequence,
                    timestamp: view.header().timestamp,
                    payload: Bytes::copy_from_slice(view.payload()),
                };
                self.metrics.loss.record(frame.sequence);
                self.metrics
                    .jitter
                    .record(frame.timestamp, self.micros_since_epoch());
                if !self.jitter.push(frame) {
                    tracing::trace!(seq = view.header().sequence, "jitter buffer rejected frame");
                }
            }
            Ok(MessageType::Heartbeat) => {
                let peer = self.peer.lock().await;
                if let Some(p) = *peer {
                    self.send_control(MessageType::HeartbeatAck, self.session_id(), &[], p)
                        .await?;
                }
            }
            Ok(MessageType::HeartbeatAck) => {
                // La RTT se calcula fuera de banda (ver bench). Aquí sólo marcamos liveness.
            }
            Ok(MessageType::Close) => {
                self.state
                    .lock()
                    .await
                    .transition(SessionEvent::PeerClosed)
                    .ok();
                return Err(TransportError::PeerClosed("remote close"));
            }
            Ok(_) => {}
            Err(_) => {
                self.metrics.counters.record_integrity_error();
            }
        }
        Ok(())
    }

    /// Envía CLOSE y transiciona a `Closing` → `Closed`.
    pub async fn close(&self) -> Result<(), TransportError> {
        let peer = *self.peer.lock().await;
        if let Some(p) = peer {
            let _ = self
                .send_control(MessageType::Close, self.session_id(), &[], p)
                .await;
        }
        let mut sm = self.state.lock().await;
        let _ = sm.transition(SessionEvent::Close);
        let _ = sm.transition(SessionEvent::Close);
        Ok(())
    }

    async fn send_heartbeat(&self) -> Result<(), TransportError> {
        let peer = match *self.peer.lock().await {
            Some(p) => p,
            None => return Ok(()),
        };
        self.send_control(MessageType::Heartbeat, self.session_id(), &[], peer)
            .await
    }

    async fn check_liveness(&self) -> Result<(), TransportError> {
        let now = self.micros_since_epoch();
        let last = self.last_rx.load(Ordering::Acquire);
        if last != 0 && now.saturating_sub(last) > HEARTBEAT_TIMEOUT_MS * 1_000 {
            self.state
                .lock()
                .await
                .transition(SessionEvent::PeerTimeout)
                .ok();
            return Err(TransportError::PeerClosed("heartbeat timeout"));
        }
        Ok(())
    }

    async fn send_control(
        &self,
        msg: MessageType,
        session_id: u32,
        payload: &[u8],
        peer: SocketAddr,
    ) -> Result<(), TransportError> {
        let seq = self.tx_sequence.fetch_add(1, Ordering::Relaxed);
        let ts = self.micros_since_epoch();
        let header = PacketHeader::new(msg.code(), session_id, seq, ts);
        let mut buf = BytesMut::with_capacity(self.config.mtu);
        buf.resize(self.config.mtu, 0);
        let n = PacketBuilder::new(header, payload)
            .encode(&mut buf)
            .map_err(TransportError::Protocol)?;
        let sent = self.transport.send_to(&buf[..n], peer).await?;
        self.metrics.counters.record_sent(sent as u64);
        Ok(())
    }

    #[inline]
    fn micros_since_epoch(&self) -> u64 {
        // Referencia monotónica desde el epoch de la sesión; el timestamp
        // del protocolo es relativo, no wall-clock.
        self.epoch.elapsed().as_micros() as u64
    }
}

#[inline]
fn jitter_slots(buffer_ms: u16, frame_ms: u8) -> u32 {
    let frames = (buffer_ms / frame_ms.max(1) as u16).max(1) as u32;
    // Redondea hacia la próxima potencia de 2 y asegura mínimo 16.
    frames.next_power_of_two().max(16)
}

/// Fuente rápida de aleatoriedad para nonces y session_id.
/// No es criptográficamente segura — no la usamos para claves.
fn rand_u32() -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let pid = std::process::id();
    let mixed = nanos ^ pid.rotate_left(13);
    // Mezcla xorshift para decorrelacionar.
    let mut x = mixed;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jitter_slots_power_of_two() {
        assert_eq!(jitter_slots(40, 20), 16);
        assert_eq!(jitter_slots(100, 20), 16);
        assert_eq!(jitter_slots(1000, 20), 64);
    }

    #[test]
    fn rand_u32_varies() {
        let a = rand_u32();
        std::thread::sleep(Duration::from_millis(1));
        let b = rand_u32();
        assert_ne!(a, b);
    }
}
