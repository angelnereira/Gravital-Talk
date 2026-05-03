//! Orquestación de sesión: handshake criptográfico 4-way, heartbeat, envío/recepción AEAD.
//!
//! ## Flujo de handshake seguro
//!
//! ```text
//! Cliente                                  Servidor
//!   │── ClientHello (0x01) ───────────────►│  X25519 pubkey + nonce
//!   │◄── ServerHello (0x02) ───────────────│  X25519 pubkey + nonce + session_id
//!   │    [ECDH → shared_secret]
//!   │    [HKDF → encrypt_key, decrypt_key]
//!   │── KeyExchange  (0x04) ───────────────►│  auth_tag cliente
//!   │◄── SessionConfirm (0x03) ────────────│  auth_tag servidor
//! ```
//!
//! Tras el handshake, todo el audio se cifra con ChaCha20-Poly1305.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::{Bytes, BytesMut};
use gravital_sound_core::constants::{
    CONGESTION_MIN_BITRATE, DEFAULT_FRAME_DURATION_MS, DEFAULT_JITTER_BUFFER_MS,
    DEFAULT_MAX_BITRATE, DEFAULT_MTU, DEFAULT_SAMPLE_RATE, HANDSHAKE_RETRY_BASE_MS,
    HANDSHAKE_TIMEOUT_MS, HEARTBEAT_INTERVAL_MS, HEARTBEAT_TIMEOUT_MS, HEADER_SIZE,
    PROTOCOL_VERSION_MAX, PROTOCOL_VERSION_MIN,
};
use gravital_sound_core::crypto::{decrypt_in_place, encrypt_in_place, make_nonce, SessionKey, TAG_SIZE};
use gravital_sound_core::header::{Flags, PacketHeader};
use gravital_sound_core::message::{
    ClientHello, ControlBitrateMsg, KeyExchangeMsg, MessageType, ServerHello, SessionConfirm,
};
use gravital_sound_core::packet::{PacketBuilder, PacketView};
use gravital_sound_core::session::{SessionEvent, SessionState, SessionStateMachine};
use gravital_sound_metrics::Metrics;
use hkdf::Hkdf;
use sha2::Sha256;
use tokio::sync::Mutex;
use tokio::time::{timeout, Instant};
use x25519_dalek::{EphemeralSecret, PublicKey};

use crate::congestion::CongestionController;
use crate::error::TransportError;
use crate::fec::{FecDecoder, FecEncoder, FecParity};
use crate::jitter_buffer::{Frame, JitterBuffer};
use crate::traits::Transport;

/// Rol de la sesión en el handshake.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionRole {
    /// Inicia el handshake (envía `ClientHello`).
    Client,
    /// Acepta el handshake (responde con `ServerHello`).
    Server,
}

