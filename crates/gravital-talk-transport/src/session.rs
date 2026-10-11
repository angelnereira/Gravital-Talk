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

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::{Bytes, BytesMut};
use gravital_talk_core::constants::{
    CONGESTION_MIN_BITRATE, DEFAULT_FRAME_DURATION_MS, DEFAULT_JITTER_BUFFER_MS,
    DEFAULT_MAX_BITRATE, DEFAULT_MTU, DEFAULT_SAMPLE_RATE, HANDSHAKE_RETRY_BASE_MS,
    HANDSHAKE_TIMEOUT_MS, HEADER_SIZE, HEARTBEAT_INTERVAL_MS, HEARTBEAT_TIMEOUT_MS,
    PROTOCOL_VERSION_MAX, PROTOCOL_VERSION_MIN,
};
use gravital_talk_core::crypto::{
    decrypt_in_place, encrypt_in_place, make_nonce, SessionKey, TAG_SIZE,
};
use gravital_talk_core::header::{Flags, PacketHeader};
use gravital_talk_core::message::{
    ClientHello, ControlBitrateMsg, KeyExchangeMsg, MessageType, ServerHello, SessionConfirm,
};
#[cfg(feature = "noise")]
use gravital_talk_core::message::{NoiseHello1, NoiseHello2};
use gravital_talk_core::packet::{PacketBuilder, PacketView};
use gravital_talk_core::session::{SessionEvent, SessionState, SessionStateMachine};
use gravital_talk_metrics::Metrics;
use hkdf::Hkdf;
use sha2::Sha256;
use tokio::sync::Mutex;
use tokio::time::{timeout, Instant};
use x25519_dalek::{EphemeralSecret, PublicKey};

use crate::address_book::AddressBook;
use crate::congestion::CongestionController;
use crate::error::TransportError;
use crate::fec::{FecDecoder, FecEncoder, FecParity};
use crate::jitter_buffer::{Frame, JitterBuffer};
#[cfg(feature = "noise")]
use crate::noise::NoiseHandshake;
use crate::rendezvous::{self, Message as RvMessage, RoomKey};
use crate::replay::ReplayWindow;
use crate::traits::Transport;

/// Timeout del intento Noise antes de caer al handshake legacy (modo `Auto`).
const NOISE_FALLBACK_MS: u64 = 1500;

/// Cada cuánto se renueva el registro en el servidor de encuentro y se
/// repiten las sondas de perforación.
const PUNCH_INTERVAL_MS: u64 = 250;

/// Cuánto espera el invitado a que algún camino conteste antes de rendirse.
const PATH_SELECT_TIMEOUT_MS: u64 = 6_000;

/// Silencio de envío tras el que se manda un latido.
///
/// Un NAT olvida una asociación UDP sin tráfico en decenas de segundos (los de
/// redes móviles, en menos), y en un walkie-talkie lo normal es estar callado.
/// El latido mantiene el camino abierto y, de paso, es lo que anuncia la nueva
/// dirección de este extremo cuando cambia de red.
const KEEPALIVE_INTERVAL_MS: u64 = 1_000;

/// Silencio de recepción a partir del cual se da el camino por caído y se
/// buscan otros.
const PATH_STALE_MS: u64 = 2_500;

/// Puertos a cada lado del conocido que el anfitrión sonda además de él.
///
/// Un NAT simétrico (el de muchas redes móviles) usa un puerto público
/// distinto para cada destino: el que se conoce del invitado —el que vio el
/// servidor de encuentro o STUN— no es el que usará hacia aquí. Pero esos NAT
/// suelen repartir los puertos de un mismo equipo en una ventana estrecha, así
/// que sondar los vecinos abre este NAT también para el puerto de verdad.
/// Cuando el invitado sonda desde él, su paquete entra y de ahí se aprende.
const PUNCH_SPREAD: u16 = 192;

/// Cada cuántos ciclos de espera se repite el barrido de puertos vecinos.
const PUNCH_SPREAD_EVERY: u32 = 4;

/// Cada cuánto se renueva el registro en el servidor de encuentro con la
/// sesión en marcha y el camino sano.
const ACTIVE_REGISTER_MS: u64 = 5_000;

/// Cada cuánto anuncia cada extremo sus direcciones al otro.
const ADVERTISE_INTERVAL_MS: u64 = 5_000;

/// Direcciones alternativas del otro extremo que se recuerdan.
const MAX_PATH_HINTS: usize = 12;

/// Margen que se da a un camino local para contestar cuando ya contestó uno
/// público: entre dos caminos que funcionan, el local tiene menos latencia.
const LOCAL_PATH_GRACE_MS: u64 = 150;

/// Primer mensaje de handshake recibido por el servidor.
enum FirstHandshake {
    /// Mensaje 1 de Noise (bytes crudos).
    Noise(Vec<u8>),
    /// `ClientHello` legacy.
    Legacy(ClientHello),
}

