//! Motor del harness: abre sesiones reales, transmite audio y verifica que
//! el audio llegó *y* es correcto.
//!
//! A diferencia de `gs ptt`, que sólo cuenta datagramas (y por eso no puede
//! distinguir audio válido de ruido), este módulo:
//!
//! - alterna turnos de habla según un plan temporal, para comprobar el floor
//!   control con turnos disjuntos y medir bidireccionalidad real;
//! - acumula el PCM recibido y lo analiza (frecuencia, RMS, saturación);
//! - evalúa los criterios dentro del proceso y devuelve `exit != 0` si
//!   fallan, en vez de dejar que un shell haga `sed` sobre el stdout.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use gravital_talk::constants::{DEFAULT_MTU, MAX_PAYLOAD_SIZE};
use gravital_talk::{
    CodecId, CodecSession, Config, SessionRole, Transport, UdpConfig, UdpTransport,
};
use tokio::sync::Mutex;

use crate::analyze::{self, AudioStats};
use crate::sine::sine_frames_i16;

/// Configuración de audio de todas las sesiones del harness.
pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: u8 = 1;

/// Duración de frame documentada en el README (20 ms).
///
/// OJO: 20 ms de PCM16 mono son 1920 bytes y el core impone un tope duro de
/// `MAX_PAYLOAD_SIZE` (1176 B), así que un frame PCM de 20 ms **no se puede
/// enviar** con el MTU por defecto. Ver `pcm_frame_duration_ms` y el test
/// `pcm_twenty_ms_frame_does_not_fit_the_wire_limit`.
pub const FRAME_DURATION_MS: u64 = 20;

/// Duración de frame que sí cabe en el tope del core para PCM16 mono.
pub const PCM_FRAME_DURATION_MS: u64 = 10;

/// Frecuencia por defecto de la senoidal de prueba.
pub const DEFAULT_FREQ_HZ: f64 = 440.0;

/// Muestras por canal en un frame de `frame_ms` milisegundos.
pub const fn frame_samples(frame_ms: u64) -> usize {
    (SAMPLE_RATE * frame_ms as u32 / 1000) as usize
}

/// Mayor duración de frame (ms) cuyo payload PCM quepa en el tope del core.
///
/// El payload viaja como `[audio_seq: 4] || pcm`, y el AEAD añade `TAG_SIZE`.
pub fn pcm_frame_duration_ms() -> u64 {
    let per_ms = SAMPLE_RATE as u64 * 2 / 1000; // PCM16 mono: 2 bytes por muestra
                                                // payload + 4 (audio_seq) + 16 (tag AEAD) <= MAX_PAYLOAD_SIZE
    let usable = MAX_PAYLOAD_SIZE.saturating_sub(4 + 16);
    (usable as u64 / per_ms).max(1)
}

/// Bytes de payload de un frame PCM16 mono de `frame_ms` milisegundos.
pub const fn pcm_frame_bytes(frame_ms: u64) -> usize {
    frame_samples(frame_ms) * 2
}

/// Margen tras el final para drenar el jitter buffer del último frame.
const DRAIN_MS: u64 = 150;

/// Tope de muestras acumuladas (10 min a 48 kHz mono) para que una sesión sin
/// corte no crezca sin límite.
const fn max_rx_samples() -> usize {
    SAMPLE_RATE as usize * 600
}

/// Ventana temporal en la que un par habla, en milisegundos desde el inicio.
///
/// Es la pieza que hace verificable la bidireccionalidad: si el host habla en
/// `[0, 3000)` y el join en `[3000, 6000)`, cada lado recibe audio
/// exclusivamente durante la ventana del otro. Con un plan "hablar siempre"
/// (que es lo que hace `gs ptt --headless`) no se puede distinguir un
/// full-duplex funcional de un audio que se devuelve sin control de turno.
#[derive(Debug, Clone, Copy)]
pub struct TurnPlan {
    pub start_ms: u64,
    pub end_ms: u64,
}

impl TurnPlan {
    /// Habla durante toda la ejecución.
    pub const fn all() -> Self {
        Self {
            start_ms: 0,
            end_ms: u64::MAX,
        }
    }