/// Parámetros negociables de sesión.
#[derive(Debug, Clone)]
pub struct Config {
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_duration_ms: u8,
    pub max_bitrate: u32,
    /// Codec preferido (1 = PCM, 2 = Opus).
    pub codec_preferred: u8,
    /// Codecs aceptables en orden de preferencia local.
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

/// Una sesión activa con cifrado AEAD.
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
    /// Clave AEAD para cifrar (cliente→servidor o servidor→cliente según rol).
    encrypt_key: Mutex<Option<SessionKey>>,
    /// Clave AEAD para descifrar (dirección opuesta).
    decrypt_key: Mutex<Option<SessionKey>>,
    /// Controlador de congestión AIMD.
    congestion: CongestionController,
    /// Encoder FEC (XOR parity por ventana de frames).
    fec_enc: Mutex<FecEncoder>,
    /// Decoder FEC (recupera un frame perdido por ventana).
    fec_dec: Mutex<FecDecoder>,
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
        let max_br = config.max_bitrate;
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
            encrypt_key: Mutex::new(None),
            decrypt_key: Mutex::new(None),
            congestion: CongestionController::new(max_br, CONGESTION_MIN_BITRATE, max_br),
            fec_enc: Mutex::new(FecEncoder::with_default_window()),
            fec_dec: Mutex::new(FecDecoder::with_default_window()),
        }
    }

    /// Codec acordado tras el handshake. Devuelve `0` si aún no se completó.
    #[must_use]
    pub fn negotiated_codec(&self) -> u8 {
        self.negotiated_codec.load(Ordering::Acquire)
    }

    /// Bitrate estimado actual según el controlador de congestión (bps).
    #[must_use]
    pub fn current_bitrate(&self) -> u32 {
        self.congestion.current_bitrate()
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

    /// Ejecuta el handshake criptográfico 4-way.
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

    // ── Handshake cliente ───────────────────────────────────────────────────

    async fn handshake_client(&self, peer: SocketAddr) -> Result<(), TransportError> {
        // 1. Generar clave efímera X25519 y nonce criptográfico.
        let client_secret = EphemeralSecret::random_from_rng(rand_core::OsRng);
        let client_pubkey = PublicKey::from(&client_secret);
        let client_nonce = random_nonce_32();

        let hello = ClientHello {
            ephemeral_public_key: *client_pubkey.as_bytes(),
            client_nonce,
            // Proponemos la versión máxima que soportamos; el servidor puede
            // hacer downgrade hasta PROTOCOL_VERSION_MIN.
            protocol_version: PROTOCOL_VERSION_MAX,
            codec_preferred: self.config.codec_preferred,
            sample_rate: self.config.sample_rate,
            channels: self.config.channels,
            frame_duration_ms: self.config.frame_duration_ms,
            max_bitrate: self.config.max_bitrate,
            capability_flags: self.config.capability_flags,
        };

        let mut hello_payload = [0u8; ClientHello::SIZE];
        hello.encode(&mut hello_payload).map_err(TransportError::Protocol)?;

        // Reintento con backoff hasta el timeout del caller.
        let mut attempt: u32 = 0;
        let mut buf = vec![0u8; self.config.mtu];
        loop {
            self.send_control(MessageType::HandshakeClientHello, 0, &hello_payload, peer)
                .await?;

            let backoff = Duration::from_millis(HANDSHAKE_RETRY_BASE_MS << attempt.min(4));
            let res = timeout(backoff, self.transport.recv(&mut buf)).await;

            if let Ok(Ok((n, from))) = res {
                if from != peer {
                    attempt = attempt.saturating_add(1);
                    continue;
                }
                let view = match PacketView::decode(&buf[..n]) {
                    Ok(v) => v,
                    Err(_) => {
                        attempt = attempt.saturating_add(1);
                        continue;
                    }
                };
                if view.header().msg_type != MessageType::HandshakeServerHello.code() {
                    attempt = attempt.saturating_add(1);
                    continue;
                }

                // 2. Decodificar ServerHello.
                let server_hello = ServerHello::decode(view.payload())
                    .map_err(TransportError::Protocol)?;

                // Validación de versión negociada: el servidor sólo puede
                // hacer downgrade (nunca proponer una versión más alta que la
                // que pedimos) y no puede proponer algo fuera del rango.
                let neg_ver = server_hello.protocol_version;
                if neg_ver < PROTOCOL_VERSION_MIN || neg_ver > PROTOCOL_VERSION_MAX {
                    return Err(TransportError::Handshake("version negotiation failed: out of range"));
                }
                if !self.config.supported_codecs.contains(&server_hello.codec_accepted) {
                    return Err(TransportError::Handshake("server selected unsupported codec"));
                }

                // 3. ECDH + HKDF → encrypt_key, decrypt_key.
                let server_pubkey = PublicKey::from(server_hello.ephemeral_public_key);
                let shared = client_secret.diffie_hellman(&server_pubkey);
                let session_id = server_hello.session_id;

                let transcript = build_transcript(&client_nonce, &server_hello.server_nonce, session_id);
                let (enc_key, dec_key) = derive_session_keys(shared.as_bytes(), &transcript);

                // 4. Calcular auth_tag del cliente y enviar KeyExchange.
                let client_auth_tag = derive_auth_tag(&enc_key, b"GS-client-fin-v1", &transcript);
                let ke_msg = KeyExchangeMsg {
                    session_id,
                    auth_tag: client_auth_tag,
                };
                let mut ke_payload = [0u8; KeyExchangeMsg::SIZE];
                ke_msg.encode(&mut ke_payload).map_err(TransportError::Protocol)?;
                self.send_control(MessageType::HandshakeKeyExchange, session_id, &ke_payload, peer)
                    .await?;

                // 5. Esperar SessionConfirm del servidor.
                let confirm = self.recv_session_confirm(peer, &mut buf, session_id).await?;

                // 6. Verificar auth_tag del servidor.
                let expected_server_tag = derive_auth_tag(&dec_key, b"GS-server-fin-v1", &transcript);
                if !constant_time_eq(&confirm.server_auth_tag, &expected_server_tag) {
                    return Err(TransportError::AuthenticationFailed(
                        "server auth tag mismatch",
                    ));
                }

                // 7. Almacenar estado de sesión.
                self.session_id.store(session_id, Ordering::Release);
                self.negotiated_codec.store(server_hello.codec_accepted, Ordering::Release);
                *self.encrypt_key.lock().await = Some(enc_key);
                *self.decrypt_key.lock().await = Some(dec_key);
                return Ok(());
            }

            attempt = attempt.saturating_add(1);
            if attempt > 6 {
                return Err(TransportError::Handshake("client retries exhausted"));
            }
        }
    }

    async fn recv_session_confirm(
        &self,
        peer: SocketAddr,
        buf: &mut Vec<u8>,
        expected_sid: u32,
    ) -> Result<SessionConfirm, TransportError> {
        loop {
            let (n, from) = self.transport.recv(buf).await?;
            if from != peer {
                continue;
            }
            let view = match PacketView::decode(&buf[..n]) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if view.header().msg_type != MessageType::HandshakeSessionConfirm.code() {
                continue;
            }
            let confirm = SessionConfirm::decode(view.payload()).map_err(TransportError::Protocol)?;
            if confirm.session_id != expected_sid {
                return Err(TransportError::Handshake("session_id mismatch in confirm"));
            }
            return Ok(confirm);
        }
    }

    // ── Handshake servidor ──────────────────────────────────────────────────

    async fn handshake_server(&self, peer: SocketAddr) -> Result<(), TransportError> {
        let mut buf = vec![0u8; self.config.mtu];

        // 1. Esperar ClientHello del peer esperado.
        let client_hello: ClientHello = loop {
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
            if view.header().msg_type != MessageType::HandshakeClientHello.code() {
                tracing::debug!(
                    msg_type = view.header().msg_type,
                    "dropping non-ClientHello packet"
                );
                continue;
            }
            match ClientHello::decode(view.payload()) {
                Ok(h) => break h,
                Err(e) => return Err(TransportError::Protocol(e)),
            }
        };

        // Negociación de versión: el cliente propone su máxima versión soportada.
        // Hacemos downgrade si el cliente pide más de lo que tenemos, o
        // rechazamos si no hay rango compatible.
        let client_ver = client_hello.protocol_version;
        if client_ver < PROTOCOL_VERSION_MIN {
            return Err(TransportError::Handshake(
                "version negotiation failed: client version too old",
            ));
        }
        let negotiated_version = client_ver.min(PROTOCOL_VERSION_MAX);

        // 2. Generar clave efímera, nonce y session_id del servidor.
        let server_secret = EphemeralSecret::random_from_rng(rand_core::OsRng);
        let server_pubkey = PublicKey::from(&server_secret);
        let server_nonce = random_nonce_32();
        let session_id = rand_u32_secure();
        self.session_id.store(session_id, Ordering::Release);

        // 3. Negociar codec.
        let chosen_codec = if self.config.supported_codecs.contains(&client_hello.codec_preferred) {
            client_hello.codec_preferred
        } else {
            *self
                .config
                .supported_codecs
                .first()
                .ok_or(TransportError::Handshake("no supported codecs configured"))?
        };
        self.negotiated_codec.store(chosen_codec, Ordering::Release);

        // 4. Enviar ServerHello.
        let server_hello = ServerHello {
            ephemeral_public_key: *server_pubkey.as_bytes(),
            server_nonce,
            session_id,
            protocol_version: negotiated_version,
            codec_accepted: chosen_codec,
            sample_rate: client_hello.sample_rate,
            channels: client_hello.channels,
            frame_duration_ms: client_hello.frame_duration_ms,
            max_bitrate: client_hello.max_bitrate.min(self.config.max_bitrate),
            capability_flags: client_hello.capability_flags & self.config.capability_flags,
        };
        let mut sh_payload = [0u8; ServerHello::SIZE];
        server_hello.encode(&mut sh_payload).map_err(TransportError::Protocol)?;
        self.send_control(MessageType::HandshakeServerHello, session_id, &sh_payload, peer)
            .await?;

        // 5. ECDH + HKDF → encrypt_key, decrypt_key (perspectiva servidor).
        let client_pubkey = PublicKey::from(client_hello.ephemeral_public_key);
        let shared = server_secret.diffie_hellman(&client_pubkey);

        let transcript = build_transcript(&client_hello.client_nonce, &server_nonce, session_id);
        // Servidor: "encrypt" = clave para cifrar hacia el cliente (decrypt_key del cliente).
        //           "decrypt" = clave para descifrar del cliente (encrypt_key del cliente).
        // HKDF usa los mismos labels que el cliente pero los keys se intercambian de perspectiva:
        //   enc_key (servidor) = dec_key (cliente)
        //   dec_key (servidor) = enc_key (cliente)
        let (client_enc, client_dec) = derive_session_keys(shared.as_bytes(), &transcript);
        let (enc_key, dec_key) = (client_dec, client_enc);

        // 6. Esperar KeyExchange del cliente.
        let ke_msg: KeyExchangeMsg = loop {
            let (n, from) = self.transport.recv(&mut buf).await?;
            if from != peer {
                continue;
            }
            let view = match PacketView::decode(&buf[..n]) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if view.header().msg_type != MessageType::HandshakeKeyExchange.code() {
                continue;
            }
            match KeyExchangeMsg::decode(view.payload()) {
                Ok(m) => break m,
                Err(e) => return Err(TransportError::Protocol(e)),
            }
        };

        if ke_msg.session_id != session_id {
            return Err(TransportError::Handshake("session_id mismatch in KeyExchange"));
        }

        // 7. Verificar auth_tag del cliente.
        // El cliente usó su encrypt_key (= dec_key del servidor) para derivar el tag.
        let expected_client_tag = derive_auth_tag(&dec_key, b"GS-client-fin-v1", &transcript);
        if !constant_time_eq(&ke_msg.auth_tag, &expected_client_tag) {
            return Err(TransportError::AuthenticationFailed("client auth tag mismatch"));
        }

        // 8. Enviar SessionConfirm con auth_tag del servidor.
        let server_auth_tag = derive_auth_tag(&enc_key, b"GS-server-fin-v1", &transcript);
        let confirm = SessionConfirm {
            session_id,
            server_auth_tag,
        };
        let mut sc_payload = [0u8; SessionConfirm::SIZE];
        confirm.encode(&mut sc_payload).map_err(TransportError::Protocol)?;
        self.send_control(MessageType::HandshakeSessionConfirm, session_id, &sc_payload, peer)
            .await?;

        // 9. Almacenar claves.
        *self.encrypt_key.lock().await = Some(enc_key);
        *self.decrypt_key.lock().await = Some(dec_key);
        Ok(())
    }

    // ── Audio send/recv ─────────────────────────────────────────────────────

    /// Envía un frame de audio cifrado con AEAD. Requiere estado `Active`.
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
        let sid = self.session_id.load(Ordering::Acquire);

        // Construir header con flag ENCRYPTED.
        let mut header = PacketHeader {
            version: 1,
            flags: Flags::ENCRYPTED,
            msg_type: MessageType::AudioFrame.code(),
            session_id: sid,
            sequence: seq,
            timestamp: ts,
        };

        // Cifrar payload si hay clave disponible.
        let (wire_payload, encrypt_flag_active) = if let Some(key) = self.encrypt_key.lock().await.as_ref() {
            let nonce = make_nonce(seq, sid);
            // Necesitamos AAD = header codificado.
            let mut hdr_buf = [0u8; HEADER_SIZE];
            header.encode(&mut hdr_buf).map_err(TransportError::Protocol)?;

            let mut cipher_buf = vec![0u8; payload.len() + TAG_SIZE];
            cipher_buf[..payload.len()].copy_from_slice(payload);
            let enc_len = encrypt_in_place(key, &nonce, &hdr_buf, &mut cipher_buf, payload.len())
                .map_err(TransportError::Protocol)?;
            (Bytes::copy_from_slice(&cipher_buf[..enc_len]), true)
        } else {
            // Sin clave: fallback a texto claro (no debería ocurrir en sesión Active).
            header.flags = Flags::empty();
            (Bytes::copy_from_slice(payload), false)
        };
        let _ = encrypt_flag_active;

        let mut buf = BytesMut::with_capacity(self.config.mtu);
        buf.resize(self.config.mtu, 0);
        let n = PacketBuilder::new(header, &wire_payload)
            .encode(&mut buf)
            .map_err(TransportError::Protocol)?;
        let sent = self.transport.send_to(&buf[..n], peer).await?;
        self.metrics.counters.record_sent(sent as u64);

        // FEC: alimentar encoder con plaintext; enviar paridad si se completó la ventana.
        if let Some(parity) = self.fec_enc.lock().await.push(seq, payload) {
            let _ = self.send_fec_parity(parity, peer, sid).await;
        }

        Ok(())
    }

    async fn send_fec_parity(
        &self,
        parity: FecParity,
        peer: SocketAddr,
        sid: u32,
    ) -> Result<(), TransportError> {
        // Wire layout del payload FEC: seq_base(4BE) + window(1) + parity_data
        let mut fec_payload = Vec::with_capacity(5 + parity.payload.len() + TAG_SIZE);
        fec_payload.extend_from_slice(&parity.seq_base.to_be_bytes());
        fec_payload.push(parity.window);
        fec_payload.extend_from_slice(&parity.payload);

        let seq = self.tx_sequence.fetch_add(1, Ordering::Relaxed);
        let ts = self.micros_since_epoch();
        let header = PacketHeader {
            version: 1,
            flags: Flags::ENCRYPTED,
            msg_type: MessageType::AudioFec.code(),
            session_id: sid,
            sequence: seq,
            timestamp: ts,
        };

        if let Some(key) = self.encrypt_key.lock().await.as_ref() {
            let nonce = make_nonce(seq, sid);
            let mut hdr_buf = [0u8; HEADER_SIZE];
            header.encode(&mut hdr_buf).map_err(TransportError::Protocol)?;

            let plain_len = fec_payload.len();
            fec_payload.resize(plain_len + TAG_SIZE, 0);
            let enc_len = encrypt_in_place(key, &nonce, &hdr_buf, &mut fec_payload, plain_len)
                .map_err(TransportError::Protocol)?;

            let mut buf = BytesMut::with_capacity(self.config.mtu);
            buf.resize(self.config.mtu, 0);
            let n = PacketBuilder::new(header, &fec_payload[..enc_len])
                .encode(&mut buf)
                .map_err(TransportError::Protocol)?;
            let sent = self.transport.send_to(&buf[..n], peer).await?;
            self.metrics.counters.record_sent(sent as u64);
        }
        Ok(())
    }

    /// Recibe el próximo frame de audio ya desjitterizado.
    pub async fn recv_audio(&self) -> Result<Frame, TransportError> {
        loop {
            if let Some(frame) = self.jitter.pop() {
                return Ok(frame);
            }
            self.poll_once().await?;
        }
    }

    /// Procesa un único datagrama entrante.
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
        self.last_rx.store(self.micros_since_epoch(), Ordering::Release);

        let mt = view.header().msg_type;
        match MessageType::from_code(mt) {
            Ok(MessageType::AudioFrame) => {
                let sid = self.session_id.load(Ordering::Acquire);
                let seq = view.header().sequence;
                let ts = view.header().timestamp;

                // Descifrar si el flag ENCRYPTED está activo.
                let plaintext = if view.header().flags.contains(Flags::ENCRYPTED) {
                    if let Some(key) = self.decrypt_key.lock().await.as_ref() {
                        let nonce = make_nonce(seq, sid);
                        // AAD = header original (primeros HEADER_SIZE bytes del buffer)
                        let aad = &buf[..HEADER_SIZE];
                        let cipher_payload = view.payload();
                        let mut tmp = cipher_payload.to_vec();
                        match decrypt_in_place(key, &nonce, aad, &mut tmp, cipher_payload.len()) {
                            Ok(plain_len) => Bytes::copy_from_slice(&tmp[..plain_len]),
                            Err(e) => {
                                self.metrics.counters.record_integrity_error();
                                tracing::debug!(?e, seq, "AEAD decryption failed, dropping frame");
                                return Ok(());
                            }
                        }
                    } else {
                        // Sesión sin clave: rechazar frame cifrado.
                        self.metrics.counters.record_integrity_error();
                        return Ok(());
                    }
                } else {
                    Bytes::copy_from_slice(view.payload())
                };

                let frame = Frame { sequence: seq, timestamp: ts, payload: plaintext.clone() };
                self.metrics.loss.record(frame.sequence);
                self.metrics.jitter.record(frame.timestamp, self.micros_since_epoch());
                // Registrar en el decoder FEC para posible recuperación del siguiente.
                self.fec_dec.lock().await.push_data(seq, plaintext);
                if !self.jitter.push(frame) {
                    tracing::trace!(seq, "jitter buffer rejected frame");
                }
            }
            Ok(MessageType::Heartbeat) => {
                let peer = self.peer.lock().await;
                if let Some(p) = *peer {
                    self.send_control(MessageType::HeartbeatAck, self.session_id(), &[], p)
                        .await?;
                }
            }
            Ok(MessageType::HeartbeatAck) => {}
            Ok(MessageType::AudioFec) => {
                let sid = self.session_id.load(Ordering::Acquire);
                let seq = view.header().sequence;
                if view.header().flags.contains(Flags::ENCRYPTED) {
                    if let Some(key) = self.decrypt_key.lock().await.as_ref() {
                        let nonce = make_nonce(seq, sid);
                        let aad = &buf[..HEADER_SIZE];
                        let cipher_payload = view.payload();
                        let mut tmp = cipher_payload.to_vec();
                        match decrypt_in_place(key, &nonce, aad, &mut tmp, cipher_payload.len()) {
                            Ok(plain_len) if plain_len >= 5 => {
                                let fec_data = &tmp[..plain_len];
                                let seq_base = u32::from_be_bytes([
                                    fec_data[0], fec_data[1], fec_data[2], fec_data[3],
                                ]);
                                let window = fec_data[4];
                                let fec_parity = FecParity {
                                    seq_base,
                                    window,
                                    payload: Bytes::copy_from_slice(&fec_data[5..]),
                                };
                                if let Some((rec_seq, rec_payload)) =
                                    self.fec_dec.lock().await.push_parity(fec_parity)
                                {
                                    let frame = Frame {
                                        sequence: rec_seq,
                                        timestamp: 0,
                                        payload: rec_payload,
                                    };
                                    if !self.jitter.push(frame) {
                                        tracing::trace!(rec_seq, "jitter buffer rejected FEC-recovered frame");
                                    }
                                }
                            }
                            Ok(_) => {
                                tracing::debug!(seq, "AudioFec payload too short");
                            }
                            Err(e) => {
                                self.metrics.counters.record_integrity_error();
                                tracing::debug!(?e, seq, "AudioFec AEAD decryption failed");
                            }
                        }
                    }
                }
            }
            Ok(MessageType::ControlBitrate) => {
                if let Ok(msg) = ControlBitrateMsg::decode(view.payload()) {
                    self.congestion.set_bitrate(msg.requested_bitrate);
                    tracing::debug!(bitrate = msg.requested_bitrate, "ControlBitrate received");
                }
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
        // Borrar claves de sesión al cerrar.
        *self.encrypt_key.lock().await = None;
        *self.decrypt_key.lock().await = None;

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

        // Actualizar controlador de congestión con métricas actuales.
        let loss_rate = self.metrics.loss.loss_percent() / 100.0;
        let rtt_us = self.metrics.rtt.current_us().unwrap_or(0) as u64;
        let jitter_us = (self.metrics.jitter.current_ms() * 1_000.0) as u64;
        self.congestion.update(loss_rate, rtt_us, jitter_us);

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
        self.epoch.elapsed().as_micros() as u64
    }
}