/// Modo de handshake de la sesión.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum HandshakeMode {
    /// Intenta Noise y cae a legacy si el peer no responde (default).
    ///
    /// Con `room_token` configurado **no hay downgrade**: el token exige
    /// Noise (PSK) para autenticar la sala.
    #[default]
    Auto,
    /// Solo Noise (falla contra peers legacy).
    Noise,
    /// Solo handshake legacy X25519 (compatibilidad).
    Legacy,
}

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
    /// Modo de handshake (Noise / legacy / auto).
    pub handshake_mode: HandshakeMode,
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
            handshake_mode: HandshakeMode::Auto,
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
    /// Agenda de peers conocidos. En P2P hay uno solo; en sala (relay) puede
    /// haber varios, y el audio se reenvía a todos los que no son el emisor.
    ///
    /// El primero registrado es el peer principal: en sala es el host, que
    /// arbitra el floor control.
    book: Mutex<AddressBook>,
    session_id: AtomicU32,
    tx_sequence: AtomicU32,
    /// Contador exclusivo para frames de audio (no se comparte con FEC ni control).
    /// Se embebe en los primeros 4 bytes del payload cifrado para que el jitter
    /// buffer del receptor use una secuencia sin huecos.
    audio_tx_seq: AtomicU32,
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
    /// `true` mientras el usuario local tiene el PTT presionado.
    ptt_active: AtomicBool,
    /// `true` cuando el peer remoto está transmitiendo (recibido ControlResume).
    peer_ptt_active: AtomicBool,
    /// SSRC del participante local (derivado del session_id).
    local_ssrc: AtomicU32,
    /// `session_id` fijado de antemano para el handshake (modo relay/sala).
    /// `0` = el servidor elige uno aleatorio (P2P directo).
    preset_session_id: AtomicU32,
    /// Token de sala (PSK de Noise). `None` = sala abierta.
    room_token: std::sync::Mutex<Option<Vec<u8>>>,
    /// Modo de handshake efectivo (1 = Legacy, 2 = Noise, otro = Auto).
    handshake_mode: AtomicU8,
    /// Ventanas anti-replay indexadas por dirección de origen.
    ///
    /// En P2P hay una sola entrada. En sala hay una por participante, porque
    /// cada uno mantiene su propio contador `tx_sequence`: con una ventana
    /// compartida, el audio legítimo de un peer se descartaría como duplicado
    /// del de otro.
    replay_by_peer: Mutex<HashMap<SocketAddr, ReplayWindow>>,
    /// Señal de cancelación: se activa en `close()` para interrumpir loops bloqueantes.
    closed: AtomicBool,
    /// Servidor de encuentro para perforar NAT. `None` = sólo caminos directos.
    rendezvous: std::sync::Mutex<Option<SocketAddr>>,
    /// Último envío de cualquier tipo, en µs desde `epoch`.
    last_tx: AtomicU64,
    /// Direcciones por las que buscar al otro extremo si el camino actual
    /// deja de funcionar: las candidatas de la invitación y las que se
    /// añadan con [`Session::add_path_hints`].
    path_hints: std::sync::Mutex<Vec<SocketAddr>>,
    /// Nonce de las sondas de recuperación de camino en curso.
    probe_nonce: std::sync::Mutex<[u8; 8]>,
    /// Última sonda de recuperación y último registro, en µs desde `epoch`.
    last_probe: AtomicU64,
    last_register: AtomicU64,
    /// Si la sesión admite que entren más peers una vez establecida.
    multi_peer: AtomicBool,
    /// Ir siempre por el relay, sin intentar caminos directos.
    relay_only: AtomicBool,
    /// Dirección pública de este extremo según STUN, si se llegó a pedir.
    own_public: std::sync::Mutex<Option<SocketAddr>>,
    /// Último anuncio de direcciones, en µs desde `epoch`.
    last_advert: AtomicU64,
    /// Desde cuándo se espera a un frame que falta con otros ya recibidos
    /// detrás, en µs desde `epoch`. `0` = no se está esperando.
    gap_since: AtomicU64,
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
        let handshake_mode_byte = match config.handshake_mode {
            HandshakeMode::Legacy => 1,
            HandshakeMode::Noise => 2,
            HandshakeMode::Auto => 0,
        };
        Self {
            transport,
            state: Mutex::new(SessionStateMachine::new()),
            metrics: Arc::new(Metrics::new()),
            jitter: Arc::new(JitterBuffer::new(jitter_depth)),
            config,
            book: Mutex::new(AddressBook::new()),
            session_id: AtomicU32::new(0),
            tx_sequence: AtomicU32::new(0),
            audio_tx_seq: AtomicU32::new(0),
            last_rx: AtomicU64::new(0),
            negotiated_codec: AtomicU8::new(0),
            epoch: Instant::now(),
            encrypt_key: Mutex::new(None),
            decrypt_key: Mutex::new(None),
            congestion: CongestionController::new(max_br, CONGESTION_MIN_BITRATE, max_br),
            fec_enc: Mutex::new(FecEncoder::with_default_window()),
            fec_dec: Mutex::new(FecDecoder::with_default_window()),
            ptt_active: AtomicBool::new(false),
            peer_ptt_active: AtomicBool::new(false),
            local_ssrc: AtomicU32::new(0),
            preset_session_id: AtomicU32::new(0),
            room_token: std::sync::Mutex::new(None),
            handshake_mode: AtomicU8::new(handshake_mode_byte),
            replay_by_peer: Mutex::new(HashMap::new()),
            closed: AtomicBool::new(false),
            rendezvous: std::sync::Mutex::new(None),
            last_tx: AtomicU64::new(0),
            path_hints: std::sync::Mutex::new(Vec::new()),
            probe_nonce: std::sync::Mutex::new(rendezvous::random_nonce()),
            last_probe: AtomicU64::new(0),
            last_register: AtomicU64::new(0),
            multi_peer: AtomicBool::new(false),
            relay_only: AtomicBool::new(false),
            own_public: std::sync::Mutex::new(None),
            last_advert: AtomicU64::new(0),
            gap_since: AtomicU64::new(0),
        }
    }

    /// Rearma una sesión cerrada para poder emparejar de nuevo.
    ///
    /// Sin esto, una `Session` es de un solo uso: `close()` fija `closed=true`
    /// y todos los lazos de handshake lo consultan para salir, así que el
    /// mismo objeto nunca puede volver a negociar. El requisito de producto es
    /// justo el contrario —"cualquiera de los dos cierra y vuelve a
    /// emparejar"—, así que esto es lo que lo hace posible.
    ///
    /// Preserva lo que es configuración de la sala: `config`, el token de sala
    /// (`room_token`) y el modo de handshake. Un emparejamiento nuevo sobre la
    /// misma sala no debería tener que reconfigurarlos.
    ///
    /// Resetea lo que es estado de la conexión anterior: claves, agenda de
    /// peers, ventanas anti-replay, contadores de secuencia, jitter buffer,
    /// FEC, congestión y flags de PTT. Dejarlos sucios haría que el segundo
    /// emparejamiento hereclara estado del primero: la ventana anti-replay
    /// descartaría paquetes legítimamente numerados, y el jitter buffer
    /// entregaría audio de la sesión anterior.
    ///
    /// Devuelve `Err` si la sesión no estaba cerrada: rearrancar una sesión
    /// activa en caliente perdería audio sin aviso, y debe ser una decisión
    /// explícita (`close()` primero).
    pub async fn reopen(&self) -> Result<(), TransportError> {
        if !self.closed.load(Ordering::Acquire) {
            return Err(TransportError::InvalidState(
                "session is still open: close() it before reopening",
            ));
        }

        // Claves y estado criptográfico: lo primero, para que ni un paquete en
        // vuelo de la sesión anterior pueda descifrarse con ellas.
        *self.encrypt_key.lock().await = None;
        *self.decrypt_key.lock().await = None;
        self.replay_by_peer.lock().await.clear();
        self.book.lock().await.clear();

        // Contadores de secuencia: empiezan de cero, como en `new()`.
        self.session_id.store(0, Ordering::Release);
        self.tx_sequence.store(0, Ordering::Release);
        self.audio_tx_seq.store(0, Ordering::Release);
        self.last_rx.store(0, Ordering::Release);
        self.negotiated_codec.store(0, Ordering::Release);
        self.local_ssrc.store(0, Ordering::Release);

        // Flags de PTT: si la sesión anterior se cerró a mitad de una
        // transmisión, el flag quedaría activo y la próxima empezaría
        // "transmitiendo" sin que nadie pulse nada.
        self.ptt_active.store(false, Ordering::Release);
        self.peer_ptt_active.store(false, Ordering::Release);

        // Jitter buffer, FEC y congestión: estado derivado del flujo anterior.
        self.jitter.reset();
        *self.fec_enc.lock().await = FecEncoder::with_default_window();
        *self.fec_dec.lock().await = FecDecoder::with_default_window();
        self.congestion.reset();

        // Métricas: se sustituye el acumulador para no mezclar dos sesiones.
        // El campo es `Arc<Metrics>` compartido con quien observe, así que se
        // reinicia en el sitio.
        self.metrics.reset();

        // La FSM acepta `(Closed, Reconnect)`, que es exactamente la
        // transición de "volver a emparejar".
        {
            let mut sm = self.state.lock().await;
            sm.transition(SessionEvent::Reconnect)
                .map_err(|_| TransportError::InvalidState("cannot reopen from current state"))?;
        }

        // La señal de cancelación va al final: mientras está puesta, ningún
        // lazo bloqueante arranca.
        self.closed.store(false, Ordering::Release);
        Ok(())
    }

    /// Modo de handshake efectivo.
    #[must_use]
    pub fn handshake_mode(&self) -> HandshakeMode {
        match self.handshake_mode.load(Ordering::Acquire) {
            1 => HandshakeMode::Legacy,
            2 => HandshakeMode::Noise,
            _ => HandshakeMode::Auto,
        }
    }

    /// Cambia el modo de handshake (debe llamarse antes de `handshake`).
    pub fn set_handshake_mode(&self, mode: HandshakeMode) {
        let byte = match mode {
            HandshakeMode::Legacy => 1,
            HandshakeMode::Noise => 2,
            HandshakeMode::Auto => 0,
        };
        self.handshake_mode.store(byte, Ordering::Release);
    }

    /// Fija el `session_id` que se usará en el handshake.
    ///
    /// Es lo que habilita el **modo servidor/relay**: todos los participantes
    /// de una sala comparten un `session_id` para que el relay pueda enrutar
    /// los paquetes del handshake. Sin un id pre-fijado, `ClientHello` viaja
    /// con `session_id = 0` y el relay lo descarta (no puede saber a qué sala
    /// pertenece).
    ///
    /// Debe llamarse **antes** de [`Session::handshake`]. Un valor `0`
    /// restaura el comportamiento P2P (el servidor elige un id aleatorio).
    pub fn set_preset_session_id(&self, id: u32) {
        self.preset_session_id.store(id, Ordering::Release);
    }

    /// `session_id` pre-fijado, o `0` si no hay ninguno.
    #[must_use]
    pub fn preset_session_id(&self) -> u32 {
        self.preset_session_id.load(Ordering::Acquire)
    }

    /// Fija el token de sala (PSK de Noise).
    ///
    /// Cuando hay token, el handshake usa `Noise_NNpsk0` y **no** hay
    /// downgrade a legacy: sin el token correcto la conexión falla.
    pub fn set_room_token(&self, token: Option<String>) {
        let bytes = token.filter(|t| !t.is_empty()).map(String::into_bytes);
        let mut guard = self.room_token.lock().unwrap_or_else(|p| p.into_inner());
        *guard = bytes;
    }

    /// Fija el servidor de encuentro que coordina la perforación de NAT.
    ///
    /// Con él, dos dispositivos en redes distintas pueden conectar directo: el
    /// anfitrión se registra mientras espera y el invitado al conectar, y cada
    /// uno aprende la dirección pública del otro. Ver [`crate::rendezvous`].
    ///
    /// Sólo actúa si la sala tiene token: la clave con la que los dos extremos
    /// se reconocen deriva de él, y sin token todas las salas abiertas del
    /// mundo compartirían la misma.
    pub fn set_rendezvous(&self, server: Option<SocketAddr>) {
        *self.rendezvous.lock().unwrap_or_else(|p| p.into_inner()) = server;
    }

    /// Servidor de encuentro efectivo: el configurado, si además hay token.
    fn rendezvous_server(&self) -> Option<SocketAddr> {
        if !self.has_room_token() {
            return None;
        }
        *self.rendezvous.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Clave de encuentro de esta sala.
    ///
    /// Sin token sale la de la cadena vacía: sirve para las sondas directas
    /// entre extremos, que no pasan por ningún servidor.
    fn room_key(&self) -> RoomKey {
        let guard = self.room_token.lock().unwrap_or_else(|p| p.into_inner());
        RoomKey::from_token(guard.as_deref().unwrap_or_default())
    }

    /// Dirección pública del socket de esta sesión, vista por STUN.
    ///
    /// Es la que tiene sentido compartir en una invitación. Llamar **antes**
    /// del handshake: usa el mismo socket que el resto del tráfico.
    ///
    /// Tiene un tope de 3 s en total: es un dato opcional de la invitación y
    /// no debe retrasar la sala, y quien llama tiene que poder contar con que
    /// al volver ya nadie lee del socket.
    pub async fn discover_public_addr(&self) -> Result<SocketAddr, crate::stun::StunError> {
        let found = timeout(
            Duration::from_secs(3),
            crate::stun::discover_public_addr_on(self.transport.as_ref()),
        )
        .await
        .unwrap_or(Err(crate::stun::StunError::Timeout));
        // Se recuerda para anunciársela al otro extremo durante la sesión.
        if let Ok(addr) = found {
            *self.own_public.lock().unwrap_or_else(|p| p.into_inner()) = Some(addr);
        }
        found
    }

    /// Permite que entren más peers con la sesión ya establecida (modo sala).
    ///
    /// Desactivado por defecto, y no por comodidad: admitir un peer nuevo
    /// rehace el handshake, y hoy eso **sustituye las claves de la sesión**.
    /// En una sesión de dos, cualquiera que capture el primer mensaje del
    /// handshake puede reenviarlo desde otra dirección y dejar la
    /// conversación en curso sin poder descifrarse. Hasta que el modo sala
    /// lleve claves por peer, esto sólo se activa donde se quiere a propósito.
    pub fn set_multi_peer(&self, enabled: bool) {
        self.multi_peer.store(enabled, Ordering::Release);
    }

    /// Fuerza a que el audio vaya siempre por el relay.
    ///
    /// Por defecto el relay es el último recurso, cuando ningún camino directo
    /// contesta. Forzarlo tiene un uso legítimo: en una conexión directa cada
    /// extremo ve la IP del otro, y por el relay sólo ve la del relay.
    pub fn set_relay_only(&self, enabled: bool) {
        self.relay_only.store(enabled, Ordering::Release);
    }

    /// `true` si el audio de esta sesión está pasando por el relay.
    pub async fn is_relayed(&self) -> bool {
        let server = self.rendezvous_server();
        server.is_some() && self.book.lock().await.primary() == server
    }

    /// Prepara la sesión para poder encontrarse en el relay: fija el
    /// `session_id` que comparten los dos extremos. Sin servidor no hace nada.
    fn arm_relay(&self) -> Option<SocketAddr> {
        let server = self.rendezvous_server()?;
        self.set_preset_session_id(self.room_key().relay_session_id());
        Some(server)
    }

    /// Añade direcciones por las que buscar al otro extremo.
    ///
    /// Tiene dos usos. Mientras se espera invitado, el anfitrión las sonda en
    /// cada ciclo: es como se perfora sin servidor de encuentro, cuando la
    /// dirección del invitado llega por otro medio (su respuesta a la
    /// invitación). Con la sesión en marcha, son los caminos alternativos que
    /// se prueban si el actual deja de funcionar.
    pub fn add_path_hints(&self, addrs: &[SocketAddr]) {
        let mut hints = self.path_hints.lock().unwrap_or_else(|p| p.into_inner());
        for addr in addrs {
            if !hints.contains(addr) {
                hints.push(*addr);
            }
        }
        // Se conservan las más recientes: tras varios cambios de red, las
        // direcciones viejas dejan de servir.
        let excess = hints.len().saturating_sub(MAX_PATH_HINTS);
        hints.drain(..excess);
    }

    /// Direcciones por las que se puede alcanzar a este extremo ahora mismo:
    /// la de la red en la que está y la pública que le vio STUN.
    async fn own_addresses(&self) -> Vec<SocketAddr> {
        let mut addrs = Vec::with_capacity(2);
        if let (Ok(local), Ok(bound)) = (
            crate::stun::discover_local_addr().await,
            self.transport.local_addr(),
        ) {
            if !local.ip().is_unspecified() {
                addrs.push(SocketAddr::new(local.ip(), bound.port()));
            }
        }
        let public = *self.own_public.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(public) = public.filter(|p| !addrs.contains(p)) {
            addrs.push(public);
        }
        addrs
    }

    fn path_hints(&self) -> Vec<SocketAddr> {
        self.path_hints
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    fn probe_nonce(&self) -> [u8; 8] {
        *self.probe_nonce.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Milisegundos desde el último paquete recibido del otro extremo.
    ///
    /// `None` antes de recibir nada. Sirve a la interfaz para avisar de que la
    /// conexión se ha interrumpido antes de que el usuario hable al vacío.
    #[must_use]
    pub fn rx_idle_ms(&self) -> Option<u64> {
        match self.last_rx.load(Ordering::Acquire) {
            0 => None,
            last => Some(self.micros_since_epoch().saturating_sub(last) / 1_000),
        }
    }

    /// Latido autenticado hacia cada peer.
    ///
    /// Va cifrado (un AEAD sobre cuerpo vacío: sólo el tag) para que quien lo
    /// recibe pueda comprobar que viene de quien tiene las claves de la
    /// sesión. Eso es lo que permite aceptarlo desde una dirección nueva: ver
    /// [`Self::try_migrate`].
    async fn send_keepalive(&self) {
        let sid = self.session_id();
        for peer in self.broadcast_targets().await {
            self.send_keepalive_to(peer, sid).await;
        }
    }

    async fn send_keepalive_to(&self, peer: SocketAddr, sid: u32) {
        let seq = self.tx_sequence.fetch_add(1, Ordering::Relaxed);
        let mut header = PacketHeader {
            version: 1,
            flags: Flags::ENCRYPTED,
            msg_type: MessageType::Heartbeat.code(),
            session_id: sid,
            sequence: seq,
            timestamp: self.micros_since_epoch(),
        };
        let mut hdr = [0u8; HEADER_SIZE];
        if header.encode(&mut hdr).is_err() {
            return;
        }
        let mut body = [0u8; TAG_SIZE];
        let sealed = {
            let key = self.encrypt_key.lock().await;
            let sealed = key
                .as_ref()
                .map(|key| encrypt_in_place(key, &make_nonce(seq, sid), &hdr, &mut body, 0).ok());
            drop(key);
            sealed
        };
        // Sin claves aún no hay sesión que mantener.
        let Some(sealed) = sealed else {
            return;
        };
        let body_len = sealed.unwrap_or_else(|| {
            header.flags = Flags::empty();
            0
        });
        let mut buf = [0u8; HEADER_SIZE + TAG_SIZE + 8];
        if let Ok(n) = PacketBuilder::new(header, &body[..body_len]).encode(&mut buf) {
            if let Ok(sent) = self.transport.send_to(&buf[..n], peer).await {
                self.metrics.counters.record_sent(sent as u64);
                self.last_tx
                    .store(self.micros_since_epoch(), Ordering::Release);
            }
        }
    }

    /// Mantenimiento del camino con la sesión en marcha.
    ///
    /// Lo llama el lazo de recepción en cada vuelta, así que tiene que ser
    /// barato cuando no hay nada que hacer: un par de lecturas atómicas.
    ///
    /// - Si hace rato que no se envía nada, un latido.
    /// - Si hay servidor de encuentro, renovar el registro: es por donde el
    ///   otro extremo se entera de una dirección nueva cuando su NAT no deja
    ///   entrar el latido.
    /// - Si hace rato que no se recibe nada, sondar los caminos alternativos.
    async fn maintain_path(&self) {
        let now = self.micros_since_epoch();
        let since = |mark: &AtomicU64| now.saturating_sub(mark.load(Ordering::Acquire)) / 1_000;

        if since(&self.last_tx) >= KEEPALIVE_INTERVAL_MS {
            self.send_keepalive().await;
        }

        let last_rx = self.last_rx.load(Ordering::Acquire);
        let stale = last_rx != 0 && now.saturating_sub(last_rx) / 1_000 >= PATH_STALE_MS;

        if let Some(server) = self.rendezvous_server() {
            let every = if stale {
                KEEPALIVE_INTERVAL_MS
            } else {
                ACTIVE_REGISTER_MS
            };
            if since(&self.last_register) >= every {
                self.last_register.store(now, Ordering::Release);
                self.send_rv(&RvMessage::Register, server).await;
            }
        }

        if stale && since(&self.last_probe) >= PUNCH_INTERVAL_MS * 2 {
            let first = self.last_probe.swap(now, Ordering::AcqRel) == 0;
            let nonce = self.probe_nonce();
            for hint in self.path_hints() {
                self.send_rv(&RvMessage::Punch { nonce }, hint).await;
                // El barrido de vecinos, sólo al empezar a buscar y luego cada
                // pocos segundos: es caro y sólo hace falta si el otro lado
                // tiene un NAT simétrico.
                if (first || now / 1_000 % 4_000 < PUNCH_INTERVAL_MS * 2)
                    && !rendezvous::is_local(&hint)
                {
                    self.punch_neighbours(hint, nonce).await;
                }
            }
        } else if !stale {
            self.last_probe.store(0, Ordering::Release);
        }

        // Anunciar las direcciones propias mientras el camino funciona: es
        // la única forma de que el otro sepa dónde buscar cuando deje de
        // hacerlo. Avisar "he cambiado de red" después del cambio no sirve si
        // el aviso tiene que viajar por el camino que se acaba de perder.
        if !stale && since(&self.last_advert) >= ADVERTISE_INTERVAL_MS {
            self.last_advert.store(now, Ordering::Release);
            let addrs = self.own_addresses().await;
            if !addrs.is_empty() {
                let msg = RvMessage::Addrs { addrs };
                for peer in self.broadcast_targets().await {
                    self.send_rv(&msg, peer).await;
                }
            }
        }
    }

    /// Sonda los puertos vecinos de `around`. Ver [`PUNCH_SPREAD`].
    async fn punch_neighbours(&self, around: SocketAddr, nonce: [u8; 8]) {
        let msg = RvMessage::Punch { nonce };
        let base = around.port();
        for delta in 1..=PUNCH_SPREAD {
            for port in [base.checked_add(delta), base.checked_sub(delta)]
                .into_iter()
                .flatten()
                .filter(|p| *p >= 1024)
            {
                self.send_rv(&msg, SocketAddr::new(around.ip(), port)).await;
            }
        }
    }

    /// Cambia la dirección de un peer conservando su estado.
    async fn switch_peer(&self, old: SocketAddr, new: SocketAddr) {
        if !self.book.lock().await.replace(old, new) {
            return;
        }
        let mut windows = self.replay_by_peer.lock().await;
        if let Some(window) = windows.remove(&old) {
            windows.insert(new, window);
        }
        drop(windows);
        // La dirección anterior queda como alternativa: si el cambio fue
        // pasajero (wifi que va y viene), por ahí se vuelve.
        self.add_path_hints(&[old]);
        tracing::info!(?old, ?new, "el peer ha cambiado de dirección");
    }

    /// Decide si un paquete de una dirección desconocida es el peer de
    /// siempre, que ha cambiado de red.
    ///
    /// Se acepta sólo si **demuestra** tener las claves de la sesión —el AEAD
    /// verifica— y si su secuencia es nueva para la ventana anti-replay del
    /// peer. Lo segundo no es opcional: sin ello, cualquiera que capture un
    /// paquete legítimo podría reenviarlo desde otra dirección y llevarse el
    /// tráfico de la sesión.
    ///
    /// Sólo en sesiones con un peer: con varios, un paquete de una dirección
    /// nueva no dice cuál de ellos se ha movido.
    async fn try_migrate(&self, view: &PacketView<'_>, raw_buf: &[u8], from: SocketAddr) -> bool {
        let header = view.header();
        let kind = MessageType::from_code(header.msg_type);
        if !header.flags.contains(Flags::ENCRYPTED)
            || !matches!(kind, Ok(MessageType::AudioFrame | MessageType::Heartbeat))
        {
            return false;
        }
        let old = {
            let book = self.book.lock().await;
            if book.len() != 1 {
                return false;
            }
            match book.primary() {
                Some(p) => p,
                None => return false,
            }
        };

        // 1. Autenticidad. No toca ningún estado: un paquete falso no deja
        //    rastro.
        let sid = self.session_id.load(Ordering::Acquire);
        let mut tmp = view.payload().to_vec();
        let len = tmp.len();
        let nonce = make_nonce(header.sequence, sid);
        let authentic = {
            let key = self.decrypt_key.lock().await;
            let verdict = key.as_ref().map(|key| {
                decrypt_in_place(key, &nonce, &raw_buf[..HEADER_SIZE], &mut tmp, len).is_ok()
            });
            drop(key);
            verdict.unwrap_or(false)
        };
        if !authentic {
            return false;
        }

        // 2. Frescura, sobre una copia: quien marca la secuencia es el flujo
        //    normal, que procesa este mismo paquete a continuación.
        let fresh = {
            let windows = self.replay_by_peer.lock().await;
            windows
                .get(&old)
                .is_none_or(|w| w.clone().check_and_mark(header.sequence))
        };
        if !fresh {
            return false;
        }

        self.switch_peer(old, from).await;
        true
    }

    /// Envía un mensaje de encuentro. Un fallo de envío no es fatal: es UDP y
    /// se repite en el siguiente ciclo.
    async fn send_rv(&self, msg: &RvMessage, to: SocketAddr) {
        let bytes = msg.encode(self.room_key());
        if let Err(e) = self.transport.send_to(&bytes, to).await {
            tracing::trace!(?to, %e, "mensaje de encuentro no enviado");
        }
    }

    /// Atiende un mensaje de encuentro recibido mientras se espera invitado.
    ///
    /// - `PEERS` del servidor: alguien más está en la sala. Se le manda una
    ///   sonda, que es lo que abre este NAT para lo que él envíe.
    /// - `PUNCH` de un extremo: se confirma, para que sepa que el camino
    ///   funciona en los dos sentidos.
    ///
    /// Todo lo que no lleve la clave de esta sala se ignora: no se contesta a
    /// quien no ha demostrado conocerla.
    async fn on_rendezvous(&self, datagram: &[u8], from: SocketAddr) {
        let Some((key, msg)) = RvMessage::decode(datagram) else {
            return;
        };
        if key != self.room_key() {
            return;
        }
        match msg {
            RvMessage::Peers { others, .. } if Some(from) == self.rendezvous_server() => {
                for peer in &others {
                    let nonce = rendezvous::random_nonce();
                    self.send_rv(&RvMessage::Punch { nonce }, *peer).await;
                }
                // Quedan como pistas: mientras se espera, el lazo las sigue
                // sondando (y barre sus puertos vecinos).
                self.add_path_hints(&others);
            }
            RvMessage::Punch { nonce } => {
                self.send_rv(&RvMessage::PunchAck { nonce }, from).await;
            }
            // El peer anuncia por dónde se le puede alcanzar. Sólo se acepta
            // de quien ya es el peer: es información para recuperar el camino
            // con él, no una forma de que un tercero redirija las sondas.
            RvMessage::Addrs { addrs } => {
                if self.book.lock().await.contains(from) {
                    self.add_path_hints(&addrs);
                }
            }
            // Respuesta a una sonda de recuperación: por `from` sí se llega al
            // otro extremo. Se le pasa a hablar ahí y se le manda un latido,
            // que es con lo que él comprueba que somos nosotros y actualiza
            // nuestra dirección.
            RvMessage::PunchAck { nonce } if nonce == self.probe_nonce() => {
                let current = self.book.lock().await.primary();
                let stale = self.rx_idle_ms().is_some_and(|ms| ms >= PATH_STALE_MS);
                if let Some(old) = current.filter(|old| *old != from && stale) {
                    if self.book.lock().await.len() == 1 {
                        self.switch_peer(old, from).await;
                        self.send_keepalive().await;
                    }
                }
            }
            _ => {}
        }
    }

    /// Elige, entre varias direcciones candidatas del anfitrión, una por la
    /// que de verdad se llega.
    ///
    /// Sonda todas a la vez y devuelve la primera que confirma. Si hay
    /// servidor de encuentro, además se registra en él: así el anfitrión
    /// aprende la dirección de este extremo (y abre su NAT hacia ella) y este
    /// extremo aprende la del anfitrión aunque no viniera en la invitación.
    ///
    /// Devuelve `Timeout` si nada contesta en `wait`.
    pub async fn select_path(
        &self,
        candidates: &[SocketAddr],
        wait: Duration,
    ) -> Result<SocketAddr, TransportError> {
        let server = self.rendezvous_server();
        let key = self.room_key();
        let nonce = rendezvous::random_nonce();
        let mut candidates: Vec<SocketAddr> = candidates.to_vec();
        // Direcciones públicas alrededor de las que barrer puertos: las de la
        // invitación y las que dé el servidor, no las que se aprenden de una
        // sonda recibida (esas ya son exactas).
        let mut sweep_targets: Vec<SocketAddr> = candidates
            .iter()
            .copied()
            .filter(|c| !rendezvous::is_local(c))
            .collect();
        let mut cycle: u32 = 0;
        let deadline = Instant::now() + wait;
        let tick = Duration::from_millis(PUNCH_INTERVAL_MS);
        let mut next_send = Instant::now();
        // Camino público que ya contestó, y hasta cuándo se espera a uno local.
        let mut fallback: Option<(SocketAddr, Instant)> = None;
        let mut buf = vec![0u8; self.config.mtu];

        loop {
            if self.closed.load(Ordering::Acquire) {
                return Err(TransportError::PeerClosed("session closed"));
            }
            let now = Instant::now();
            if let Some((addr, until)) = fallback {
                if now >= until {
                    return Ok(addr);
                }
            }
            if now >= deadline {
                return fallback
                    .map(|(addr, _)| addr)
                    .ok_or(TransportError::Timeout);
            }
            if now >= next_send {
                if let Some(server) = server {
                    self.send_rv(&RvMessage::Register, server).await;
                }
                for &candidate in &candidates {
                    self.send_rv(&RvMessage::Punch { nonce }, candidate).await;
                }
                // Si el anfitrión está tras un NAT simétrico, hacia aquí
                // saldrá por un puerto vecino del que se conoce. Sondar esos
                // vecinos abre este NAT para cuando su sonda llegue.
                if cycle.is_multiple_of(PUNCH_SPREAD_EVERY) {
                    for &target in &sweep_targets {
                        self.punch_neighbours(target, nonce).await;
                    }
                }
                cycle = cycle.wrapping_add(1);
                next_send = now + tick;
            }

            let mut wake = next_send.min(deadline);
            if let Some((_, until)) = fallback {
                wake = wake.min(until);
            }
            let received = timeout(
                wake.saturating_duration_since(now),
                self.transport.recv(&mut buf),
            )
            .await;
            let (n, from) = match received {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => return Err(e),
                Err(_) => continue,
            };
            let Some((got_key, msg)) = RvMessage::decode(&buf[..n]) else {
                continue;
            };
            if got_key != key {
                continue;
            }
            match msg {
                RvMessage::Peers { others, .. } if Some(from) == server => {
                    for peer in others {
                        if !candidates.contains(&peer) {
                            tracing::debug!(?peer, "candidato aprendido del servidor de encuentro");
                            candidates.push(peer);
                            if !rendezvous::is_local(&peer) {
                                sweep_targets.push(peer);
                            }
                            // Sin esperar al siguiente ciclo: cada sonda que
                            // sale antes es conexión que llega antes.
                            self.send_rv(&RvMessage::Punch { nonce }, peer).await;
                        }
                    }
                }
                RvMessage::PunchAck { nonce: echoed } if echoed == nonce => {
                    if rendezvous::is_local(&from) {
                        return Ok(from);
                    }
                    if fallback.is_none() {
                        let grace = Duration::from_millis(LOCAL_PATH_GRACE_MS);
                        fallback = Some((from, Instant::now() + grace));
                    }
                }
                // El anfitrión sondeando desde una dirección que no estaba en
                // la lista: pasa cuando su NAT es simétrico y usa hacia aquí un
                // puerto distinto del que vio el servidor. Esa dirección es la
                // buena, así que se añade y se sonda.
                RvMessage::Punch { nonce: theirs } => {
                    self.send_rv(&RvMessage::PunchAck { nonce: theirs }, from)
                        .await;
                    if !candidates.contains(&from) {
                        candidates.push(from);
                        self.send_rv(&RvMessage::Punch { nonce }, from).await;
                    }
                }
                _ => {}
            }
        }
    }

    /// Handshake de cliente contra un anfitrión con varias direcciones
    /// posibles (red local, pública…).
    ///
    /// Primero elige el camino con [`Self::select_path`] y luego hace el
    /// handshake normal por él. Si ninguna sonda vuelve, lo intenta igualmente
    /// contra la primera candidata: un anfitrión con una versión anterior no
    /// contesta sondas pero sí acepta el handshake.
    ///
    /// `candidates` puede ir vacía si hay servidor de encuentro: la dirección
    /// del anfitrión llega entonces por él.
    pub async fn handshake_any(&self, candidates: &[SocketAddr]) -> Result<(), TransportError> {
        // Sin candidatas sólo hay algo que hacer si el servidor de encuentro
        // puede aportar alguna.
        self.handshake_any_within(candidates, Duration::from_millis(PATH_SELECT_TIMEOUT_MS))
            .await
    }

    /// Como [`Self::handshake_any`], esperando hasta `wait` a que algún camino
    /// directo conteste.
    ///
    /// Un plazo largo sirve cuando la dirección de este extremo le llega al
    /// anfitrión a mano (escaneando la respuesta a la invitación): hay que
    /// seguir sondando mientras tanto.
    ///
    /// Si ningún camino directo contesta y hay relay, el audio va por él: es
    /// lo que hace que dos extremos tras NAT simétrico (dos móviles en datos)
    /// puedan hablar. El cifrado sigue siendo de extremo a extremo; el relay
    /// reenvía paquetes que no puede leer.
    pub async fn handshake_any_within(
        &self,
        candidates: &[SocketAddr],
        wait: Duration,
    ) -> Result<(), TransportError> {
        let relay = self.arm_relay();
        if candidates.is_empty() && relay.is_none() {
            return Err(TransportError::InvalidState("no candidates"));
        }
        if let Some(relay) = relay.filter(|_| self.relay_only.load(Ordering::Acquire)) {
            return self.handshake(SessionRole::Client, relay).await;
        }
        let peer = match self.select_path(candidates, wait).await {
            Ok(peer) => peer,
            Err(TransportError::Timeout) => match relay {
                Some(relay) => {
                    tracing::debug!("sin camino directo: el audio irá por el relay");
                    relay
                }
                None => {
                    tracing::debug!("ningún camino contestó a las sondas; se prueba el primero");
                    *candidates.first().ok_or(TransportError::Timeout)?
                }
            },
            Err(e) => return Err(e),
        };
        tracing::debug!(?peer, "camino elegido");
        // Las candidatas se guardan: si este camino cae (cambio de red de
        // cualquiera de los dos), son por donde se vuelve a buscar.
        self.add_path_hints(candidates);
        self.handshake(SessionRole::Client, peer).await
    }

    /// `true` si hay token de sala configurado.
    #[must_use]
    pub fn has_room_token(&self) -> bool {
        self.room_token
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some()
    }

    /// PSK derivada del token de sala (SHA-256), si hay token.
    #[must_use]
    pub fn room_psk(&self) -> Option<[u8; 32]> {
        use sha2::Digest as _;
        let guard = self.room_token.lock().unwrap_or_else(|p| p.into_inner());
        guard.as_ref().map(|token| {
            let digest = Sha256::digest(token);
            let mut key = [0u8; 32];
            key.copy_from_slice(&digest);
            key
        })
    }

    /// Resuelve el `session_id` a usar: el pre-fijado o uno aleatorio.
    fn resolved_session_id(&self) -> u32 {
        match self.preset_session_id() {
            0 => rand_u32_secure(),
            id => id,
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
    pub const fn config(&self) -> &Config {
        &self.config
    }

    // ── PTT (Push-to-Talk) ──────────────────────────────────────────────────

    /// Activa PTT: marca el flag local y envía FloorRequest + ControlResume al peer.
    ///
    /// FloorRequest es la señal de floor control (árbitro/relay la maneja).
    /// ControlResume es el indicador inmediato para el peer (mostrar "alguien habla").
    pub async fn ptt_press(&self) -> Result<(), TransportError> {
        self.ptt_active.store(true, Ordering::Release);
        let targets = self.broadcast_targets().await;
        if !targets.is_empty() {
            let sid = self.session_id();
            // SSRC local = session_id como proxy (sin SSRC dedicado aún)
            let mut ssrc_buf = [0u8; 4];
            ssrc_buf.copy_from_slice(&sid.to_be_bytes());
            for p in targets {
                self.send_control(MessageType::FloorRequest, sid, &ssrc_buf, p)
                    .await?;
                self.send_control(MessageType::ControlResume, sid, &[], p)
                    .await?;
            }
        }
        Ok(())
    }

    /// Destinos de un envío de control o audio: todos los peers conocidos.
    ///
    /// En P2P devuelve el único peer, así que el comportamiento 1:1 no cambia.
    /// En sala devuelve todos los participantes, que es lo que permite N-a-N.
    async fn broadcast_targets(&self) -> Vec<SocketAddr> {
        self.book.lock().await.all()
    }

    /// Desactiva PTT: limpia el flag local y envía FloorRelease + ControlPause.
    pub async fn ptt_release(&self) -> Result<(), TransportError> {
        self.ptt_active.store(false, Ordering::Release);
        let targets = self.broadcast_targets().await;
        if !targets.is_empty() {
            let sid = self.session_id();
            let mut ssrc_buf = [0u8; 4];
            ssrc_buf.copy_from_slice(&sid.to_be_bytes());
            for p in targets {
                self.send_control(MessageType::FloorRelease, sid, &ssrc_buf, p)
                    .await?;
                self.send_control(MessageType::ControlPause, sid, &[], p)
                    .await?;
            }
        }
        Ok(())
    }

    /// Devuelve `true` si PTT está actualmente presionado por el usuario local.
    #[must_use]
    pub fn is_ptt_active(&self) -> bool {
        self.ptt_active.load(Ordering::Acquire)
    }

    /// Devuelve `true` si el peer remoto está actualmente transmitiendo.
    ///
    /// Se actualiza cuando se reciben mensajes `ControlResume` (inicio de
    /// transmisión del peer) y `ControlPause` (fin de transmisión).
    #[must_use]
    pub fn is_peer_ptt_active(&self) -> bool {
        self.peer_ptt_active.load(Ordering::Acquire)
    }

    /// SSRC local (disponible después del handshake).
    #[must_use]
    pub fn local_ssrc(&self) -> u32 {
        self.local_ssrc.load(Ordering::Acquire)
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

    /// Dirección local del transporte subyacente.
    pub fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        self.transport.local_addr()
    }

    /// Número de peers conocidos de la sesión.
    ///
    /// Siempre 1 en P2P; en sala crece con cada handshake válido.
    pub async fn peer_count(&self) -> usize {
        self.book.lock().await.len()
    }

    /// Direcciones de los peers conocidos, en orden de registro.
    pub async fn peers(&self) -> Vec<SocketAddr> {
        self.book.lock().await.all()
    }

    /// Ejecuta el handshake criptográfico 4-way.
    pub async fn handshake(
        &self,
        role: SessionRole,
        peer: SocketAddr,
    ) -> Result<(), TransportError> {
        self.book.lock().await.insert(peer);

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
        let result: Result<Result<(), TransportError>, tokio::time::error::Elapsed> = match role {
            SessionRole::Client => Ok(self.run_client_handshake(peer, deadline).await),
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

    /// Decide el handshake de cliente según `handshake_mode`.
    ///
    /// En `Auto` se intenta Noise primero y, si el peer no responde (peer
    /// legacy), se cae al handshake v1 — **salvo** que haya token de sala,
    /// que exige Noise para autenticar.
    async fn run_client_handshake(
        &self,
        peer: SocketAddr,
        deadline: Duration,
    ) -> Result<(), TransportError> {
        let expired = |_| TransportError::Timeout;

        match self.handshake_mode() {
            HandshakeMode::Legacy => timeout(deadline, self.handshake_client(peer))
                .await
                .map_err(expired)?,
            HandshakeMode::Noise => timeout(deadline, self.handshake_noise_client(peer))
                .await
                .map_err(expired)?,
            HandshakeMode::Auto => {
                let token_set = self.has_room_token();

                // Sin la feature `noise` el handshake Noise no existe. El
                // servidor ya lo refleja con `allow_noise = cfg!(feature =
                // "noise")`, así que aquí se va directo a v1 — salvo con token
                // de sala, que exige Noise y no puede degradarse a legacy.
                if !cfg!(feature = "noise") {
                    if token_set {
                        return Err(TransportError::Handshake(
                            "room token requires the `noise` feature",
                        ));
                    }
                    tracing::debug!("build sin `noise`: handshake v1 directo");
                    return timeout(deadline, self.handshake_client(peer))
                        .await
                        .map_err(expired)?;
                }

                let noise_res = timeout(
                    Duration::from_millis(NOISE_FALLBACK_MS),
                    self.handshake_noise_client(peer),
                )
                .await;

                match noise_res {
                    Ok(Ok(())) => Ok(()),
                    Ok(Err(TransportError::Timeout)) if !token_set => {
                        tracing::debug!("peer sin Noise: fallback a handshake v1");
                        timeout(deadline, self.handshake_client(peer))
                            .await
                            .map_err(expired)?
                    }
                    Ok(Err(e)) => Err(e),
                    Err(_) if !token_set => {
                        tracing::debug!("Noise sin respuesta: fallback a handshake v1");
                        timeout(deadline, self.handshake_client(peer))
                            .await
                            .map_err(expired)?
                    }
                    Err(_) => Err(TransportError::Timeout),
                }
            }
        }
    }

    /// Handshake servidor que acepta el primer cliente que llegue desde
    /// cualquier dirección (modo QR pairing — el servidor no conoce la IP
    /// del cliente de antemano).
    ///
    /// El peer queda fijado automáticamente tras recibir el primer
    /// `ClientHello` válido, igual que en `handshake()` para rol `Server`,
    /// salvo que no se filtra por IP de origen.
    pub async fn handshake_open(&self) -> Result<(), TransportError> {
        self.arm_relay();
        self.state
            .lock()
            .await
            .transition(SessionEvent::StartAccept)
            .map_err(|_| TransportError::InvalidState("cannot start open handshake"))?;

        self.accept_loop().await
    }

    /// Acepta un peer nuevo en una sesión ya establecida (modo sala).
    ///
    /// Registro continuo de peers adicionales mientras la sesión esté activa.
    ///
    /// Debe invocarse desde la misma tarea que consume el socket de audio, NO
    /// en paralelo: dos tareas leyendo el mismo `Transport` se reparten los
    /// datagramas y se roban audio. Por eso el trabajo real ocurre en
    /// `recv_audio`, que detecta un handshake entrante de una dirección nueva y
    /// promueve al peer allí mismo. Este método queda como reintento perezoso
    /// para cuando no hay tráfico de audio que dispare la detección.
    ///
    /// Sin tráfico no hay peert yet: un participante nuevo siempre manda su
    /// `ClientHello`, y ese paquete pasa por `recv_audio`.
    pub async fn accept_additional_peer(&self) -> Result<bool, TransportError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(TransportError::PeerClosed("session closed"));
        }
        let st = self.state.lock().await.state();
        if st != SessionState::Active {
            return Err(TransportError::InvalidState("session not active"));
        }

        let allow_noise = cfg!(feature = "noise")
            && matches!(
                self.handshake_mode(),
                HandshakeMode::Auto | HandshakeMode::Noise
            );
        let allow_legacy = !matches!(self.handshake_mode(), HandshakeMode::Noise);
        let (first, peer) = match self
            .wait_first_handshake_packet(None, allow_noise, allow_legacy)
            .await
        {
            Ok(v) => v,
            Err(TransportError::PeerClosed(_)) => {
                return Err(TransportError::PeerClosed("session closed"))
            }
            Err(_) => return Ok(false),
        };

        match first {
            FirstHandshake::Noise(msg1) => {
                self.handshake_noise_server_after_hello1(peer, msg1).await?;
            }
            FirstHandshake::Legacy(hello) => {
                self.handshake_server_after_hello(peer, hello).await?;
            }
        }
        Ok(true)
    }

    /// Detecta si un datagrama entrante es el inicio del handshake de un peer
    /// nuevo (modo sala).
    ///
    /// Sólo tiene sentido con la sesión ya activa y desde una dirección que no
    /// está en la agenda: en P2P no hay peers nuevos, y en sala un `ClientHello`
    /// de un conocido es un reenvío del relay, no una petición de entrada.
    fn peek_new_peer(&self, view: &PacketView<'_>, from: SocketAddr) -> Option<FirstHandshake> {
        if self.closed.load(Ordering::Acquire) || !self.multi_peer.load(Ordering::Acquire) {
            return None;
        }
        let code = view.header().msg_type;
        if code != MessageType::HandshakeClientHello.code()
            && code != MessageType::HandshakeNoiseHello1.code()
        {
            return None;
        }

        // La dirección de origen no viaja en el header: la aporta el recv.
        let known = {
            let book = self.book.try_lock();
            match book {
                Ok(b) => b.contains(from),
                Err(_) => return None,
            }
        };
        if known {
            return None;
        }
        // Sólo en sala: en P2P el handshake ya ocurrió y no hay más peers.
        // `session_id` distinto de 0 indica modo sala (relay).
        if self.session_id.load(Ordering::Acquire) == 0 {
            return None;
        }

        if code == MessageType::HandshakeNoiseHello1.code() {
            return Some(FirstHandshake::Noise(view.payload().to_vec()));
        }
        match view
            .payload()
            .get(..4)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_be_bytes)
        {
            Some(0) => Some(FirstHandshake::Legacy(
                ClientHello::decode(view.payload()).ok()?,
            )),
            _ => None,
        }
    }

    /// Promueve a un peer ya detectado dentro del lazo de recepción.
    ///
    /// `first` y `peer` provienen del datagrama que `recv_audio` ya consumió, de
    /// modo que no hay lectura adicional aquí: el resto del handshake sí consume
    /// del socket, pero desde la misma tarea propietaria.
    async fn promote_peer(
        &self,
        first: FirstHandshake,
        peer: SocketAddr,
    ) -> Result<(), TransportError> {
        self.book.lock().await.insert(peer);
        let result = match first {
            FirstHandshake::Noise(msg1) => {
                self.handshake_noise_server_after_hello1(peer, msg1).await
            }
            FirstHandshake::Legacy(hello) => self.handshake_server_after_hello(peer, hello).await,
        };
        match result {
            Ok(()) => {
                let n = self.book.lock().await.len();
                tracing::info!(peers = n, ?peer, "peer promovido en la sala");
                Ok(())
            }
            Err(e) => {
                // El peer no llegó a completar: sacarlo de la agenda para no
                // enviarle audio que no puede descifrar.
                self.book.lock().await.remove(peer);
                Err(e)
            }
        }
    }

    /// Loop de reintento del handshake de servidor abierto.
    async fn accept_loop(&self) -> Result<(), TransportError> {
        // Loop de reintento: acepta hasta completar o que close() sea llamado.
        // Cada intento tiene 30 s de timeout; si falla se reinicia el handshake
        // (state: Closed → Reconnecting → Handshaking) para que un segundo
        // dispositivo pueda conectarse si el primero falló a mitad.
        loop {
            if self.closed.load(Ordering::Acquire) {
                let _ = self.state.lock().await.transition(SessionEvent::Close);
                return Err(TransportError::PeerClosed("session closed"));
            }

            let deadline = Duration::from_millis(30_000);
            let result = timeout(deadline, self.handshake_server_any()).await;

            match result {
                Ok(Ok(())) => {
                    self.state
                        .lock()
                        .await
                        .transition(SessionEvent::HandshakeOk)
                        .map_err(|_| TransportError::InvalidState("handshake_ok"))?;
                    return Ok(());
                }
                Ok(Err(TransportError::PeerClosed(_))) => {
                    // close() fue llamado — salir limpiamente.
                    let _ = self.state.lock().await.transition(SessionEvent::Close);
                    return Err(TransportError::PeerClosed("session closed"));
                }
                Ok(Err(_)) | Err(_) => {
                    // Error de red o timeout de 30 s: reintentar.
                    // Transicionar: Handshaking → Closed → Reconnecting → Handshaking
                    let mut sm = self.state.lock().await;
                    let _ = sm.transition(SessionEvent::HandshakeTimeout);
                    let _ = sm.transition(SessionEvent::Reconnect);
                    let _ = sm.transition(SessionEvent::StartAccept);
                    drop(sm);
                    self.book.lock().await.clear();
                }
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
        hello
            .encode(&mut hello_payload)
            .map_err(TransportError::Protocol)?;

        // Reintento con backoff hasta el timeout del caller.
        let mut attempt: u32 = 0;
        let mut buf = vec![0u8; self.config.mtu];

        // Modo relay/sala: el `session_id` pre-fijado viaja en `ClientHello`
        // para que el relay sepa enrutarlo. P2P directo: 0 (el servidor elige).
        let preset_sid = self.preset_session_id();
        loop {
            self.send_control(
                MessageType::HandshakeClientHello,
                preset_sid,
                &hello_payload,
                peer,
            )
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
                let server_hello =
                    ServerHello::decode(view.payload()).map_err(TransportError::Protocol)?;

                // Si el id estaba pre-fijado (sala), el servidor debe respetarlo.
                if preset_sid != 0 && server_hello.session_id != preset_sid {
                    return Err(TransportError::Handshake(
                        "server replied with a different session_id than preset",
                    ));
                }

                // Validación de versión negociada: el servidor sólo puede
                // hacer downgrade (nunca proponer una versión más alta que la
                // que pedimos) y no puede proponer algo fuera del rango.
                let neg_ver = server_hello.protocol_version;
                if neg_ver < PROTOCOL_VERSION_MIN || neg_ver > PROTOCOL_VERSION_MAX {
                    return Err(TransportError::Handshake(
                        "version negotiation failed: out of range",
                    ));
                }
                if !self
                    .config
                    .supported_codecs
                    .contains(&server_hello.codec_accepted)
                {
                    return Err(TransportError::Handshake(
                        "server selected unsupported codec",
                    ));
                }

                // 3. ECDH + HKDF → encrypt_key, decrypt_key.
                let server_pubkey = PublicKey::from(server_hello.ephemeral_public_key);
                let shared = client_secret.diffie_hellman(&server_pubkey);
                let session_id = server_hello.session_id;

                let transcript =
                    build_transcript(&client_nonce, &server_hello.server_nonce, session_id);
                let (enc_key, dec_key) = derive_session_keys(shared.as_bytes(), &transcript);

                // 4. Calcular auth_tag del cliente y enviar KeyExchange.
                let client_auth_tag = derive_auth_tag(&enc_key, b"GS-client-fin-v1", &transcript);
                let ke_msg = KeyExchangeMsg {
                    session_id,
                    auth_tag: client_auth_tag,
                };
                let mut ke_payload = [0u8; KeyExchangeMsg::SIZE];
                ke_msg
                    .encode(&mut ke_payload)
                    .map_err(TransportError::Protocol)?;
                self.send_control(
                    MessageType::HandshakeKeyExchange,
                    session_id,
                    &ke_payload,
                    peer,
                )
                .await?;

                // 5. Esperar SessionConfirm del servidor.
                let confirm = self
                    .recv_session_confirm(peer, &mut buf, session_id)
                    .await?;

                // 6. Verificar auth_tag del servidor.
                let expected_server_tag =
                    derive_auth_tag(&dec_key, b"GS-server-fin-v1", &transcript);
                if !constant_time_eq(&confirm.server_auth_tag, &expected_server_tag) {
                    return Err(TransportError::AuthenticationFailed(
                        "server auth tag mismatch",
                    ));
                }

                // 7. Almacenar estado de sesión.
                self.session_id.store(session_id, Ordering::Release);
                // SSRC local = los primeros 4 bytes del session_id XOR con
                // los últimos 4 de la clave de cifrado (distingue cliente/servidor).
                let ssrc = session_id
                    ^ u32::from_be_bytes([enc_key[0], enc_key[1], enc_key[2], enc_key[3]]);
                self.local_ssrc.store(ssrc, Ordering::Release);
                self.negotiated_codec
                    .store(server_hello.codec_accepted, Ordering::Release);
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
        buf: &mut [u8],
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
            let confirm =
                SessionConfirm::decode(view.payload()).map_err(TransportError::Protocol)?;
            if confirm.session_id != expected_sid {
                return Err(TransportError::Handshake("session_id mismatch in confirm"));
            }
            return Ok(confirm);
        }
    }

    // ── Handshake servidor ──────────────────────────────────────────────────

    async fn handshake_server(&self, peer: SocketAddr) -> Result<(), TransportError> {
        // Modo relay/sala: registrarse ante el relay antes de esperar el
        // primer mensaje. El relay aprende la dirección del servidor con el
        // primer paquete que recibe; sin este registro, el ClientHello del
        // cliente no tendría destino al que reenviarse.
        let preset_sid = self.preset_session_id();
        if preset_sid != 0 {
            self.send_control(MessageType::Heartbeat, preset_sid, &[], peer)
                .await?;
        }

        let allow_noise = cfg!(feature = "noise")
            && matches!(
                self.handshake_mode(),
                HandshakeMode::Auto | HandshakeMode::Noise
            );
        let allow_legacy = !matches!(self.handshake_mode(), HandshakeMode::Noise);
        let (first, from) = self
            .wait_first_handshake_packet(Some(peer), allow_noise, allow_legacy)
            .await?;
        match first {
            FirstHandshake::Noise(msg1) => {
                self.handshake_noise_server_after_hello1(from, msg1).await
            }
            FirstHandshake::Legacy(hello) => self.handshake_server_after_hello(from, hello).await,
        }
    }

    /// Cuerpo del handshake servidor legacy (tras recibir el `ClientHello`).
    async fn handshake_server_after_hello(
        &self,
        peer: SocketAddr,
        client_hello: ClientHello,
    ) -> Result<(), TransportError> {
        let mut buf = vec![0u8; self.config.mtu];
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
        let session_id = self.resolved_session_id();
        self.session_id.store(session_id, Ordering::Release);

        // 3. Negociar codec.
        let chosen_codec = if self
            .config
            .supported_codecs
            .contains(&client_hello.codec_preferred)
        {
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
        server_hello
            .encode(&mut sh_payload)
            .map_err(TransportError::Protocol)?;
        self.send_control(
            MessageType::HandshakeServerHello,
            session_id,
            &sh_payload,
            peer,
        )
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
        // Si llega un ClientHello repetido (nuestro ServerHello se perdió),
        // reenviar ServerHello para que el cliente pueda avanzar.
        let ke_msg: KeyExchangeMsg = loop {
            if self.closed.load(Ordering::Acquire) {
                return Err(TransportError::PeerClosed("session closed"));
            }
            let recv_res = timeout(Duration::from_millis(500), self.transport.recv(&mut buf)).await;
            let (n, from) = match recv_res {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => return Err(e),
                Err(_) => continue, // timeout de 500ms, volver a chequear closed
            };
            if from != peer {
                continue;
            }
            let view = match PacketView::decode(&buf[..n]) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if view.header().msg_type == MessageType::HandshakeClientHello.code() {
                self.send_control(
                    MessageType::HandshakeServerHello,
                    session_id,
                    &sh_payload,
                    peer,
                )
                .await?;
                continue;
            }
            if view.header().msg_type != MessageType::HandshakeKeyExchange.code() {
                continue;
            }
            match KeyExchangeMsg::decode(view.payload()) {
                Ok(m) => break m,
                Err(e) => return Err(TransportError::Protocol(e)),
            }
        };

        if ke_msg.session_id != session_id {
            return Err(TransportError::Handshake(
                "session_id mismatch in KeyExchange",
            ));
        }

        // 7. Verificar auth_tag del cliente.
        // El cliente usó su encrypt_key (= dec_key del servidor) para derivar el tag.
        let expected_client_tag = derive_auth_tag(&dec_key, b"GS-client-fin-v1", &transcript);
        if !constant_time_eq(&ke_msg.auth_tag, &expected_client_tag) {
            return Err(TransportError::AuthenticationFailed(
                "client auth tag mismatch",
            ));
        }

        // 8. Enviar SessionConfirm con auth_tag del servidor.
        let server_auth_tag = derive_auth_tag(&enc_key, b"GS-server-fin-v1", &transcript);
        let confirm = SessionConfirm {
            session_id,
            server_auth_tag,
        };
        let mut sc_payload = [0u8; SessionConfirm::SIZE];
        confirm
            .encode(&mut sc_payload)
            .map_err(TransportError::Protocol)?;
        self.send_control(
            MessageType::HandshakeSessionConfirm,
            session_id,
            &sc_payload,
            peer,
        )
        .await?;

        // 9. Almacenar claves.
        *self.encrypt_key.lock().await = Some(enc_key);
        *self.decrypt_key.lock().await = Some(dec_key);
        Ok(())
    }

    /// Espera el primer mensaje de handshake válido (Noise o `ClientHello`).
    ///
    /// `expect_peer = None` acepta cualquier origen (modo QR/abierto).
    async fn wait_first_handshake_packet(
        &self,
        expect_peer: Option<SocketAddr>,
        allow_noise: bool,
        allow_legacy: bool,
    ) -> Result<(FirstHandshake, SocketAddr), TransportError> {
        let mut buf = vec![0u8; self.config.mtu];
        // El encuentro sólo aplica cuando se espera a cualquiera: con un peer
        // fijado ya se sabe a dónde hablar.
        let open = expect_peer.is_none();
        let tick = Duration::from_millis(PUNCH_INTERVAL_MS);
        let mut next_register = Instant::now();
        let mut cycle: u32 = 0;
        loop {
            if self.closed.load(Ordering::Acquire) {
                return Err(TransportError::PeerClosed("session closed"));
            }
            // Registrarse mientras se espera: es lo que deja que el invitado
            // encuentre esta dirección y lo que trae la suya de vuelta.
            let wait = if open {
                let now = Instant::now();
                if now >= next_register {
                    if let Some(server) = self.rendezvous_server() {
                        self.send_rv(&RvMessage::Register, server).await;
                        // Además de presentarse, se da de alta en la ruta del
                        // relay: si el invitado no encuentra camino directo,
                        // su handshake llegará por ahí.
                        if cycle.is_multiple_of(PUNCH_SPREAD_EVERY) {
                            let sid = self.preset_session_id();
                            let _ = self
                                .send_control(MessageType::Heartbeat, sid, &[], server)
                                .await;
                        }
                    }
                    // Direcciones del invitado que llegaron por otro medio:
                    // sondarlas abre este NAT hacia ellas.
                    let nonce = rendezvous::random_nonce();
                    let sweep = cycle.is_multiple_of(PUNCH_SPREAD_EVERY);
                    cycle = cycle.wrapping_add(1);
                    for hint in self.path_hints() {
                        self.send_rv(&RvMessage::Punch { nonce }, hint).await;
                        // El barrido sólo tiene sentido hacia internet: en una
                        // red local no hay NAT que cambie el puerto.
                        if sweep && !rendezvous::is_local(&hint) {
                            self.punch_neighbours(hint, nonce).await;
                        }
                    }
                    next_register = now + tick;
                }
                next_register
                    .saturating_duration_since(now)
                    .max(Duration::from_millis(1))
            } else {
                Duration::from_millis(500)
            };
            let recv_res = timeout(wait, self.transport.recv(&mut buf)).await;
            let (n, from) = match recv_res {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => return Err(e),
                Err(_) => continue,
            };
            if rendezvous::is_rendezvous(&buf[..n]) {
                if open {
                    self.on_rendezvous(&buf[..n], from).await;
                }
                continue;
            }
            if let Some(expected) = expect_peer {
                if from != expected {
                    tracing::debug!(?from, ?expected, "dropping datagram from wrong peer");
                    continue;
                }
            }
            let view = match PacketView::decode(&buf[..n]) {
                Ok(v) => v,
                Err(_) => continue,
            };
            let msg_type = view.header().msg_type;
            if allow_noise && msg_type == MessageType::HandshakeNoiseHello1.code() {
                return Ok((FirstHandshake::Noise(view.payload().to_vec()), from));
            }
            if allow_legacy && msg_type == MessageType::HandshakeClientHello.code() {
                if let Ok(h) = ClientHello::decode(view.payload()) {
                    return Ok((FirstHandshake::Legacy(h), from));
                }
            }
        }
    }

    /// Handshake Noise como cliente (feature `noise`).
    #[cfg(feature = "noise")]
    async fn handshake_noise_client(&self, peer: SocketAddr) -> Result<(), TransportError> {
        let psk = self.room_psk();
        let mut hs = NoiseHandshake::new_initiator(psk.as_ref())
            .map_err(|_| TransportError::Handshake("noise initiator init failed"))?;

        let client_nonce = random_nonce_32();
        let hello1 = NoiseHello1 {
            protocol_version: PROTOCOL_VERSION_MAX,
            codec_preferred: self.config.codec_preferred,
            sample_rate: self.config.sample_rate,
            channels: self.config.channels,
            frame_duration_ms: self.config.frame_duration_ms,
            max_bitrate: self.config.max_bitrate,
            capability_flags: self.config.capability_flags,
            client_nonce,
        };
        let mut p1 = [0u8; NoiseHello1::SIZE];
        hello1.encode(&mut p1).map_err(TransportError::Protocol)?;
        let msg1 = hs
            .write(&p1)
            .map_err(|_| TransportError::Handshake("noise write msg1 failed"))?;

        let preset_sid = self.preset_session_id();
        let mut buf = vec![0u8; self.config.mtu];
        let mut attempt: u32 = 0;
        let hello2: NoiseHello2 = loop {
            if self.closed.load(Ordering::Acquire) {
                return Err(TransportError::PeerClosed("session closed"));
            }
            if attempt > 8 {
                return Err(TransportError::Timeout);
            }
            self.send_control(MessageType::HandshakeNoiseHello1, preset_sid, &msg1, peer)
                .await?;
            let backoff = Duration::from_millis(HANDSHAKE_RETRY_BASE_MS << attempt.min(4));
            match timeout(backoff, self.transport.recv(&mut buf)).await {
                Ok(Ok((n, from))) if from == peer => {
                    if let Ok(view) = PacketView::decode(&buf[..n]) {
                        if view.header().msg_type == MessageType::HandshakeNoiseHello2.code() {
                            let payload = hs
                                .read(view.payload())
                                .map_err(|_| TransportError::Handshake("noise msg2 invalid"))?;
                            if let Ok(h) = NoiseHello2::decode(&payload) {
                                break h;
                            }
                        }
                    }
                    attempt += 1;
                }
                Ok(Ok(_)) => attempt += 1,
                Ok(Err(e)) => return Err(e),
                Err(_) => attempt += 1,
            }
        };

        if hello2.protocol_version < PROTOCOL_VERSION_MIN {
            return Err(TransportError::Handshake("noise version too old"));
        }
        if !self
            .config
            .supported_codecs
            .contains(&hello2.codec_accepted)
        {
            return Err(TransportError::Handshake(
                "server selected unsupported codec",
            ));
        }
        let session_id = hello2.session_id;
        if preset_sid != 0 && session_id != preset_sid {
            return Err(TransportError::Handshake(
                "server replied with a different session_id than preset",
            ));
        }

        let hash = hs
            .handshake_hash()
            .map_err(|_| TransportError::Handshake("noise handshake incomplete"))?;
        let transcript = build_transcript(&client_nonce, &hello2.server_nonce, session_id);
        let (enc_key, dec_key) = derive_session_keys(&hash, &transcript);

        self.session_id.store(session_id, Ordering::Release);
        self.negotiated_codec
            .store(hello2.codec_accepted, Ordering::Release);
        let ssrc =
            session_id ^ u32::from_be_bytes([enc_key[0], enc_key[1], enc_key[2], enc_key[3]]);
        self.local_ssrc.store(ssrc, Ordering::Release);

        // Confirmación mutua con auth tags derivados de las claves Noise.
        let client_tag = derive_auth_tag(&enc_key, b"GS-client-fin-v2", &transcript);
        let ke = KeyExchangeMsg {
            session_id,
            auth_tag: client_tag,
        };
        let mut ke_payload = [0u8; KeyExchangeMsg::SIZE];
        ke.encode(&mut ke_payload)
            .map_err(TransportError::Protocol)?;
        self.send_control(
            MessageType::HandshakeKeyExchange,
            session_id,
            &ke_payload,
            peer,
        )
        .await?;

        let confirm = self
            .recv_session_confirm(peer, &mut buf, session_id)
            .await?;
        let expected = derive_auth_tag(&dec_key, b"GS-server-fin-v2", &transcript);
        if !constant_time_eq(&confirm.server_auth_tag, &expected) {
            return Err(TransportError::AuthenticationFailed(
                "server auth tag mismatch (noise)",
            ));
        }
        *self.encrypt_key.lock().await = Some(enc_key);
        *self.decrypt_key.lock().await = Some(dec_key);
        Ok(())
    }

    /// Stub cuando la feature `noise` no está compilada.
    #[cfg(not(feature = "noise"))]
    async fn handshake_noise_client(&self, _peer: SocketAddr) -> Result<(), TransportError> {
        Err(TransportError::Handshake(
            "compiled without `noise` feature",
        ))
    }

    /// Handshake Noise como servidor (tras leer `NoiseHello1`).
    #[cfg(feature = "noise")]
    async fn handshake_noise_server_after_hello1(
        &self,
        peer: SocketAddr,
        msg1: Vec<u8>,
    ) -> Result<(), TransportError> {
        let psk = self.room_psk();
        let mut hs = NoiseHandshake::new_responder(psk.as_ref())
            .map_err(|_| TransportError::Handshake("noise responder init failed"))?;
        let payload1 = hs
            .read(&msg1)
            .map_err(|_| TransportError::Handshake("noise msg1 invalid"))?;
        let hello1 = NoiseHello1::decode(&payload1).map_err(TransportError::Protocol)?;
        if hello1.protocol_version < PROTOCOL_VERSION_MIN {
            return Err(TransportError::Handshake("noise version too old"));
        }

        let session_id = self.resolved_session_id();
        self.session_id.store(session_id, Ordering::Release);
        let chosen_codec = if self
            .config
            .supported_codecs
            .contains(&hello1.codec_preferred)
        {
            hello1.codec_preferred
        } else {
            *self
                .config
                .supported_codecs
                .first()
                .ok_or(TransportError::Handshake("no supported codecs configured"))?
        };
        self.negotiated_codec.store(chosen_codec, Ordering::Release);

        let server_nonce = random_nonce_32();
        let hello2 = NoiseHello2 {
            session_id,
            protocol_version: hello1.protocol_version.min(PROTOCOL_VERSION_MAX),
            codec_accepted: chosen_codec,
            sample_rate: hello1.sample_rate,
            channels: hello1.channels,
            frame_duration_ms: hello1.frame_duration_ms,
            max_bitrate: hello1.max_bitrate.min(self.config.max_bitrate),
            capability_flags: hello1.capability_flags & self.config.capability_flags,
            server_nonce,
        };
        let mut p2 = [0u8; NoiseHello2::SIZE];
        hello2.encode(&mut p2).map_err(TransportError::Protocol)?;
        let msg2 = hs
            .write(&p2)
            .map_err(|_| TransportError::Handshake("noise write msg2 failed"))?;

        let mut buf = vec![0u8; self.config.mtu];
        let ke_msg: KeyExchangeMsg = loop {
            if self.closed.load(Ordering::Acquire) {
                return Err(TransportError::PeerClosed("session closed"));
            }
            let recv_res = timeout(Duration::from_millis(500), self.transport.recv(&mut buf)).await;
            let (n, from) = match recv_res {
                Ok(Ok(r)) => r,
                Ok(Err(e)) => return Err(e),
                Err(_) => continue,
            };
            if from != peer {
                continue;
            }
            let view = match PacketView::decode(&buf[..n]) {
                Ok(v) => v,
                Err(_) => continue,
            };
            if view.header().msg_type == MessageType::HandshakeNoiseHello1.code() {
                // msg2 se perdió: reenviar el mismo (el estado Noise ya avanzó).
                self.send_control(MessageType::HandshakeNoiseHello2, session_id, &msg2, peer)
                    .await?;
                continue;
            }
            if view.header().msg_type != MessageType::HandshakeKeyExchange.code() {
                continue;
            }
            match KeyExchangeMsg::decode(view.payload()) {
                Ok(m) => break m,
                Err(e) => return Err(TransportError::Protocol(e)),
            }
        };

        if ke_msg.session_id != session_id {
            return Err(TransportError::Handshake(
                "session_id mismatch in KeyExchange",
            ));
        }

        let hash = hs
            .handshake_hash()
            .map_err(|_| TransportError::Handshake("noise handshake incomplete"))?;
        let transcript = build_transcript(&hello1.client_nonce, &server_nonce, session_id);
        let (client_enc, client_dec) = derive_session_keys(&hash, &transcript);
        let (enc_key, dec_key) = (client_dec, client_enc);

        let expected_client_tag = derive_auth_tag(&dec_key, b"GS-client-fin-v2", &transcript);
        if !constant_time_eq(&ke_msg.auth_tag, &expected_client_tag) {
            return Err(TransportError::AuthenticationFailed(
                "client auth tag mismatch (noise)",
            ));
        }

        let server_auth_tag = derive_auth_tag(&enc_key, b"GS-server-fin-v2", &transcript);
        let confirm = SessionConfirm {
            session_id,
            server_auth_tag,
        };
        let mut sc_payload = [0u8; SessionConfirm::SIZE];
        confirm
            .encode(&mut sc_payload)
            .map_err(TransportError::Protocol)?;
        self.send_control(
            MessageType::HandshakeSessionConfirm,
            session_id,
            &sc_payload,
            peer,
        )
        .await?;

        *self.encrypt_key.lock().await = Some(enc_key);
        *self.decrypt_key.lock().await = Some(dec_key);
        Ok(())
    }

    /// Stub cuando la feature `noise` no está compilada.
    #[cfg(not(feature = "noise"))]
    async fn handshake_noise_server_after_hello1(
        &self,
        _peer: SocketAddr,
        _msg1: Vec<u8>,
    ) -> Result<(), TransportError> {
        Err(TransportError::Handshake(
            "compiled without `noise` feature",
        ))
    }

    // ── Handshake servidor abierto (acepta cualquier cliente) ───────────────

    /// Igual que `handshake_server` pero acepta el primer mensaje válido
    /// (Noise o `ClientHello`) de cualquier dirección. El peer queda registrado
    /// en la agenda.
    async fn handshake_server_any(&self) -> Result<(), TransportError> {
        let allow_noise = cfg!(feature = "noise")
            && matches!(
                self.handshake_mode(),
                HandshakeMode::Auto | HandshakeMode::Noise
            );
        let allow_legacy = !matches!(self.handshake_mode(), HandshakeMode::Noise);
        let (first, peer) = self
            .wait_first_handshake_packet(None, allow_noise, allow_legacy)
            .await?;

        // Registrar el peer antes de proceder: desde aquí el tráfico se envía
        // a esa dirección. En sala pueden registrarse más participantes
        // después, mediante `register_peer`.
        self.book.lock().await.insert(peer);

        match first {
            FirstHandshake::Noise(msg1) => {
                self.handshake_noise_server_after_hello1(peer, msg1).await
            }
            FirstHandshake::Legacy(hello) => self.handshake_server_after_hello(peer, hello).await,
        }
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
        let targets = self.broadcast_targets().await;
        if targets.is_empty() {
            return Err(TransportError::InvalidState("no peer"));
        }

        // audio_tx_seq es el contador exclusivo de frames de audio.  Se embebe
        // en los 4 primeros bytes del plaintext cifrado para que el jitter buffer
        // del receptor use una secuencia continua sin huecos (los paquetes FEC y
        // de control también consumen tx_sequence, pero nunca audio_tx_seq).
        let audio_seq = self.audio_tx_seq.fetch_add(1, Ordering::Relaxed);
        let seq = self.tx_sequence.fetch_add(1, Ordering::Relaxed);
        let ts = self.micros_since_epoch();
        let sid = self.session_id.load(Ordering::Acquire);

        // Plaintext = [audio_seq: 4 BE] || payload
        let mut prefixed = Vec::with_capacity(4 + payload.len());
        prefixed.extend_from_slice(&audio_seq.to_be_bytes());
        prefixed.extend_from_slice(payload);

        // Construir header con flag ENCRYPTED.
        let mut header = PacketHeader {
            version: 1,
            flags: Flags::ENCRYPTED,
            msg_type: MessageType::AudioFrame.code(),
            session_id: sid,
            sequence: seq,
            timestamp: ts,
        };

        // Cifrar prefixed si hay clave disponible.
        let (wire_payload, encrypt_flag_active) =
            if let Some(key) = self.encrypt_key.lock().await.as_ref() {
                let nonce = make_nonce(seq, sid);
                let mut hdr_buf = [0u8; HEADER_SIZE];
                header
                    .encode(&mut hdr_buf)
                    .map_err(TransportError::Protocol)?;

                let plain_len = prefixed.len();
                prefixed.resize(plain_len + TAG_SIZE, 0);
                let enc_len = encrypt_in_place(key, &nonce, &hdr_buf, &mut prefixed, plain_len)
                    .map_err(TransportError::Protocol)?;
                (Bytes::copy_from_slice(&prefixed[..enc_len]), true)
            } else {
                // Sin clave: fallback a texto claro (no debería ocurrir en sesión Active).
                header.flags = Flags::empty();
                (Bytes::copy_from_slice(&prefixed), false)
            };
        let _ = encrypt_flag_active;

        let mut buf = BytesMut::with_capacity(self.config.mtu);
        buf.resize(self.config.mtu, 0);
        let n = PacketBuilder::new(header, &wire_payload)
            .encode(&mut buf)
            .map_err(TransportError::Protocol)?;
        let mut sent_total = 0u64;
        let mut parity = {
            let mut enc = self.fec_enc.lock().await;
            enc.push(audio_seq, payload)
        };
        for p in targets {
            let sent = self.transport.send_to(&buf[..n], p).await?;
            sent_total += sent as u64;
            // La paridad FEC se envía al mismo destino que el frame, para que
            // cada peer pueda recuperar sus propias pérdidas.
            if let Some(par) = parity.take() {
                let _ = self.send_fec_parity(par, p, sid).await;
            }
        }
        self.metrics.counters.record_sent(sent_total);
        self.last_tx
            .store(self.micros_since_epoch(), Ordering::Release);

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
            header
                .encode(&mut hdr_buf)
                .map_err(TransportError::Protocol)?;

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
    ///
    /// Implementa gap-skipping: si el jitter buffer lleva más de
    /// `jitter_buffer_ms × 3` esperando por una secuencia concreta (pérdida
    /// irrecuperable), avanza el cursor y continúa con el siguiente frame.
    pub async fn recv_audio(&self) -> Result<Frame, TransportError> {
        let skip_us = u64::from(self.config.jitter_buffer_ms) * 3 * 1_000;
        let frame_ms = u64::from(self.config.frame_duration_ms).max(5);

        loop {
            let now_us = self.micros_since_epoch();

            if let Some(frame) = self.jitter.pop() {
                self.gap_since.store(0, Ordering::Release);
                return Ok(frame);
            }
            self.maintain_path().await;

            // Hay audio posterior esperando a un frame que no llega. El plazo
            // para darlo por perdido vive en la sesión, no en esta llamada:
            // quien llama suele hacerlo con un timeout corto (la app, cada
            // 20 ms), y un plazo local se reiniciaba antes de vencer nunca.
            // Un solo paquete perdido dejaba el audio parado para siempre.
            if self.jitter.has_pending() {
                match self.gap_since.load(Ordering::Acquire) {
                    0 => self.gap_since.store(now_us.max(1), Ordering::Release),
                    since if now_us.saturating_sub(since) >= skip_us => {
                        self.jitter.skip_to_available();
                        self.gap_since.store(0, Ordering::Release);
                        continue;
                    }
                    _ => {}
                }
            } else {
                self.gap_since.store(0, Ordering::Release);
            }

            // Short receive window so the gap deadline is checked frequently.
            let mut buf = vec![0u8; self.config.mtu];
            let recv_res = timeout(
                Duration::from_millis(frame_ms),
                self.transport.recv(&mut buf),
            )
            .await;

            match recv_res {
                Ok(Ok((n, from))) => {
                    self.metrics.counters.record_received(n as u64);
                    // Sondas de quien aún está eligiendo camino hacia esta
                    // sala: se contestan, pero no son tráfico de la sesión.
                    if rendezvous::is_rendezvous(&buf[..n]) {
                        self.on_rendezvous(&buf[..n], from).await;
                        continue;
                    }
                    if let Ok(view) = PacketView::decode(&buf[..n]) {
                        self.last_rx
                            .store(self.micros_since_epoch(), Ordering::Release);
                        // Un segundo participante puede pedir entrar en la sala
                        // con un ClientHello/NoiseHello1. Aquí se promueve
                        // porque este lazo es el único dueño del socket: si dos
                        // tareas leyeran el mismo `Transport`, compartirían los
                        // datagramas y se robarían audio.
                        if let Some(first) = self.peek_new_peer(&view, from) {
                            if self.promote_peer(first, from).await.is_err() {
                                // Un handshake fallido no rompe la sesión: el
                                // peer simplemente no se une.
                                tracing::debug!(?from, "handshake de peer nuevo fallido");
                            }
                            continue;
                        }
                        self.dispatch_packet(view, &buf[..n], from).await?;
                    } else {
                        self.metrics.counters.record_integrity_error();
                    }
                }
                Ok(Err(e)) => return Err(e),
                Err(_) => { /* short timeout; loop to re-check deadline */ }
            }
        }
    }

    /// Procesa un único datagrama entrante (heartbeat + mantenimiento de sesión).
    pub async fn poll_once(&self) -> Result<(), TransportError> {
        let mut buf = vec![0u8; self.config.mtu];
        let recv_res = timeout(
            Duration::from_millis(HEARTBEAT_INTERVAL_MS),
            self.transport.recv(&mut buf),
        )
        .await;

        let (n, from) = match recv_res {
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
        self.last_rx
            .store(self.micros_since_epoch(), Ordering::Release);
        // Misma detección que en `recv_audio`: un `ClientHello` desde una
        // dirección nueva promueve a ese peer en la sala. Compartir la lógica en
        // un helper evita que los dos lazos se comporten distinto.
        if let Some(first) = self.peek_new_peer(&view, from) {
            if self.promote_peer(first, from).await.is_err() {
                tracing::debug!(?from, "handshake de peer nuevo fallido");
            }
            return Ok(());
        }
        self.dispatch_packet(view, &buf[..n], from).await
    }

    /// Despacha un paquete ya decodificado a su manejador correspondiente.
    async fn dispatch_packet(
        &self,
        view: PacketView<'_>,
        raw_buf: &[u8],
        from: SocketAddr,
    ) -> Result<(), TransportError> {
        // El audio entrante se acepta de cualquier peer conocido: en sala son
        // varios y todos deben poder oírse. Un datagrama de una dirección
        // desconocida se ignora para no procesar ruido de red.
        let known = self.book.lock().await.contains(from);
        if !known && !self.try_migrate(&view, raw_buf, from).await {
            tracing::trace!(?from, "dropping datagram from unknown peer");
            return Ok(());
        }

        // Anti-replay: una vez establecidas las claves, cada secuencia sólo se
        // acepta una vez (ventana deslizante). Protege contra reinyección de
        // datagramas capturados y paquetes duplicados.
        //
        // La ventana es por emisor, no compartida: cada participante tiene su
        // propio `tx_sequence`, así que una ventana común descartaría audio
        // legítimo de los demás peers de la sala.
        if self.decrypt_key.lock().await.is_some() {
            let seq = view.header().sequence;
            let accepted = {
                let mut windows = self.replay_by_peer.lock().await;
                windows
                    .entry(from)
                    .or_insert_with(ReplayWindow::new)
                    .check_and_mark(seq)
            };
            if !accepted {
                self.metrics.counters.record_replay_drop();
                tracing::trace!(seq, ?from, "dropping replayed/stale packet");
                return Ok(());
            }
        }

        let mt = view.header().msg_type;
        match MessageType::from_code(mt) {
            Ok(MessageType::AudioFrame) => {
                let sid = self.session_id.load(Ordering::Acquire);
                let seq = view.header().sequence;
                let ts = view.header().timestamp;

                let plaintext = if view.header().flags.contains(Flags::ENCRYPTED) {
                    if let Some(key) = self.decrypt_key.lock().await.as_ref() {
                        let nonce = make_nonce(seq, sid);
                        let aad = &raw_buf[..HEADER_SIZE];
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
                        self.metrics.counters.record_integrity_error();
                        return Ok(());
                    }
                } else {
                    Bytes::copy_from_slice(view.payload())
                };

                if plaintext.len() < 4 {
                    self.metrics.counters.record_integrity_error();
                    tracing::debug!(seq, "AudioFrame plaintext too short for audio_seq prefix");
                    return Ok(());
                }
                let audio_seq =
                    u32::from_be_bytes([plaintext[0], plaintext[1], plaintext[2], plaintext[3]]);
                let audio_payload = plaintext.slice(4..);

                let frame = Frame {
                    sequence: audio_seq,
                    timestamp: ts,
                    payload: audio_payload.clone(),
                };
                self.metrics.loss.record(audio_seq);
                self.metrics
                    .jitter
                    .record(frame.timestamp, self.micros_since_epoch());
                self.fec_dec
                    .lock()
                    .await
                    .push_data(audio_seq, audio_payload);
                if !self.jitter.push(frame) {
                    tracing::trace!(seq, audio_seq, "jitter buffer rejected frame");
                }
            }
            Ok(MessageType::Heartbeat) => {
                // Se responde al emisor (`from`), no al peer principal: en sala
                // cada participante mantiene viva su propia ruta.
                let sid = self.session_id();
                self.send_control(MessageType::HeartbeatAck, sid, &[], from)
                    .await?;
            }
            Ok(MessageType::HeartbeatAck) => {}
            Ok(MessageType::AudioFec) => {
                let sid = self.session_id.load(Ordering::Acquire);
                let seq = view.header().sequence;
                if view.header().flags.contains(Flags::ENCRYPTED) {
                    if let Some(key) = self.decrypt_key.lock().await.as_ref() {
                        let nonce = make_nonce(seq, sid);
                        let aad = &raw_buf[..HEADER_SIZE];
                        let cipher_payload = view.payload();
                        let mut tmp = cipher_payload.to_vec();
                        match decrypt_in_place(key, &nonce, aad, &mut tmp, cipher_payload.len()) {
                            Ok(plain_len) if plain_len >= 5 => {
                                let fec_data = &tmp[..plain_len];
                                let seq_base = u32::from_be_bytes([
                                    fec_data[0],
                                    fec_data[1],
                                    fec_data[2],
                                    fec_data[3],
                                ]);
                                let window = fec_data[4];
                                let fec_parity = FecParity {
                                    seq_base,
                                    window,
                                    payload: Bytes::copy_from_slice(&fec_data[5..]),
                                };
                                let recovered = self.fec_dec.lock().await.push_parity(fec_parity);
                                if let Some((rec_seq, rec_payload)) = recovered {
                                    let frame = Frame {
                                        sequence: rec_seq,
                                        timestamp: 0,
                                        payload: rec_payload,
                                    };
                                    if !self.jitter.push(frame) {
                                        tracing::trace!(
                                            rec_seq,
                                            "jitter buffer rejected FEC-recovered frame"
                                        );
                                    }
                                }
                            }
                            Ok(_) => tracing::debug!(seq, "AudioFec payload too short"),
                            Err(e) => {
                                self.metrics.counters.record_integrity_error();
                                tracing::debug!(?e, seq, "AudioFec AEAD decryption failed");
                            }
                        }
                    }
                }
            }
            Ok(MessageType::ControlPause) => {
                self.peer_ptt_active.store(false, Ordering::Release);
                tracing::debug!("remote peer released PTT");
            }
            Ok(MessageType::ControlResume) => {
                self.peer_ptt_active.store(true, Ordering::Release);
                tracing::debug!("remote peer pressed PTT");
            }
            // ── Floor Control ─────────────────────────────────────────────────
            Ok(MessageType::FloorRequest) => {
                tracing::debug!("floor request received from peer");
                // En modo P2P optimista, conceder el floor inmediatamente si no
                // estamos transmitiendo nosotros. El árbitro real está en el relay.
                if !self.ptt_active.load(Ordering::Acquire) {
                    // El FloorGrant va al peer que lo pidió, no al principal:
                    // en sala hay varios participantes pidiendo turno.
                    let ssrc = self.session_id();
                    let mut payload_buf = [0u8; 4];
                    payload_buf.copy_from_slice(&ssrc.to_be_bytes());
                    let _ = self
                        .send_control(MessageType::FloorGrant, ssrc, &payload_buf, from)
                        .await;
                }
            }
            Ok(MessageType::FloorGrant) => {
                tracing::debug!("floor granted");
            }
            Ok(MessageType::FloorDeny) => {
                tracing::debug!("floor denied");
                self.ptt_active.store(false, Ordering::Release);
            }
            Ok(MessageType::FloorRelease) => {
                self.peer_ptt_active.store(false, Ordering::Release);
                tracing::debug!("remote peer released floor");
            }
            Ok(MessageType::FloorTaken) => {
                tracing::debug!("floor taken by another peer");
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
        // Activar señal de cancelación ANTES de todo para que los loops
        // bloqueantes (handshake_server_any, recv_audio) salgan en ≤500 ms.
        self.closed.store(true, Ordering::Release);

        // Borrar claves de sesión al cerrar.
        *self.encrypt_key.lock().await = None;
        *self.decrypt_key.lock().await = None;

        // Avisar a todos los peers conocidos: en sala el CLOSE debe llegar a cada
        // participante, no solo al principal.
        for p in self.broadcast_targets().await {
            let _ = self
                .send_control(MessageType::Close, self.session_id(), &[], p)
                .await;
        }
        // Dos transiciones, no una: `Active -> Closing -> Closed`. La FSM exige
        // pasar por `Closing` antes de llegar a `Closed`. Con una sola la
        // sesión se quedaba en `Closing` para siempre y no podía
        // re-emparejarse.
        let mut sm = self.state.lock().await;
        let _ = sm.transition(SessionEvent::Close);
        let _ = sm.transition(SessionEvent::Close);
        drop(sm);
        self.book.lock().await.clear();
        self.replay_by_peer.lock().await.clear();
        Ok(())
    }

    /// Envía un heartbeat a cada peer conocido.
    ///
    /// En sala esto es lo que mantiene viva la ruta en el relay: `evict_idle`
    /// borra las rutas sin tráfico tras `session_ttl_secs` (300 s), y con PTT
    /// hay intervalos largos en silencio.
    async fn send_heartbeat(&self) -> Result<(), TransportError> {
        let sid = self.session_id();
        for peer in self.broadcast_targets().await {
            let _ = self
                .send_control(MessageType::Heartbeat, sid, &[], peer)
                .await;
        }
        Ok(())
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
fn derive_session_keys(
    shared_secret: &[u8; 32],
    transcript: &[u8; 68],
) -> (SessionKey, SessionKey) {
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