    /// No habla nunca: sólo escucha.
    pub const fn never() -> Self {
        Self {
            start_ms: u64::MAX,
            end_ms: u64::MAX,
        }
    }

    /// Parsea `all`, `never` o `INICIO-FIN` en milisegundos.
    pub fn parse(spec: &str) -> Result<Self> {
        let spec = spec.trim();
        match spec {
            "all" | "todo" => return Ok(Self::all()),
            "never" | "nunca" | "none" => return Ok(Self::never()),
            _ => {}
        }
        let (start, end) = spec
            .split_once('-')
            .with_context(|| format!("invalid turn plan '{spec}', expected all|never|START-END"))?;
        Ok(Self {
            start_ms: start
                .trim()
                .parse()
                .with_context(|| format!("invalid turn plan start in '{spec}'"))?,
            end_ms: end
                .trim()
                .parse()
                .with_context(|| format!("invalid turn plan end in '{spec}'"))?,
        })
    }

    /// `true` si en `elapsed_ms` este actor debe estar hablando.
    pub const fn contains(&self, elapsed_ms: u64) -> bool {
        elapsed_ms >= self.start_ms && elapsed_ms < self.end_ms
    }

    /// `true` si el plan deja algún momento de habla.
    pub const fn speaks_at_all(&self) -> bool {
        self.start_ms < self.end_ms
    }
}

/// Cómo se alcanza el peer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransportMode {
    /// UDP directo entre los dos extremos.
    P2p,
    /// A través del relay, usando un código de sala.
    Room,
}

impl TransportMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::P2p => "p2p",
            Self::Room => "room",
        }
    }
}

/// Rol dentro del par.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerRole {
    /// Espera al peer (acepta el primer handshake válido de cualquier origen).
    Host,
    /// Inicia la conexión contra una dirección conocida.
    Join,
}

impl PeerRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Host => "host",
            Self::Join => "join",
        }
    }
}

/// Contadores y observaciones de un peer durante la ejecución.
#[derive(Debug, Default, Clone)]
pub struct RunStats {
    pub tx_frames: u64,
    pub rx_frames: u64,
    /// Veces que el PTT remoto se activó. Es la única señal de turno
    /// observable desde la API pública de `Session` (no expone eventos).
    pub peer_ptt_activations: u64,
    pub send_errors: u64,
    /// Errores de recepción. Incluye el caso normal del `CLOSE` del peer al
    /// terminar, así que un valor de 1 al final de la corrida no es un fallo.
    pub recv_errors: u64,
}

/// Parámetros de una ejecución de peer.
#[derive(Debug, Clone)]
pub struct PeerOptions {
    pub label: String,
    pub role: PeerRole,
    pub mode: TransportMode,
    /// Dirección de escucha del socket UDP local.
    pub bind_addr: SocketAddr,
    /// Si es `true` y el rol es `Host`, el handshake de servidor espera al
    /// peer esperado (`handshake(Server, peer)`) en vez de aceptar cualquier
    /// origen (`handshake_open`).
    ///
    /// Hace falta en modo sala: todo el tráfico llega con dirección de origen
    /// del relay, así que el host debe esperarspecifically a esa dirección.
    /// Es el mismo criterio que usa `gs ptt --listen --relay`.
    pub expect_known_peer: bool,
    /// Destino del handshake. El host lo ignora: usa `handshake_open`.
    pub peer_addr: SocketAddr,
    /// Fija el `session_id` antes del handshake (modo sala, para que el relay
    /// pueda enrutarlo).
    pub preset_session_id: Option<u32>,
    pub room_token: Option<String>,
    pub codec: CodecId,
    pub freq_hz: f64,
    pub duration_ms: u64,
    pub turn: TurnPlan,
    pub dump_rx_wav: Option<PathBuf>,
    /// Duración del frame de audio en milisegundos.
    pub frame_duration_ms: u64,
    /// MTU por paquete. El default del core (1200) es válido mientras el
    /// payload quepa en `MAX_PAYLOAD_SIZE`.
    pub mtu: usize,
    /// Verifica el tono del audio recibido (frecuencia, RMS, saturación).
    pub verify_audio: bool,
    pub freq_tolerance_hz: f64,
    pub min_rms: f64,
    pub expect_rx_frames: Option<u64>,
    pub expect_tx_frames: Option<u64>,
    pub expect_peer_ptt_activations: Option<u64>,
}