// ── Helpers criptográficos ──────────────────────────────────────────────────

/// transcript = client_nonce (32) || server_nonce (32) || session_id BE (4)
fn build_transcript(client_nonce: &[u8; 32], server_nonce: &[u8; 32], session_id: u32) -> [u8; 68] {
    let mut t = [0u8; 68];
    t[..32].copy_from_slice(client_nonce);
    t[32..64].copy_from_slice(server_nonce);
    t[64..68].copy_from_slice(&session_id.to_be_bytes());
    t
}

/// Deriva encrypt_key y decrypt_key desde el shared_secret (X25519) y el transcript.
///
/// HKDF-SHA256:
///   salt = transcript
///   IKM  = shared_secret
///   info para encrypt_key = b"GS-encrypt-v1"
///   info para decrypt_key = b"GS-decrypt-v1"
fn derive_session_keys(shared_secret: &[u8; 32], transcript: &[u8; 68]) -> (SessionKey, SessionKey) {
    let hkdf = Hkdf::<Sha256>::new(Some(transcript.as_ref()), shared_secret);
    let mut encrypt_key = [0u8; 32];
    let mut decrypt_key = [0u8; 32];
    hkdf.expand(b"GS-encrypt-v1", &mut encrypt_key)
        .expect("HKDF expand encrypt_key");
    hkdf.expand(b"GS-decrypt-v1", &mut decrypt_key)
        .expect("HKDF expand decrypt_key");
    (encrypt_key, decrypt_key)
}