impl PeerOptions {
    /// Nombre legible del códec, para el informe.
    pub fn codec_label(&self) -> String {
        format!("{:?}", self.codec)
    }
}

/// Resultado de un peer, con sus criterios ya evaluados.
#[derive(Debug)]
pub struct PeerOutcome {
    pub label: String,
    pub role: PeerRole,
    pub mode: TransportMode,
    pub codec: String,
    pub freq_hz: f64,
    pub local_addr: String,
    pub peer_addr: String,
    pub stats: RunStats,
    pub audio: AudioStats,
    pub rx_wav: Option<PathBuf>,
    pub rtt_ms: f64,
    pub jitter_ms: f64,
    pub loss_percent: f64,
    pub mos: f64,
    pub packets_received: u64,
    pub bytes_received: u64,
    /// Criterios incumplidos. Vacío = pasó.
    pub failures: Vec<String>,
}

impl PeerOutcome {
    pub const fn passed(&self) -> bool {
        self.failures.is_empty()
    }
}

/// Ejecuta un peer completo: handshake, turnos, transmisión y verificación.
pub async fn run_peer(opts: &PeerOptions) -> Result<PeerOutcome> {
    let transport = UdpTransport::bind(UdpConfig {
        bind_addr: opts.bind_addr,
        ..Default::default()
    })
    .await
    .with_context(|| format!("cannot bind UDP transport on {}", opts.bind_addr))?;
    let local_addr = transport
        .local_addr()
        .with_context(|| format!("cannot read local addr on {}", opts.bind_addr))?;
    let transport: Arc<dyn Transport> = Arc::new(transport);

    // Validación temprana: es mejor fallar con un mensaje explícito que descubrir
    // el problema frame a frame como "output buffer too small".
    let samples_per_frame = frame_samples(opts.frame_duration_ms);
    if opts.codec == CodecId::Pcm {
        let payload = samples_per_frame * CHANNELS as usize * 2;
        // El payload viaja como [audio_seq: 4] || pcm, y el AEAD añade 16 de tag.
        let wire = payload + 4 + 16;
        if wire > MAX_PAYLOAD_SIZE {
            bail!(
                "un frame PCM de {} ms ocupa {} bytes de payload y no cabe en el tope del \
                 core ({} bytes utiles, MTU {}): el envio fallaria con 'payload exceeds \
                 maximum size'. Usa --frame-ms {} o menos",
                opts.frame_duration_ms,
                payload,
                MAX_PAYLOAD_SIZE,
                opts.mtu,
                pcm_frame_duration_ms()
            );
        }
    }

    let config = Config {
        sample_rate: SAMPLE_RATE,
        channels: CHANNELS,
        frame_duration_ms: opts.frame_duration_ms as u8,
        mtu: opts.mtu,
        ..Config::default()
    };
    let cs = Arc::new(
        CodecSession::new(transport, config, opts.codec)
            .with_context(|| format!("cannot create codec session for {}", opts.codec_label()))?,
    );
    let session = cs.session();

    if let Some(sid) = opts.preset_session_id {
        session.set_preset_session_id(sid);
    }
    if let Some(token) = opts.room_token.as_ref() {
        if !token.is_empty() {
            session.set_room_token(Some(token.clone()));
        }
    }

    // Handshake. En P2P el host usa `handshake_open` porque el puerto del join
    // es efímero: es el mismo camino que el emparejamiento por QR, donde el
    // servidor no conoce la dirección del cliente.
    //
    // En modo sala NO vale: todo el tráfico llega reenviado por el relay, así
    // que hay que esperar a esa dirección concreta (`handshake(Server, peer)`),
    // igual que hace `gs ptt --listen --relay`.
    match opts.role {
        PeerRole::Host if opts.expect_known_peer => session
            .handshake(SessionRole::Server, opts.peer_addr)
            .await
            .with_context(|| {
                format!(
                    "[{}] handshake as server against {} failed",
                    opts.label, opts.peer_addr
                )
            })?,
        PeerRole::Host => session
            .handshake_open()
            .await
            .with_context(|| format!("[{}] handshake_open failed", opts.label))?,
        PeerRole::Join => cs
            .handshake(SessionRole::Client, opts.peer_addr)
            .await
            .with_context(|| {
                format!(
                    "[{}] handshake as client against {} failed",
                    opts.label, opts.peer_addr
                )
            })?,
    }

    // Recepción en task aparte: `recv_samples` bloquea hasta el próximo frame.
    let rx_samples: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::new()));
    let rx_frames = Arc::new(AtomicU64::new(0));
    let recv_errors = Arc::new(AtomicU64::new(0));
    let quit = Arc::new(AtomicBool::new(false));

    {
        let cs = cs.clone();
        let rx_samples = Arc::clone(&rx_samples);
        let rx_frames = Arc::clone(&rx_frames);
        let recv_errors = Arc::clone(&recv_errors);
        let quit_recv = Arc::clone(&quit);
        tokio::spawn(async move {
            while !quit_recv.load(Ordering::Acquire) {
                match cs.recv_samples().await {
                    Ok(samples) => {
                        rx_frames.fetch_add(1, Ordering::Relaxed);
                        let mut guard = rx_samples.lock().await;
                        if guard.len() < max_rx_samples() {
                            guard.extend_from_slice(&samples);
                        }
                    }
                    Err(_) => {
                        recv_errors.fetch_add(1, Ordering::Relaxed);
                        break;
                    }
                }
            }
        });
    }

    // Envío siguiendo el plan de turnos.
    let tx_frames = Arc::new(AtomicU64::new(0));
    let send_errors = Arc::new(AtomicU64::new(0));
    // El primer error de envio se conserva con su mensaje: un contador sin
    // motivo no sirve para diagnosticar.
    let first_send_error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let peer_ptt_activations = Arc::new(AtomicU64::new(0));
    let mut sine = sine_frames_i16(samples_per_frame, CHANNELS, SAMPLE_RATE, opts.freq_hz);
    let mut ptt_on = false;
    let mut peer_ptt_was_active = false;

    let start = std::time::Instant::now();
    while start.elapsed().as_millis() < u128::from(opts.duration_ms) {
        let elapsed_ms = start.elapsed().as_millis() as u64;
        let should_speak = opts.turn.contains(elapsed_ms);

        if should_speak && !ptt_on {
            // Pedir turno no es fatal: el PTT es una señal de control y el
            // audio sigue fluyendo aunque el floor se rechace.
            let _ = session.ptt_press().await;
            ptt_on = true;
        } else if !should_speak && ptt_on {
            let _ = session.ptt_release().await;
            ptt_on = false;
        }

        if ptt_on {
            if let Some(frame) = sine.next() {
                match cs.send_samples(&frame).await {
                    Ok(()) => {
                        tx_frames.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(e) => {
                        send_errors.fetch_add(1, Ordering::Relaxed);
                        let mut slot = first_send_error.lock().await;
                        if slot.is_none() {
                            *slot = Some(format!("{e}"));
                        }
                    }
                }
            }
        }

        // Sondeo del PTT remoto: la API pública no emite eventos de floor.
        if session.is_peer_ptt_active() && !peer_ptt_was_active {
            peer_ptt_activations.fetch_add(1, Ordering::Relaxed);
        }
        peer_ptt_was_active = session.is_peer_ptt_active();

        tokio::time::sleep(Duration::from_millis(opts.frame_duration_ms)).await;
    }

    if ptt_on {
        let _ = session.ptt_release().await;
    }
    // Margen para que el último frame del peer vacíe el jitter buffer.
    tokio::time::sleep(Duration::from_millis(DRAIN_MS)).await;

    let _ = cs.close().await;
    quit.store(true, Ordering::Release);

    let samples = rx_samples.lock().await.clone();
    let audio = AudioStats::analyze(&samples, SAMPLE_RATE, CHANNELS as u16);
    if let Some(path) = opts.dump_rx_wav.as_ref() {
        analyze::write_wav(path, &samples, SAMPLE_RATE, CHANNELS as u16)
            .with_context(|| format!("cannot dump rx wav to {}", path.display()))?;
    }

    let snap = session
        .metrics()
        .snapshot(session.jitter_buffer().fill_percent());
    let stats = RunStats {
        tx_frames: tx_frames.load(Ordering::Relaxed),
        rx_frames: rx_frames.load(Ordering::Relaxed),
        peer_ptt_activations: peer_ptt_activations.load(Ordering::Relaxed),
        send_errors: send_errors.load(Ordering::Relaxed),
        recv_errors: recv_errors.load(Ordering::Relaxed),
    };

    let mut failures = Vec::new();
    if let Some(min) = opts.expect_rx_frames {
        if stats.rx_frames < min {
            failures.push(format!("rx frames {} < esperado {}", stats.rx_frames, min));
        }
    }
    if let Some(min) = opts.expect_tx_frames {
        if stats.tx_frames < min {
            failures.push(format!("tx frames {} < esperado {}", stats.tx_frames, min));
        }
    }
    if let Some(min) = opts.expect_peer_ptt_activations {
        if stats.peer_ptt_activations < min {
            failures.push(format!(
                "eventos de PTT remoto {} < esperado {}: el turno del peer nunca fue visible",
                stats.peer_ptt_activations, min
            ));
        }
    }
    if opts.verify_audio {
        if let Err(e) = audio.expect_tone(opts.freq_hz, opts.freq_tolerance_hz, opts.min_rms) {
            failures.push(format!("audio inválido: {e}"));
        }
    }

    let send_error = first_send_error.lock().await.clone();
    if let Some(msg) = send_error {
        failures.push(format!("error de envio: {msg}"));
    }

    Ok(PeerOutcome {
        label: opts.label.clone(),
        role: opts.role,
        mode: opts.mode,
        codec: opts.codec_label(),
        freq_hz: opts.freq_hz,
        local_addr: local_addr.to_string(),
        peer_addr: opts.peer_addr.to_string(),
        stats,
        audio,
        rx_wav: opts.dump_rx_wav.clone(),
        rtt_ms: f64::from(snap.rtt_ms),
        jitter_ms: f64::from(snap.jitter_ms),
        loss_percent: f64::from(snap.loss_percent),
        mos: f64::from(snap.estimated_mos),
        packets_received: snap.packets_received,
        bytes_received: snap.bytes_received,
        failures,
    })
}

/// Configuración de un par host/join completo.
#[derive(Debug, Clone)]
pub struct PairOptions {
    pub scenario: String,
    pub mode: TransportMode,
    /// Duración del frame de audio en milisegundos.
    pub frame_duration_ms: u64,
    /// MTU por paquete.
    pub mtu: usize,
    /// Dirección donde escucha el host.
    pub host_bind: SocketAddr,
    pub host_turn: TurnPlan,
    pub join_turn: TurnPlan,
    pub freq_hz: f64,
    /// Relay y sala (modo `Room`).
    pub relay: Option<String>,
    pub relay_udp_port: u16,
    pub relay_obs_port: u16,
    pub room: Option<String>,
    pub room_token: Option<String>,
    pub codec: CodecId,
    pub duration_ms: u64,
    pub freq_tolerance_hz: f64,
    pub min_rms: f64,
    pub verify_audio: bool,
    pub expect_rx_frames_each: Option<u64>,
    pub expect_tx_frames_each: Option<u64>,
    pub expect_peer_ptt_activations: Option<u64>,
    pub dump_dir: Option<PathBuf>,
}

impl Default for PairOptions {
    fn default() -> Self {
        Self {
            scenario: "pair".to_string(),
            mode: TransportMode::P2p,
            frame_duration_ms: FRAME_DURATION_MS,
            mtu: DEFAULT_MTU,
            host_bind: SocketAddr::from(([127, 0, 0, 1], 34_100)),
            host_turn: TurnPlan::all(),
            join_turn: TurnPlan::all(),
            freq_hz: DEFAULT_FREQ_HZ,
            relay: None,
            relay_udp_port: 9000,
            relay_obs_port: 9100,
            room: None,
            room_token: None,
            codec: CodecId::Pcm,
            duration_ms: 2_000,
            freq_tolerance_hz: 8.0,
            min_rms: 0.01,
            verify_audio: false,
            expect_rx_frames_each: None,
            expect_tx_frames_each: None,
            expect_peer_ptt_activations: None,
            dump_dir: None,
        }
    }
}