/// Deriva un auth_tag de 16 bytes: HKDF-Expand(key, label || transcript)[..16].
fn derive_auth_tag(key: &SessionKey, label: &[u8], transcript: &[u8; 68]) -> [u8; 16] {
    // Construir info = label || transcript
    let mut info = Vec::with_capacity(label.len() + 68);
    info.extend_from_slice(label);
    info.extend_from_slice(transcript);

    let hkdf = Hkdf::<Sha256>::new(None, key);
    let mut tag = [0u8; 16];
    hkdf.expand(&info, &mut tag).expect("HKDF expand auth_tag");
    tag
}

/// Genera un nonce criptográfico de 32 bytes usando OsRng.
fn random_nonce_32() -> [u8; 32] {
    use rand_core::RngCore;
    let mut nonce = [0u8; 32];
    rand_core::OsRng.fill_bytes(&mut nonce);
    nonce
}

/// Genera un session_id criptográficamente aleatorio.
fn rand_u32_secure() -> u32 {
    use rand_core::RngCore;
    rand_core::OsRng.next_u32()
}

/// Comparación en tiempo constante de dos slices de igual longitud.
#[inline]
fn constant_time_eq(a: &[u8; 16], b: &[u8; 16]) -> bool {
    let mut diff: u8 = 0;
    for i in 0..16 {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

fn jitter_slots(buffer_ms: u16, frame_ms: u8) -> u32 {
    let frames = (buffer_ms / frame_ms.max(1) as u16).max(1) as u32;
    frames.next_power_of_two().max(16)
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
    fn constant_time_eq_works() {
        assert!(constant_time_eq(&[0u8; 16], &[0u8; 16]));
        let mut a = [0u8; 16];
        a[0] = 1;
        assert!(!constant_time_eq(&a, &[0u8; 16]));
    }

    #[test]
    fn key_derivation_deterministic() {
        let secret = [0x42u8; 32];
        let transcript = build_transcript(&[0xAA; 32], &[0xBB; 32], 0x1234_5678);
        let (enc1, dec1) = derive_session_keys(&secret, &transcript);
        let (enc2, dec2) = derive_session_keys(&secret, &transcript);
        assert_eq!(enc1, enc2);
        assert_eq!(dec1, dec2);
        assert_ne!(enc1, dec1, "encrypt and decrypt keys must differ");
    }

    #[test]
    fn auth_tag_deterministic() {
        let key = [0x11u8; 32];
        let transcript = build_transcript(&[1u8; 32], &[2u8; 32], 42);
        let tag1 = derive_auth_tag(&key, b"GS-client-fin-v1", &transcript);
        let tag2 = derive_auth_tag(&key, b"GS-client-fin-v1", &transcript);
        assert_eq!(tag1, tag2);
        let tag3 = derive_auth_tag(&key, b"GS-server-fin-v1", &transcript);
        assert_ne!(tag1, tag3, "client and server tags must differ");
    }

    #[test]
    fn rand_u32_secure_varies() {
        let a = rand_u32_secure();
        let b = rand_u32_secure();
        // Con probabilidad 1 - 1/2^32 ≈ 1, serán distintos.
        assert_ne!(a, b);
    }
}