impl PairOptions {
    /// Divide la duración en dos turnos disjuntos: el host habla la primera
    /// mitad, el join la segunda.
    ///
    /// Así cada lado recibe audio *mientras el otro habla*, que es lo que hace
    /// verificable la bidireccionalidad y el control de turno.
    pub fn with_split_turns(&mut self) -> &mut Self {
        let half = (self.duration_ms / 2).max(1);
        self.host_turn = TurnPlan {
            start_ms: 0,
            end_ms: half,
        };
        self.join_turn = TurnPlan {
            start_ms: half,
            end_ms: self.duration_ms,
        };
        self
    }

    fn peer_options(&self, role: PeerRole) -> PeerOptions {
        let label = format!("{}/{}", self.scenario, role.as_str());
        PeerOptions {
            dump_rx_wav: self
                .dump_dir
                .as_ref()
                .map(|d| d.join(format!("{}-{}-rx.wav", self.scenario, role.as_str()))),
            mtu: self.mtu,
            label,
            role,
            mode: self.mode,
            bind_addr: if role == PeerRole::Host {
                self.host_bind
            } else {
                SocketAddr::from(([127, 0, 0, 1], 0))
            },
            expect_known_peer: false,
            peer_addr: self.host_bind,
            preset_session_id: None,
            room_token: self.room_token.clone(),
            codec: self.codec,
            freq_hz: self.freq_hz,
            duration_ms: self.duration_ms,
            frame_duration_ms: self.frame_duration_ms,
            turn: if role == PeerRole::Host {
                self.host_turn
            } else {
                self.join_turn
            },
            verify_audio: self.verify_audio,
            freq_tolerance_hz: self.freq_tolerance_hz,
            min_rms: self.min_rms,
            expect_rx_frames: self.expect_rx_frames_each,
            expect_tx_frames: self.expect_tx_frames_each,
            expect_peer_ptt_activations: self.expect_peer_ptt_activations,
        }
    }
}

/// Ejecuta un par host/join y devuelve ambos resultados.
///
/// El host se lanza primero y el join entra con un pequeño retardo, para que
/// el servidor esté escuchando cuando el cliente intente el handshake.
pub async fn run_pair(opts: &PairOptions) -> Result<Vec<PeerOutcome>> {
    // Modo sala: resolver el código da el `session_id` que el relay usa para
    // enrutar el handshake de todos los participantes.
    let (target, preset_session_id) = match opts.mode {
        TransportMode::P2p => (opts.host_bind, None),
        TransportMode::Room => {
            let relay = opts
                .relay
                .as_ref()
                .context("--relay es obligatorio en modo room")?;
            let room = opts
                .room
                .as_ref()
                .context("--room es obligatorio en modo room")?;
            let sid = crate::http::resolve_room(
                relay,
                opts.relay_obs_port,
                room,
                opts.room_token.as_deref(),
            )
            .await
            .with_context(|| format!("cannot resolve room {room}"))?;
            let addr = crate::http::lookup_host_any(relay, opts.relay_udp_port)
                .await
                .with_context(|| format!("cannot resolve relay {relay}"))?;
            (addr, Some(sid))
        }
    };

    let mut host_opts = opts.peer_options(PeerRole::Host);
    host_opts.peer_addr = target;
    host_opts.preset_session_id = preset_session_id;
    // En sala el host espera al relay, no "cualquier origen".
    host_opts.expect_known_peer = opts.mode == TransportMode::Room;
    let mut join_opts = opts.peer_options(PeerRole::Join);
    join_opts.peer_addr = target;
    join_opts.preset_session_id = preset_session_id;

    let host_task = host_opts.clone();
    let join_task = join_opts.clone();
    let host_fut = tokio::spawn(async move { run_peer(&host_task).await });
    tokio::time::sleep(Duration::from_millis(150)).await;
    let join_fut = tokio::spawn(async move { run_peer(&join_task).await });

    let host = host_fut
        .await
        .context("host task panicked")?
        .with_context(|| format!("[{}] host failed", host_opts.label))?;
    let join = join_fut
        .await
        .context("join task panicked")?
        .with_context(|| format!("[{}] join failed", join_opts.label))?;

    Ok(vec![host, join])
}

/// Ejecuta `n` pares independientes de forma concurrente.
///
/// Cada par usa una frecuencia distinta (`base + i * step`), así el análisis
/// del audio recibido demuestra que las sesiones no se mezclaron: si hubiera
/// cruce de tráfico, un par mediría la frecuencia de otro. Los turnos van
/// partidos por la mitad de la duración, de modo que cada par verifica
/// audio en ambas direcciones.
pub async fn run_multi_pair(opts: &PairOptions, n: usize) -> Result<Vec<PeerOutcome>> {
    if n == 0 {
        bail!("--sessions debe ser >= 1");
    }
    let base_port = opts.host_bind.port();

    let mut futures = Vec::with_capacity(n);
    for i in 0..n {
        let mut pair = opts.clone();
        pair.scenario = format!("{}#{i}", opts.scenario);
        pair.host_bind = SocketAddr::from(([127, 0, 0, 1], base_port + i as u16));
        pair.freq_hz = DEFAULT_FREQ_STEP_HZ.mul_add(i as f64, opts.freq_hz);
        pair.with_split_turns();
        futures.push(tokio::spawn(async move { run_pair(&pair).await }));
    }

    let mut all = Vec::with_capacity(n * 2);
    for f in futures {
        let outcomes = f.await.context("pair task panicked")??;
        all.extend(outcomes);
    }
    Ok(all)
}

/// Separación de frecuencia entre sesiones paralelas, en Hz.
pub const DEFAULT_FREQ_STEP_HZ: f64 = 60.0;

/// Resumen legible de una ejecución, para consola.
pub fn render_summary(outcomes: &[PeerOutcome]) -> String {
    let mut out = String::new();
    for o in outcomes {
        let status = if o.passed() { "PASS" } else { "FAIL" };
        out.push_str(&format!(
            "[{status}] {:<22} role={:<4} mode={:<4} codec={:<6} tx={:<4} rx={:<4} \
             rx_hz={:<6.1} rms={:.4} mos={:.2} rtt={:.1}ms loss={:.1}%\n",
            o.label,
            o.role.as_str(),
            o.mode.as_str(),
            o.codec,
            o.stats.tx_frames,
            o.stats.rx_frames,
            o.audio.freq_hz,
            o.audio.rms,
            o.mos,
            o.rtt_ms,
            o.loss_percent
        ));
        for f in &o.failures {
            out.push_str(&format!("         fallo: {f}\n"));
        }
    }
    let passed = outcomes.iter().filter(|o| o.passed()).count();
    out.push_str(&format!("\n{passed}/{} peers pasaron\n", outcomes.len()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_turn_plans() {
        assert!(TurnPlan::parse("all").unwrap().contains(999_999));
        assert!(TurnPlan::parse("todo").unwrap().contains(0));
        assert!(!TurnPlan::parse("never").unwrap().contains(0));
        assert!(!TurnPlan::parse("nunca").unwrap().speaks_at_all());

        let plan = TurnPlan::parse("3000-6000").unwrap();
        assert_eq!(plan.start_ms, 3000);
        assert_eq!(plan.end_ms, 6000);
        assert!(!plan.contains(2_999));
        assert!(plan.contains(3_000));
        assert!(plan.contains(5_999));
        // Fin exclusivo: los turnos de host y join no pueden solaparse.
        assert!(!plan.contains(6_000));
    }

    #[test]
    fn rejects_malformed_turn_plans() {
        assert!(TurnPlan::parse("nonsense").is_err());
        assert!(TurnPlan::parse("1000-").is_err());
        assert!(TurnPlan::parse("-2000").is_err());
    }

    #[test]
    fn split_turns_are_disjoint_and_cover_the_run() {
        let mut opts = PairOptions {
            duration_ms: 4_000,
            ..PairOptions::default()
        };
        opts.with_split_turns();
        assert_eq!(opts.host_turn.start_ms, 0);
        assert_eq!(opts.host_turn.end_ms, 2_000);
        assert_eq!(opts.join_turn.start_ms, 2_000);
        assert_eq!(opts.join_turn.end_ms, 4_000);
        // Ningún instante tiene ambos hablando.
        for ms in 0..4_000 {
            assert!(
                !(opts.host_turn.contains(ms) && opts.join_turn.contains(ms)),
                "turns overlap at {ms} ms"
            );
        }
    }

    #[test]
    fn frame_geometry_matches_the_documented_audio_format() {
        // 20 ms a 48 kHz mono son 960 muestras, como documenta el README.
        assert_eq!(frame_samples(20), 960);
        assert_eq!(frame_samples(20) * CHANNELS as usize, 960);
        assert_eq!(frame_samples(10), 480);
    }

    /// Fija la limitación real del protocolo: un frame PCM de 20 ms NO cabe en
    /// el tope del core, aunque el README lo documente como el formato de
    /// audio del proyecto.
    ///
    /// Si este test empieza a fallar es porque alguien arregló el transporte
    /// (fragmentando el payload, por ejemplo) y entonces `--frame-ms 20` con
    /// PCM debe volver a ser viable: en ese caso actualiza el README y el
    /// `docs/packet-format.md`.
    #[test]
    fn pcm_twenty_ms_frame_does_not_fit_the_wire_limit() {
        let payload_20ms = pcm_frame_bytes(FRAME_DURATION_MS);
        assert_eq!(payload_20ms, 1920, "20 ms PCM16 mono son 1920 bytes");
        // + audio_seq (4) + tag AEAD (16) = wire payload que excede el tope.
        let wire_20ms = payload_20ms + 4 + 16;
        assert!(
            wire_20ms > MAX_PAYLOAD_SIZE,
            "un frame de 20 ms ({wire_20ms} B) no puede pasar por un paquete de \
             {MAX_PAYLOAD_SIZE} B utiles"
        );

        // 10 ms y 12 ms sí caben. El máximo exacto es 12 ms (1176 - 20 = 1156 bytes
        // útiles = 1152 a 12 ms); el harness usa 10 ms porque es un tamaño de
        // frame válido también para Opus (2.5/5/10/20/40/60 ms).
        let wire_10ms = pcm_frame_bytes(PCM_FRAME_DURATION_MS) + 4 + 16;
        assert!(wire_10ms <= MAX_PAYLOAD_SIZE, "10 ms deberia caber");
        assert_eq!(pcm_frame_duration_ms(), 12, "el maximo que cabe es 12 ms");
        assert!(
            PCM_FRAME_DURATION_MS <= pcm_frame_duration_ms(),
            "el default del harness debe caber"
        );
        // Y 13 ms ya no cabe, que es lo que hace la comprobación del runtime.
        let wire_13ms = pcm_frame_bytes(13) + 4 + 16;
        assert!(wire_13ms > MAX_PAYLOAD_SIZE, "13 ms no deberia caber");
    }

    #[test]
    fn multi_pair_assigns_distinct_frequencies() {
        let base = 400.0;
        let freqs: Vec<f64> = (0..4)
            .map(|i| DEFAULT_FREQ_STEP_HZ.mul_add(i as f64, base))
            .collect();
        assert_eq!(freqs, vec![400.0, 460.0, 520.0, 580.0]);
        // Separación suficiente para que el análisis no se solape.
        for w in freqs.windows(2) {
            assert!(w[1] - w[0] > 40.0);
        }
    }
}
