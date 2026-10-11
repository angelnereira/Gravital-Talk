//! Gravital Talk — superficie FFI C.
//!
//! Exporta una API estable en estilo handle-opaco + códigos de error.
//!
//! ## Seguridad
//!
//! Todas las funciones que reciben punteros validan nulidad y longitud antes
//! de tocar memoria. Los handles son `repr(C)` pero opaque desde C (sólo
//! se pasan por puntero).
//!
//! Cada sesión mantiene su propio runtime Tokio actual-threaded para no
//! requerir que el embebedor configure async.

#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(clippy::missing_safety_doc)]
#![allow(non_camel_case_types)]

use std::cell::RefCell;
use std::ffi::{c_char, c_int, CStr, CString};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ptr;
use std::sync::{Arc, Mutex};

mod audio;
mod voice;

use gravital_talk::{
    discover_local_addr, discover_public_addr, reach, Config as RustConfig, HandshakeMode,
    LatencyClass, MetricsSnapshot, Session, SessionRole, SessionState, TransportError, UdpConfig,
    UdpTransport,
};

thread_local! {
    static LAST_ERROR: RefCell<Option<CString>> = const { RefCell::new(None) };
}

fn set_last_error(msg: impl Into<String>) {
    let cstr =
        CString::new(msg.into()).unwrap_or_else(|_| CString::new("invalid error message").unwrap());
    LAST_ERROR.with(|e| *e.borrow_mut() = Some(cstr));
}

/// Handle opaco a una sesión.
#[repr(C)]
pub struct GsSessionHandle {
    _private: [u8; 0],
}

struct SessionInner {
    /// Va primero para soltarse primero: sus hilos usan el runtime y la sesión.
    audio: Mutex<Option<audio::Engine>>,
    /// En `Arc` porque los hilos de audio también hacen `block_on` sobre él.
    runtime: Arc<tokio::runtime::Runtime>,
    session: Arc<Session>,
    /// Camino de audio: comprime y descomprime lo que viaja por la sesión.
    pipeline: Arc<voice::Pipeline>,
    config: GsConfig,
    /// Puerto abierto en el router por UPnP, para cerrarlo al terminar.
    port_mapping: Mutex<Option<gravital_talk::upnp::PortMapping>>,
}

impl SessionInner {
    /// Cierra el puerto abierto en el router, si lo hay. Con poco plazo: si el
    /// router no contesta, la concesión caduca sola.
    fn close_port_mapping(&self) {
        let mapping = match self.port_mapping.lock() {
            Ok(mut g) => g.take(),
            Err(p) => p.into_inner().take(),
        };
        if let Some(mapping) = mapping {
            let _ = self.runtime.block_on(async {
                tokio::time::timeout(std::time::Duration::from_secs(2), mapping.remove()).await
            });
        }
    }

    fn audio(&self) -> std::sync::MutexGuard<'_, Option<audio::Engine>> {
        match self.audio.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        }
    }
}

/// Versión ABI estable.
pub const GS_ABI_VERSION: u32 = 1;

/// Códigos de error. `GS_OK = 0`.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GsStatus {
    GS_OK = 0,
    GS_ERR_NULL_POINTER = -1,
    GS_ERR_INVALID_ARGUMENT = -2,
    GS_ERR_IO = -3,
    GS_ERR_TIMEOUT = -4,
    GS_ERR_HANDSHAKE = -5,
    GS_ERR_PROTOCOL = -6,
    GS_ERR_INVALID_STATE = -7,
    GS_ERR_CLOSED = -8,
    GS_ERR_BUFFER_TOO_SMALL = -9,
    /// La operación no existe en este build (p. ej. audio nativo en Android).
    GS_ERR_UNSUPPORTED = -10,
    GS_ERR_INTERNAL = -99,
}

/// Estado dinámico de una sesión para consumidores C.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GsSessionState {
    GS_STATE_IDLE = 0,
    GS_STATE_HANDSHAKING = 1,
    GS_STATE_ACTIVE = 2,
    GS_STATE_PAUSED = 3,
    GS_STATE_CLOSING = 4,
    GS_STATE_CLOSED = 5,
    GS_STATE_ERROR = 6,
    GS_STATE_RECONNECTING = 7,
}

impl From<SessionState> for GsSessionState {
    fn from(s: SessionState) -> Self {
        match s {
            SessionState::Idle => Self::GS_STATE_IDLE,
            SessionState::Handshaking => Self::GS_STATE_HANDSHAKING,
            SessionState::Active => Self::GS_STATE_ACTIVE,
            SessionState::Paused => Self::GS_STATE_PAUSED,
            SessionState::Closing => Self::GS_STATE_CLOSING,
            SessionState::Closed => Self::GS_STATE_CLOSED,
            SessionState::Error => Self::GS_STATE_ERROR,
            SessionState::Reconnecting => Self::GS_STATE_RECONNECTING,
        }
    }
}

/// Configuración pasada desde C a `gs_session_create`.
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct GsConfig {
    pub sample_rate: u32,
    pub channels: u8,
    pub frame_duration_ms: u8,
    pub max_bitrate: u32,
    pub codec_preferred: u8,
    pub capability_flags: u32,
    pub jitter_buffer_ms: u16,
    pub mtu: u32,
}

impl From<&GsConfig> for RustConfig {
    fn from(c: &GsConfig) -> Self {
        Self {
            sample_rate: c.sample_rate,
            channels: c.channels,
            frame_duration_ms: c.frame_duration_ms,
            max_bitrate: c.max_bitrate,
            codec_preferred: c.codec_preferred,
            supported_codecs: vec![0x01, 0x02],
            capability_flags: c.capability_flags,
            jitter_buffer_ms: c.jitter_buffer_ms,
            mtu: c.mtu as usize,
            handshake_mode: HandshakeMode::Auto,
        }
    }
}

/// Layout binario idéntico a `MetricsSnapshot`. Ver `docs/protocol-spec.md`.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct GsMetrics {
    pub rtt_ms: f32,
    pub jitter_ms: f32,
    pub loss_percent: f32,
    pub reorder_percent: f32,
    pub buffer_fill_percent: f32,
    pub estimated_mos: f32,
    pub packets_sent: u64,
    pub packets_received: u64,
    pub bytes_sent: u64,
    pub bytes_received: u64,
}

impl From<MetricsSnapshot> for GsMetrics {
    fn from(s: MetricsSnapshot) -> Self {
        Self {
            rtt_ms: s.rtt_ms,
            jitter_ms: s.jitter_ms,
            loss_percent: s.loss_percent,
            reorder_percent: s.reorder_percent,
            buffer_fill_percent: s.buffer_fill_percent,
            estimated_mos: s.estimated_mos,
            packets_sent: s.packets_sent,
            packets_received: s.packets_received,
            bytes_sent: s.bytes_sent,
            bytes_received: s.bytes_received,
        }
    }
}

// ─── Funciones públicas ────────────────────────────────────────────────

/// Devuelve la versión SemVer del crate como C-string estática.
#[no_mangle]
pub extern "C" fn gs_version() -> *const c_char {
    static VERSION: &str = concat!(env!("CARGO_PKG_VERSION"), "\0");
    VERSION.as_ptr() as *const c_char
}

/// Versión del protocolo wire.
#[no_mangle]
pub extern "C" fn gs_protocol_version() -> u32 {
    u32::from(gravital_talk::PROTOCOL_VERSION)
}

/// Versión del ABI C.
#[no_mangle]
pub const extern "C" fn gs_abi_version() -> u32 {
    GS_ABI_VERSION
}

/// Rellena `out` con la configuración por default.
///
/// # Safety
/// `out` debe ser un puntero válido a un `GsConfig` escribible.
#[no_mangle]
pub unsafe extern "C" fn gs_config_default(out: *mut GsConfig) -> GsStatus {
    if out.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let d = RustConfig::default();
    let cfg = GsConfig {
        sample_rate: d.sample_rate,
        channels: d.channels,
        frame_duration_ms: d.frame_duration_ms,
        max_bitrate: d.max_bitrate,
        codec_preferred: d.codec_preferred,
        capability_flags: d.capability_flags,
        jitter_buffer_ms: d.jitter_buffer_ms,
        mtu: d.mtu as u32,
    };
    unsafe { *out = cfg };
    GsStatus::GS_OK
}

/// Crea una sesión bindeando un socket UDP.
///
/// `bind_addr` e `bind_port` controlan el socket local; pasa `0.0.0.0:0` para
/// ephemeral.
///
/// # Safety
/// `config`, `bind_addr` y `out_handle` deben ser punteros válidos. `bind_addr`
/// debe apuntar a una C-string NUL-terminada.
#[no_mangle]
pub unsafe extern "C" fn gs_session_create(
    config: *const GsConfig,
    bind_addr: *const c_char,
    bind_port: u16,
    out_handle: *mut *mut GsSessionHandle,
) -> GsStatus {
    if config.is_null() || bind_addr.is_null() || out_handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let cfg_c = unsafe { &*config };
    let addr_str = match unsafe { CStr::from_ptr(bind_addr) }.to_str() {
        Ok(s) => s,
        Err(_) => return GsStatus::GS_ERR_INVALID_ARGUMENT,
    };
    let ip: IpAddr = match addr_str.parse() {
        Ok(ip) => ip,
        Err(_) => {
            // Caso típico: "0.0.0.0"; si es sólo IP, sigue siendo válido. Si
            // viene con puerto, rechazamos — se pasa por `bind_port`.
            if addr_str.is_empty() {
                IpAddr::V4(Ipv4Addr::UNSPECIFIED)
            } else {
                return GsStatus::GS_ERR_INVALID_ARGUMENT;
            }
        }
    };
    let bind = SocketAddr::new(ip, bind_port);
    let rust_cfg = RustConfig::from(cfg_c);

    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            set_last_error(format!("failed to build runtime: {e}"));
            return GsStatus::GS_ERR_INTERNAL;
        }
    };

    let transport = match runtime.block_on(UdpTransport::bind(UdpConfig {
        bind_addr: bind,
        peer: None,
        latency_class: LatencyClass::RealTime,
        ..Default::default()
    })) {
        Ok(t) => Arc::new(t) as Arc<dyn gravital_talk::Transport>,
        Err(e) => {
            set_last_error(format!("bind failed: {e}"));
            return GsStatus::GS_ERR_IO;
        }
    };

    let session = Arc::new(Session::new(transport, rust_cfg));
    let pipeline = Arc::new(voice::Pipeline::new(
        session.clone(),
        cfg_c.sample_rate,
        cfg_c.channels,
        cfg_c.frame_duration_ms,
        cfg_c.max_bitrate,
    ));
    let inner = Box::new(SessionInner {
        audio: Mutex::new(None),
        runtime: Arc::new(runtime),
        session,
        pipeline,
        config: *cfg_c,
        port_mapping: Mutex::new(None),
    });
    let ptr = Box::into_raw(inner) as *mut GsSessionHandle;
    unsafe { *out_handle = ptr };
    GsStatus::GS_OK
}

/// Destruye una sesión y libera su runtime. El handle pasa a ser inválido.
///
/// # Safety
/// `handle` debe haber sido creado por `gs_session_create` y no haber sido
/// destruido ya. `NULL` es un no-op seguro.
#[no_mangle]
pub unsafe extern "C" fn gs_session_destroy(handle: *mut GsSessionHandle) {
    if handle.is_null() {
        return;
    }
    // SAFETY: `handle` viene de `gs_session_create` y no se ha destruido ya
    // (contrato); a partir de aquí nadie más lo usa.
    let inner = unsafe { Box::from_raw(handle as *mut SessionInner) };
    inner.close_port_mapping();
}

/// Ejecuta el handshake como cliente hacia `peer_addr:peer_port`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_connect(
    handle: *mut GsSessionHandle,
    peer_addr: *const c_char,
    peer_port: u16,
) -> GsStatus {
    unsafe { handshake_common(handle, peer_addr, peer_port, SessionRole::Client) }
}

/// Ejecuta el handshake como servidor esperando a `peer_addr:peer_port`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_accept(
    handle: *mut GsSessionHandle,
    peer_addr: *const c_char,
    peer_port: u16,
) -> GsStatus {
    unsafe { handshake_common(handle, peer_addr, peer_port, SessionRole::Server) }
}

unsafe fn handshake_common(
    handle: *mut GsSessionHandle,
    peer_addr: *const c_char,
    peer_port: u16,
    role: SessionRole,
) -> GsStatus {
    if handle.is_null() || peer_addr.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let addr_str = match unsafe { CStr::from_ptr(peer_addr) }.to_str() {
        Ok(s) => s,
        Err(_) => return GsStatus::GS_ERR_INVALID_ARGUMENT,
    };
    let ip: IpAddr = match addr_str.parse() {
        Ok(ip) => ip,
        Err(_) => return GsStatus::GS_ERR_INVALID_ARGUMENT,
    };
    let peer = SocketAddr::new(ip, peer_port);

    let session = inner.session.clone();
    match inner
        .runtime
        .block_on(async move { session.handshake(role, peer).await })
    {
        Ok(()) => GsStatus::GS_OK,
        Err(e) => {
            set_last_error(format!("handshake: {e}"));
            GsStatus::GS_ERR_HANDSHAKE
        }
    }
}

/// Envía un frame de audio. Requiere estado `Active`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_send_audio(
    handle: *mut GsSessionHandle,
    data: *const u8,
    len: usize,
) -> GsStatus {
    if handle.is_null() || (data.is_null() && len != 0) {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let slice = unsafe { std::slice::from_raw_parts(data, len) };
    let pipeline = inner.pipeline.clone();
    let payload = slice.to_vec();
    match inner
        .runtime
        .block_on(async move { pipeline.send_pcm(&payload).await })
    {
        Ok(()) => GsStatus::GS_OK,
        Err(e) => {
            set_last_error(format!("send_audio: {e}"));
            GsStatus::GS_ERR_IO
        }
    }
}

/// Recibe el próximo frame de audio. `buf` debe tener al menos `*len_inout`
/// bytes; al retorno `*len_inout` contiene los bytes escritos.
#[no_mangle]
pub unsafe extern "C" fn gs_session_recv_audio(
    handle: *mut GsSessionHandle,
    buf: *mut u8,
    len_inout: *mut usize,
) -> GsStatus {
    unsafe { recv_audio_impl(handle, buf, len_inout, None) }
}

/// Igual que `gs_session_recv_audio` pero con timeout en milisegundos.
///
/// Devuelve `GS_ERR_TIMEOUT` si no llegó ningún frame en `timeout_ms`.
/// Con `timeout_ms = 0` es no bloqueante (solo saca lo ya disponible).
/// Pensada para loops de audio en isolates que deben poder parar.
#[no_mangle]
pub unsafe extern "C" fn gs_session_recv_audio_timeout(
    handle: *mut GsSessionHandle,
    buf: *mut u8,
    len_inout: *mut usize,
    timeout_ms: u32,
) -> GsStatus {
    unsafe { recv_audio_impl(handle, buf, len_inout, Some(timeout_ms)) }
}

unsafe fn recv_audio_impl(
    handle: *mut GsSessionHandle,
    buf: *mut u8,
    len_inout: *mut usize,
    timeout_ms: Option<u32>,
) -> GsStatus {
    if handle.is_null() || buf.is_null() || len_inout.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let cap = unsafe { *len_inout };
    let pipeline = inner.pipeline.clone();

    let recv = async move {
        match timeout_ms {
            Some(ms) => {
                let dur = std::time::Duration::from_millis(u64::from(ms));
                tokio::time::timeout(dur, pipeline.recv_pcm()).await
            }
            None => Ok(pipeline.recv_pcm().await),
        }
    };

    let frame = match inner.runtime.block_on(recv) {
        Ok(Ok(f)) => f,
        Ok(Err(e)) => {
            set_last_error(format!("recv_audio: {e}"));
            return GsStatus::GS_ERR_IO;
        }
        Err(_) => return GsStatus::GS_ERR_TIMEOUT,
    };
    let n = frame.len();
    if n > cap {
        unsafe { *len_inout = n };
        return GsStatus::GS_ERR_BUFFER_TOO_SMALL;
    }
    unsafe {
        ptr::copy_nonoverlapping(frame.as_ptr(), buf, n);
        *len_inout = n;
    }
    GsStatus::GS_OK
}

/// Cierra la sesión enviando `CLOSE`. El handle queda en estado `Closed`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_close(handle: *mut GsSessionHandle) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    // El audio se para antes de cerrar: sus hilos leen y escriben la sesión.
    drop(inner.audio().take());
    inner.close_port_mapping();
    inner.pipeline.reset();
    let session = inner.session.clone();
    let _ = inner.runtime.block_on(async move { session.close().await });
    GsStatus::GS_OK
}

/// Rearma una sesión cerrada para poder emparejar de nuevo.
///
/// Es el requisito de "cualquiera de los dos cierra y vuelve a solicitar el
/// emparejamiento". Sin esto, `gs_session_close` deja el handle inútil para
/// siempre: sólo cabe destruirlo y crear otro, con el coste de rehacer todo el
/// setup (transporte, configuración, token de sala).
///
/// Preserva la configuración de la sala (token, modo de handshake) y borra el
/// estado de la conexión anterior: claves, peers, ventanas anti-replay,
/// métricas y contadores.
///
/// Devuelve `GS_ERR_INVALID_STATE` si la sesión no estaba cerrada: rearrancar en
/// caliente perdería audio sin aviso.
#[no_mangle]
pub unsafe extern "C" fn gs_session_reopen(handle: *mut GsSessionHandle) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let session = inner.session.clone();
    match inner
        .runtime
        .block_on(async move { session.reopen().await })
    {
        Ok(()) => GsStatus::GS_OK,
        Err(_) => GsStatus::GS_ERR_INVALID_STATE,
    }
}

/// Devuelve el estado actual por el puntero `out_state`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_state(
    handle: *mut GsSessionHandle,
    out_state: *mut GsSessionState,
) -> GsStatus {
    if handle.is_null() || out_state.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let session = inner.session.clone();
    let st = inner.runtime.block_on(async move { session.state().await });
    unsafe { *out_state = GsSessionState::from(st) };
    GsStatus::GS_OK
}

/// Devuelve el `session_id` negociado.
#[no_mangle]
pub unsafe extern "C" fn gs_session_id(handle: *mut GsSessionHandle, out_id: *mut u32) -> GsStatus {
    if handle.is_null() || out_id.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    unsafe { *out_id = inner.session.session_id() };
    GsStatus::GS_OK
}

/// Fija el `session_id` a usar durante el handshake.
///
/// Es lo que habilita el **modo servidor/relay por sala**: todas las partes
/// comparten el mismo id para que el relay pueda enrutar los paquetes del
/// handshake. Debe llamarse **antes** de `gs_session_connect` /
/// `gs_session_accept`. Un valor `0` restaura el modo P2P (id aleatorio).
#[no_mangle]
pub unsafe extern "C" fn gs_session_set_session_id(
    handle: *mut GsSessionHandle,
    session_id: u32,
) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    inner.session.set_preset_session_id(session_id);
    GsStatus::GS_OK
}

/// Fija el token de sala (PSK de Noise).
///
/// Con token, el handshake usa `Noise_NNpsk0` y no hay downgrade: peers sin
/// el token no pueden conectar. `NULL` o `""` desactiva el token.
/// Debe llamarse **antes** de `gs_session_connect` / `gs_session_accept`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_set_room_token(
    handle: *mut GsSessionHandle,
    token: *const c_char,
) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let token = if token.is_null() {
        None
    } else {
        match unsafe { CStr::from_ptr(token) }.to_str() {
            Ok(s) => Some(s.to_string()),
            Err(_) => return GsStatus::GS_ERR_INVALID_ARGUMENT,
        }
    };
    let inner = unsafe { &*(handle as *mut SessionInner) };
    inner.session.set_room_token(token);
    GsStatus::GS_OK
}

/// Modo de handshake: `0` = Auto (Noise con fallback), `1` = Noise, `2` = Legacy.
///
/// Debe llamarse **antes** de `gs_session_connect` / `gs_session_accept`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_set_handshake_mode(
    handle: *mut GsSessionHandle,
    mode: u8,
) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let mode = match mode {
        1 => HandshakeMode::Noise,
        2 => HandshakeMode::Legacy,
        _ => HandshakeMode::Auto,
    };
    let inner = unsafe { &*(handle as *mut SessionInner) };
    inner.session.set_handshake_mode(mode);
    GsStatus::GS_OK
}

/// Rellena `out` con un snapshot atómico de métricas.
#[no_mangle]
pub unsafe extern "C" fn gs_session_metrics(
    handle: *mut GsSessionHandle,
    out: *mut GsMetrics,
) -> GsStatus {
    if handle.is_null() || out.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let fill = inner.session.jitter_buffer().fill_percent();
    let snap = inner.session.metrics().snapshot(fill);
    unsafe { *out = GsMetrics::from(snap) };
    GsStatus::GS_OK
}

/// Devuelve el último error como C-string NUL-terminada, o NULL si no hay.
/// El puntero es válido hasta la próxima llamada FFI en el mismo hilo.
#[no_mangle]
pub extern "C" fn gs_error_last() -> *const c_char {
    LAST_ERROR.with(|e| e.borrow().as_ref().map_or(ptr::null(), |s| s.as_ptr()))
}

/// Limpia el buffer de último error.
#[no_mangle]
pub extern "C" fn gs_error_clear() {
    LAST_ERROR.with(|e| *e.borrow_mut() = None);
}

/// Solicita el floor (permiso para transmitir).
///
/// Envía `FloorRequest` al peer/relay. El resultado real (Grant/Deny)
/// llega asíncronamente vía `dispatch_packet`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_ptt_press(handle: *mut GsSessionHandle) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let session = inner.session.clone();
    match inner
        .runtime
        .block_on(async move { session.ptt_press().await })
    {
        Ok(()) => GsStatus::GS_OK,
        Err(e) => {
            set_last_error(format!("ptt_press: {e}"));
            GsStatus::GS_ERR_IO
        }
    }
}

/// Libera el floor (fin de transmisión).
#[no_mangle]
pub unsafe extern "C" fn gs_session_ptt_release(handle: *mut GsSessionHandle) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let session = inner.session.clone();
    match inner
        .runtime
        .block_on(async move { session.ptt_release().await })
    {
        Ok(()) => GsStatus::GS_OK,
        Err(e) => {
            set_last_error(format!("ptt_release: {e}"));
            GsStatus::GS_ERR_IO
        }
    }
}

/// Devuelve `1` si el peer remoto está transmitiendo actualmente, `0` si no.
#[no_mangle]
pub unsafe extern "C" fn gs_session_is_peer_ptt_active(handle: *mut GsSessionHandle) -> c_int {
    if handle.is_null() {
        return 0;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    if inner.session.is_peer_ptt_active() {
        1
    } else {
        0
    }
}

/// Devuelve el `local_ssrc` de la sesión.
#[no_mangle]
pub unsafe extern "C" fn gs_session_local_ssrc(
    handle: *mut GsSessionHandle,
    out_ssrc: *mut u32,
) -> GsStatus {
    if handle.is_null() || out_ssrc.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    unsafe { *out_ssrc = inner.session.local_ssrc() };
    GsStatus::GS_OK
}

/// Devuelve el puerto UDP local del socket de la sesión.
///
/// Útil para incluirlo en el QR de pairing (el host lo usa como `lan_port`).
///
/// # Safety
/// `handle` debe ser válido y `out_port` no nulo.
#[no_mangle]
pub unsafe extern "C" fn gs_session_local_port(
    handle: *mut GsSessionHandle,
    out_port: *mut u16,
) -> GsStatus {
    if handle.is_null() || out_port.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    match inner.session.local_addr() {
        Ok(addr) => {
            unsafe { *out_port = addr.port() };
            GsStatus::GS_OK
        }
        Err(e) => {
            set_last_error(format!("local_addr: {e}"));
            GsStatus::GS_ERR_IO
        }
    }
}

/// Descubre la dirección IP pública usando STUN (stun.l.google.com:19302).
///
/// Escribe `"ip:port"` como C-string NUL-terminada en `out_buf`.
/// `buf_len` debe ser al menos 48 bytes para acomodar IPv4+puerto.
///
/// Diagnostica si este dispositivo puede recibir invitados de fuera de su red.
///
/// Es la comprobación que evita el fallo silencioso: sin ella, un anfitrión
/// detrás de CGNAT (red móvil) muestra un QR que nunca va a funcionar y nadie
/// entiende por qué. Con ella, la app avisa *antes* de que el invitado lo
/// intente.
///
/// Escribe en `out_code` uno de:
/// `0` = alcanzable públicamente, `1` = sólo LAN, `2` = CGNAT, `3` = desconocido.
///
/// Devuelve `GS_OK` siempre que el diagnóstico se pueda expresar; el
/// `GS_ERR_*` es para fallos de la propia llamada, no de la red.
///
/// # Safety
/// `out_code` debe apuntar a un `i32` escribible.
#[no_mangle]
pub unsafe extern "C" fn gs_diagnose_reachability(local_port: u16, out_code: *mut i32) -> GsStatus {
    if out_code.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            set_last_error(format!("runtime: {e}"));
            return GsStatus::GS_ERR_INTERNAL;
        }
    };

    // `map_or` en vez de un `match`: el caso de error es el mismo para todos
    // los fallos de red, y no hace falta distinguirlos.
    let code = runtime
        .block_on(discover_public_addr(local_port))
        .map_or(reach::Reachability::Unknown as i32, |addr| {
            reach::diagnose(addr.ip()) as i32
        });
    unsafe { *out_code = code };
    GsStatus::GS_OK
}

/// Puerto que dedica Gravital Talk al anfitrión, según política.
///
/// `preferred` de 0 significa "el que sea más rápido y sencillo", que en la
/// práctica es el puerto documentado (9000) si está libre. La ventaja de un
/// puerto fijo sobre el efímero es concreta: el reenvío del router sólo hay que
/// hacerlo una vez.
///
/// # Safety
/// `out_port` debe apuntar a un `u16` escribible.
#[no_mangle]
pub unsafe extern "C" fn gs_preferred_host_port(preferred: u16, out_port: *mut u16) -> GsStatus {
    if out_port.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    unsafe { *out_port = reach::preferred_host_port(preferred) };
    GsStatus::GS_OK
}

/// # Safety
/// `out_buf` debe apuntar a un buffer de al menos `buf_len` bytes escribibles.
#[no_mangle]
pub unsafe extern "C" fn gs_discover_public_addr(
    local_port: u16,
    out_buf: *mut c_char,
    buf_len: usize,
) -> GsStatus {
    if out_buf.is_null() || buf_len < 8 {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            set_last_error(format!("runtime: {e}"));
            return GsStatus::GS_ERR_INTERNAL;
        }
    };
    match runtime.block_on(discover_public_addr(local_port)) {
        Ok(addr) => {
            let s = format!("{addr}");
            let bytes = s.as_bytes();
            if bytes.len() + 1 > buf_len {
                return GsStatus::GS_ERR_BUFFER_TOO_SMALL;
            }
            unsafe {
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr() as *const c_char,
                    out_buf,
                    bytes.len(),
                );
                *out_buf.add(bytes.len()) = 0;
            }
            GsStatus::GS_OK
        }
        Err(e) => {
            set_last_error(format!("STUN: {e}"));
            GsStatus::GS_ERR_TIMEOUT
        }
    }
}

/// Descubre la IP local (LAN) de este dispositivo.
///
/// Es el host que va en el QR: dos dispositivos en la misma WiFi se conectan
/// por la LAN, y la IP pública del router no siempre vuelve a entrar (hairpin
/// NAT).
///
/// Escribe **sólo la IP** como C-string NUL-terminada. El puerto es el de la
/// sesión (`gs_session_local_port`): el socket con el que se averigua la IP es
/// desechable y su puerto no sirve a nadie.
///
/// # Safety
/// `out_buf` debe apuntar a un buffer de al menos `buf_len` bytes escribibles.
#[no_mangle]
pub unsafe extern "C" fn gs_discover_local_addr(out_buf: *mut c_char, buf_len: usize) -> GsStatus {
    if out_buf.is_null() || buf_len < 8 {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(rt) => rt,
        Err(e) => {
            set_last_error(format!("runtime: {e}"));
            return GsStatus::GS_ERR_INTERNAL;
        }
    };
    match runtime.block_on(discover_local_addr()) {
        Ok(addr) => {
            if addr.ip().is_unspecified() {
                set_last_error("local addr: sin ruta de red");
                return GsStatus::GS_ERR_IO;
            }
            let s = addr.ip().to_string();
            let bytes = s.as_bytes();
            if bytes.len() + 1 > buf_len {
                return GsStatus::GS_ERR_BUFFER_TOO_SMALL;
            }
            // SAFETY: `out_buf` tiene `buf_len` bytes escribibles (contrato de
            // la función) y se acaba de comprobar que caben los bytes y el NUL.
            unsafe {
                std::ptr::copy_nonoverlapping(
                    bytes.as_ptr() as *const c_char,
                    out_buf,
                    bytes.len(),
                );
                *out_buf.add(bytes.len()) = 0;
            }
            GsStatus::GS_OK
        }
        Err(e) => {
            set_last_error(format!("local addr: {e}"));
            GsStatus::GS_ERR_IO
        }
    }
}

/// Handshake servidor que acepta el primer cliente que llegue desde cualquier
/// dirección (modo QR/code pairing — no se requiere conocer la IP del cliente).
///
/// Bloqueante hasta completar el handshake o agotar el timeout configurado.
///
/// # Safety
/// `handle` debe ser válido y creado con `gs_session_create`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_accept_any(handle: *mut GsSessionHandle) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let session = inner.session.clone();
    // Quien acepta a cualquiera aloja una sala: tras el primer invitado sigue
    // admitiendo a más.
    session.set_multi_peer(true);
    match inner
        .runtime
        .block_on(async move { session.handshake_open().await })
    {
        Ok(()) => GsStatus::GS_OK,
        Err(TransportError::PeerClosed(_) | TransportError::Closed) => GsStatus::GS_ERR_CLOSED,
        Err(e) => {
            set_last_error(format!("accept_any: {e}"));
            GsStatus::GS_ERR_HANDSHAKE
        }
    }
}

// ─── Conexión entre redes distintas ────────────────────────────────────

/// Copia `text` a `out_buf` como C-string NUL-terminada.
///
/// # Safety
/// `out_buf` debe apuntar a `buf_len` bytes escribibles.
unsafe fn write_c_string(text: &str, out_buf: *mut c_char, buf_len: usize) -> GsStatus {
    let bytes = text.as_bytes();
    if bytes.len() + 1 > buf_len {
        return GsStatus::GS_ERR_BUFFER_TOO_SMALL;
    }
    // SAFETY: `out_buf` tiene `buf_len` bytes escribibles (contrato) y se acaba
    // de comprobar que caben los bytes y el NUL.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr() as *const c_char, out_buf, bytes.len());
        *out_buf.add(bytes.len()) = 0;
    }
    GsStatus::GS_OK
}

/// Fija el servidor de encuentro que coordina la perforación de NAT.
///
/// Con él, dos dispositivos en redes distintas conectan directo: el servidor
/// (un `gs-relay`) sólo le dice a cada uno la dirección pública del otro; el
/// audio no pasa por él. Llamar después de `gs_session_set_room_token` y antes
/// del handshake. `host` nulo o vacío lo desactiva.
///
/// Devuelve `GS_ERR_IO` si `host` no resuelve.
///
/// # Safety
/// `handle` debe ser válido; `host`, nulo o una C-string NUL-terminada.
#[no_mangle]
pub unsafe extern "C" fn gs_session_set_rendezvous(
    handle: *mut GsSessionHandle,
    host: *const c_char,
    port: u16,
) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    if host.is_null() {
        inner.session.set_rendezvous(None);
        return GsStatus::GS_OK;
    }
    // SAFETY: `host` no es nulo y es una C-string NUL-terminada (contrato).
    let Ok(host) = unsafe { CStr::from_ptr(host) }.to_str() else {
        return GsStatus::GS_ERR_INVALID_ARGUMENT;
    };
    if host.is_empty() {
        inner.session.set_rendezvous(None);
        return GsStatus::GS_OK;
    }
    let resolved = inner.runtime.block_on(async {
        let lookup = tokio::net::lookup_host((host, port));
        tokio::time::timeout(std::time::Duration::from_secs(4), lookup).await
    });
    // El socket de la sesión es IPv4: una dirección IPv6 no serviría.
    match resolved {
        Ok(Ok(mut addrs)) => {
            let ipv4 = addrs.find(SocketAddr::is_ipv4);
            if ipv4.is_none() {
                set_last_error(format!("rendezvous: {host} no tiene dirección IPv4"));
                return GsStatus::GS_ERR_IO;
            }
            inner.session.set_rendezvous(ipv4);
            GsStatus::GS_OK
        }
        Ok(Err(e)) => {
            set_last_error(format!("rendezvous: {host}: {e}"));
            GsStatus::GS_ERR_IO
        }
        Err(_) => {
            set_last_error(format!("rendezvous: {host} no resolvió a tiempo"));
            GsStatus::GS_ERR_TIMEOUT
        }
    }
}

/// Dirección pública (`ip:puerto`) del socket **de esta sesión**, vista por
/// STUN.
///
/// A diferencia de `gs_discover_public_addr`, que pregunta desde otro socket,
/// el puerto que devuelve es el de la sala: es la dirección que tiene sentido
/// poner en una invitación. Llamar antes del handshake.
///
/// # Safety
/// `handle` debe ser válido y `out_buf` apuntar a `buf_len` bytes escribibles.
#[no_mangle]
pub unsafe extern "C" fn gs_session_public_addr(
    handle: *mut GsSessionHandle,
    out_buf: *mut c_char,
    buf_len: usize,
) -> GsStatus {
    if handle.is_null() || out_buf.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let session = inner.session.clone();
    match inner
        .runtime
        .block_on(async move { session.discover_public_addr().await })
    {
        // SAFETY: `out_buf` y `buf_len` cumplen el contrato de esta función.
        Ok(addr) => unsafe { write_c_string(&addr.to_string(), out_buf, buf_len) },
        Err(e) => {
            set_last_error(format!("public addr: {e}"));
            GsStatus::GS_ERR_IO
        }
    }
}

/// Interpreta una lista `ip:puerto` separada por comas.
///
/// # Safety
/// `text` debe ser una C-string NUL-terminada.
unsafe fn parse_addr_list(text: *const c_char) -> Result<Vec<SocketAddr>, GsStatus> {
    // SAFETY: `text` es una C-string NUL-terminada (contrato).
    let Ok(text) = unsafe { CStr::from_ptr(text) }.to_str() else {
        return Err(GsStatus::GS_ERR_INVALID_ARGUMENT);
    };
    let mut addrs = Vec::new();
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.parse() {
            Ok(addr) => addrs.push(addr),
            Err(_) => {
                set_last_error(format!("dirección no válida: {part}"));
                return Err(GsStatus::GS_ERR_INVALID_ARGUMENT);
            }
        }
    }
    Ok(addrs)
}

/// Añade direcciones (`ip:puerto` separadas por comas) por las que buscar al
/// otro extremo.
///
/// Mientras se espera invitado, el anfitrión las sonda: es como se perfora el
/// NAT sin servidor, cuando la dirección del invitado llega por otro medio
/// (su respuesta a la invitación). Se puede llamar desde otro hilo con el
/// handshake en curso.
///
/// # Safety
/// `handle` debe ser válido y `addrs` una C-string NUL-terminada.
#[no_mangle]
pub unsafe extern "C" fn gs_session_add_path_hints(
    handle: *mut GsSessionHandle,
    addrs: *const c_char,
) -> GsStatus {
    if handle.is_null() || addrs.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    // SAFETY: `addrs` no es nulo y es una C-string NUL-terminada.
    match unsafe { parse_addr_list(addrs) } {
        Ok(list) => {
            inner.session.add_path_hints(&list);
            GsStatus::GS_OK
        }
        Err(status) => status,
    }
}

/// Pide al router que abra un puerto hacia esta sesión (UPnP) y escribe la
/// dirección pública resultante (`ip:puerto`).
///
/// Con el puerto abierto, la sala es alcanzable desde cualquier red sin
/// servidor de por medio. Se cierra solo en `gs_session_close`.
///
/// Devuelve `GS_ERR_UNSUPPORTED` si no hay router que lo admita o si el router
/// está a su vez detrás de otro NAT (la dirección no sería pública).
///
/// # Safety
/// `handle` debe ser válido y `out_buf` apuntar a `buf_len` bytes escribibles.
#[no_mangle]
pub unsafe extern "C" fn gs_session_map_port(
    handle: *mut GsSessionHandle,
    out_buf: *mut c_char,
    buf_len: usize,
) -> GsStatus {
    if handle.is_null() || out_buf.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let port = match inner.session.local_addr() {
        Ok(addr) => addr.port(),
        Err(e) => {
            set_last_error(format!("map port: {e}"));
            return GsStatus::GS_ERR_IO;
        }
    };
    // Una hora: de sobra para una conversación, y si la app muere sin cerrar
    // el puerto, el router lo cierra solo.
    let lease = std::time::Duration::from_secs(3600);
    match inner
        .runtime
        .block_on(gravital_talk::upnp::map_udp_port(port, lease))
    {
        Ok(mapping) => {
            let external = mapping.external.to_string();
            match inner.port_mapping.lock() {
                Ok(mut slot) => *slot = Some(mapping),
                Err(p) => *p.into_inner() = Some(mapping),
            }
            // SAFETY: `out_buf` y `buf_len` cumplen el contrato de esta función.
            unsafe { write_c_string(&external, out_buf, buf_len) }
        }
        Err(e) => {
            set_last_error(format!("map port: {e}"));
            GsStatus::GS_ERR_UNSUPPORTED
        }
    }
}

/// Milisegundos desde el último paquete recibido del otro extremo, o `-1` si
/// aún no se ha recibido ninguno.
///
/// Sirve para avisar de que la conexión se ha interrumpido (cambio de red,
/// cobertura) antes de que el usuario hable al vacío.
///
/// # Safety
/// `handle` debe ser un handle válido de `gs_session_create`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_rx_idle_ms(handle: *mut GsSessionHandle) -> i64 {
    if handle.is_null() {
        return -1;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    inner
        .session
        .rx_idle_ms()
        .map_or(-1, |ms| i64::try_from(ms).unwrap_or(i64::MAX))
}

/// Devuelve `1` si el audio de la sesión está pasando por el relay y `0` si
/// va directo entre los dos dispositivos.
///
/// # Safety
/// `handle` debe ser un handle válido de `gs_session_create`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_is_relayed(handle: *mut GsSessionHandle) -> c_int {
    if handle.is_null() {
        return 0;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let session = inner.session.clone();
    c_int::from(
        inner
            .runtime
            .block_on(async move { session.is_relayed().await }),
    )
}

/// Como `gs_session_connect_any`, esperando hasta `timeout_ms` a que algún
/// camino directo conteste.
///
/// El plazo largo es para cuando la dirección de este extremo le llega al
/// anfitrión a mano (escaneando la respuesta a la invitación): hay que seguir
/// sondando mientras tanto.
///
/// # Safety
/// `handle` debe ser válido y `candidates` una C-string NUL-terminada.
#[no_mangle]
pub unsafe extern "C" fn gs_session_connect_any_timeout(
    handle: *mut GsSessionHandle,
    candidates: *const c_char,
    timeout_ms: u32,
) -> GsStatus {
    if handle.is_null() || candidates.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    // SAFETY: `candidates` no es nulo y es una C-string NUL-terminada.
    let addrs = match unsafe { parse_addr_list(candidates) } {
        Ok(list) => list,
        Err(status) => return status,
    };
    let wait = std::time::Duration::from_millis(u64::from(timeout_ms));
    let session = inner.session.clone();
    match inner
        .runtime
        .block_on(async move { session.handshake_any_within(&addrs, wait).await })
    {
        Ok(()) => GsStatus::GS_OK,
        Err(TransportError::Timeout) => {
            set_last_error("handshake timeout");
            GsStatus::GS_ERR_TIMEOUT
        }
        Err(e) => {
            set_last_error(format!("handshake: {e}"));
            GsStatus::GS_ERR_HANDSHAKE
        }
    }
}

/// Handshake de cliente contra un anfitrión con varias direcciones posibles.
///
/// `candidates` es una lista `ip:puerto` separada por comas (red local,
/// pública…). Se sondan todas a la vez y se conecta por la primera que
/// contesta, prefiriendo la de red local. Puede ir vacía si hay servidor de
/// encuentro: la dirección del anfitrión llega entonces por él.
///
/// # Safety
/// `handle` debe ser válido y `candidates` una C-string NUL-terminada.
#[no_mangle]
pub unsafe extern "C" fn gs_session_connect_any(
    handle: *mut GsSessionHandle,
    candidates: *const c_char,
) -> GsStatus {
    if handle.is_null() || candidates.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    // SAFETY: `candidates` no es nulo y es una C-string NUL-terminada.
    let Ok(text) = unsafe { CStr::from_ptr(candidates) }.to_str() else {
        return GsStatus::GS_ERR_INVALID_ARGUMENT;
    };
    let mut addrs: Vec<SocketAddr> = Vec::new();
    for part in text.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        match part.parse() {
            Ok(addr) => addrs.push(addr),
            Err(_) => {
                set_last_error(format!("candidata no válida: {part}"));
                return GsStatus::GS_ERR_INVALID_ARGUMENT;
            }
        }
    }
    let session = inner.session.clone();
    match inner
        .runtime
        .block_on(async move { session.handshake_any(&addrs).await })
    {
        Ok(()) => GsStatus::GS_OK,
        Err(TransportError::Timeout) => {
            set_last_error("handshake timeout");
            GsStatus::GS_ERR_TIMEOUT
        }
        Err(e) => {
            set_last_error(format!("handshake: {e}"));
            GsStatus::GS_ERR_HANDSHAKE
        }
    }
}

/// Número de dispositivos conectados a esta sesión, sin contar el propio.
///
/// En una sala lo normal es llamarlo desde el anfitrión: los invitados sólo
/// están conectados a él y siempre ven `1`.
///
/// # Safety
/// `handle` debe ser un handle válido de `gs_session_create`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_peer_count(handle: *mut GsSessionHandle) -> c_int {
    if handle.is_null() {
        return 0;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let session = inner.session.clone();
    let count = inner
        .runtime
        .block_on(async move { session.peer_count().await });
    c_int::try_from(count).unwrap_or(c_int::MAX)
}

// ─── Tonos de turno ────────────────────────────────────────────────────

/// Tono de "empieza un turno" para `gs_tone_pcm` y `gs_audio_play_tone`.
pub const GS_TONE_TURN_START: c_int = 0;
/// Tono de "termina el turno".
pub const GS_TONE_TURN_END: c_int = 1;

/// Escribe en `out` las muestras (PCM 16 bits, mono) de un tono de turno.
///
/// Es para embebedores que ponen su propio audio (móvil): el tono se
/// reproduce en local, no se envía. `out` necesita sitio para 85 ms de audio
/// a `sample_rate`; `out_len` recibe las muestras escritas.
///
/// # Safety
/// `out` debe apuntar a `out_cap` muestras escribibles y `out_len` a un
/// `size_t` escribible.
#[no_mangle]
pub unsafe extern "C" fn gs_tone_pcm(
    kind: c_int,
    sample_rate: u32,
    out: *mut i16,
    out_cap: usize,
    out_len: *mut usize,
) -> GsStatus {
    if out.is_null() || out_len.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    if !(8_000..=48_000).contains(&sample_rate) {
        return GsStatus::GS_ERR_INVALID_ARGUMENT;
    }
    let tone = match kind {
        GS_TONE_TURN_START => gravital_talk::tone::turn_start_tone(sample_rate),
        GS_TONE_TURN_END => gravital_talk::tone::turn_end_tone(sample_rate),
        _ => return GsStatus::GS_ERR_INVALID_ARGUMENT,
    };
    if tone.len() > out_cap {
        return GsStatus::GS_ERR_BUFFER_TOO_SMALL;
    }
    // SAFETY: `out` tiene `out_cap` muestras escribibles, se acaba de
    // comprobar que el tono cabe, y `out_len` es válido (contrato).
    unsafe {
        ptr::copy_nonoverlapping(tone.as_ptr(), out, tone.len());
        *out_len = tone.len();
    }
    GsStatus::GS_OK
}

/// Suena un tono de turno por el altavoz del audio nativo, mezclado con lo
/// que esté sonando. `GS_ERR_INVALID_STATE` si el audio nativo no está en
/// marcha o no hay altavoz.
///
/// # Safety
/// `handle` debe ser un handle válido de `gs_session_create`.
#[no_mangle]
pub unsafe extern "C" fn gs_audio_play_tone(handle: *mut GsSessionHandle, kind: c_int) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    if kind != GS_TONE_TURN_START && kind != GS_TONE_TURN_END {
        return GsStatus::GS_ERR_INVALID_ARGUMENT;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let played = inner
        .audio()
        .as_ref()
        .is_some_and(|engine| engine.play_tone(kind == GS_TONE_TURN_END));
    if played {
        GsStatus::GS_OK
    } else {
        GsStatus::GS_ERR_INVALID_STATE
    }
}

// ─── Contactos ─────────────────────────────────────────────────────────

/// Lee `len` bytes de `ptr`, o `None` si el puntero es nulo.
///
/// # Safety
/// Si `ptr` no es nulo debe apuntar a `len` bytes legibles.
const unsafe fn bytes_arg<'a>(ptr: *const u8, len: usize) -> Option<&'a [u8]> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: `ptr` no es nulo y apunta a `len` bytes legibles (contrato).
    Some(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// Copia `data` a `out` y deja su tamaño en `out_len`.
///
/// # Safety
/// `out` debe apuntar a `out_cap` bytes escribibles y `out_len` ser válido.
unsafe fn write_out(data: &[u8], out: *mut u8, out_cap: usize, out_len: *mut usize) -> GsStatus {
    if out.is_null() || out_len.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    if data.len() > out_cap {
        return GsStatus::GS_ERR_BUFFER_TOO_SMALL;
    }
    // SAFETY: `out` tiene `out_cap` bytes escribibles, se acaba de comprobar
    // que `data` cabe, y `out_len` es un puntero válido (contrato).
    unsafe {
        ptr::copy_nonoverlapping(data.as_ptr(), out, data.len());
        *out_len = data.len();
    }
    GsStatus::GS_OK
}

/// Identificador público (8 bytes) del contacto que comparte `secret`.
///
/// # Safety
/// `secret` debe apuntar a `secret_len` bytes y `out_id` a 8 bytes escribibles.
#[no_mangle]
pub unsafe extern "C" fn gs_contact_id(
    secret: *const u8,
    secret_len: usize,
    out_id: *mut u8,
) -> GsStatus {
    // SAFETY: mismos requisitos que esta función (contrato).
    let Some(secret) = (unsafe { bytes_arg(secret, secret_len) }) else {
        return GsStatus::GS_ERR_NULL_POINTER;
    };
    if out_id.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let id = gravital_talk::knock::contact_id(secret);
    // SAFETY: `out_id` no es nulo y tiene 8 bytes escribibles (contrato).
    unsafe { ptr::copy_nonoverlapping(id.as_ptr(), out_id, id.len()) };
    GsStatus::GS_OK
}

/// Sella una llamada para el contacto que comparte `secret`.
///
/// El resultado es un datagrama listo para enviar por UDP; sólo quien tenga
/// el mismo secreto puede abrirlo con `gs_knock_open`. `out` necesita
/// `payload_len + 41` bytes.
///
/// # Safety
/// `secret` y `payload` deben apuntar a sus longitudes; `out` a `out_cap`
/// bytes escribibles y `out_len` a un `size_t` escribible.
#[no_mangle]
pub unsafe extern "C" fn gs_knock_seal(
    secret: *const u8,
    secret_len: usize,
    payload: *const u8,
    payload_len: usize,
    out: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> GsStatus {
    // SAFETY: mismos requisitos que esta función (contrato).
    let (Some(secret), Some(payload)) = (unsafe {
        (
            bytes_arg(secret, secret_len),
            bytes_arg(payload, payload_len),
        )
    }) else {
        return GsStatus::GS_ERR_NULL_POINTER;
    };
    gravital_talk::knock::seal(secret, payload).map_or_else(
        |_| {
            set_last_error("knock: el contenido no cabe en un datagrama");
            GsStatus::GS_ERR_INVALID_ARGUMENT
        },
        // SAFETY: `out`, `out_cap` y `out_len` cumplen el contrato.
        |sealed| unsafe { write_out(&sealed, out, out_cap, out_len) },
    )
}

/// Abre una llamada recibida. `GS_ERR_PROTOCOL` si no viene de quien
/// comparte `secret` o ha sido alterada.
///
/// # Safety
/// `secret` y `datagram` deben apuntar a sus longitudes; `out` a `out_cap`
/// bytes escribibles y `out_len` a un `size_t` escribible.
#[no_mangle]
pub unsafe extern "C" fn gs_knock_open(
    secret: *const u8,
    secret_len: usize,
    datagram: *const u8,
    datagram_len: usize,
    out: *mut u8,
    out_cap: usize,
    out_len: *mut usize,
) -> GsStatus {
    // SAFETY: mismos requisitos que esta función (contrato).
    let (Some(secret), Some(datagram)) = (unsafe {
        (
            bytes_arg(secret, secret_len),
            bytes_arg(datagram, datagram_len),
        )
    }) else {
        return GsStatus::GS_ERR_NULL_POINTER;
    };
    gravital_talk::knock::open(secret, datagram).map_or(
        GsStatus::GS_ERR_PROTOCOL,
        // SAFETY: `out`, `out_cap` y `out_len` cumplen el contrato.
        |plain| unsafe { write_out(&plain, out, out_cap, out_len) },
    )
}

// ─── Diagnóstico ───────────────────────────────────────────────────────

/// Fichero donde se anota un pánico antes de que el proceso muera.
static CRASH_LOG: Mutex<Option<std::path::PathBuf>> = Mutex::new(None);
static CRASH_HOOK: std::sync::Once = std::sync::Once::new();

/// Fija el fichero donde dejar constancia de un fallo interno.
///
/// La librería se compila con `panic = "abort"`: un pánico mata el proceso
/// que la carga, y en un móvil eso se ve como "la app se cierra sola", sin
/// más pista. Con esto, antes de morir se añade al fichero una línea con el
/// lugar y el mensaje, para que el embebedor la enseñe en el siguiente
/// arranque. `path` nulo deja de anotar.
///
/// # Safety
/// `path` debe ser nulo o una C-string NUL-terminada válida.
#[no_mangle]
pub unsafe extern "C" fn gs_crash_log_set_path(path: *const c_char) -> GsStatus {
    let target = if path.is_null() {
        None
    } else {
        // SAFETY: `path` no es nulo y es una C-string válida (contrato).
        match unsafe { CStr::from_ptr(path) }.to_str() {
            Ok(s) if !s.is_empty() => Some(std::path::PathBuf::from(s)),
            _ => {
                set_last_error("crash log: ruta no válida");
                return GsStatus::GS_ERR_INVALID_ARGUMENT;
            }
        }
    };
    *CRASH_LOG.lock().unwrap_or_else(|p| p.into_inner()) = target;
    CRASH_HOOK.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            record_panic(info);
            previous(info);
        }));
    });
    GsStatus::GS_OK
}

fn record_panic(info: &std::panic::PanicHookInfo<'_>) {
    use std::io::Write;

    // `try_lock`: si el pánico saltó con el cerrojo tomado, mejor perder la
    // nota que quedarse colgado en vez de morir.
    let Ok(guard) = CRASH_LOG.try_lock() else {
        return;
    };
    let Some(path) = guard.as_ref() else {
        return;
    };
    let payload = info.payload();
    let message = payload
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("(sin mensaje)");
    let place = info.location().map_or_else(
        || "lugar desconocido".to_owned(),
        |l| format!("{}:{}", l.file(), l.line()),
    );
    let thread = std::thread::current();
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(
            file,
            "gravital-talk {} · hilo {} · {place} · {message}",
            env!("CARGO_PKG_VERSION"),
            thread.name().unwrap_or("sin nombre"),
        );
    }
}

// ─── Códec de voz ──────────────────────────────────────────────────────

/// Activa (`on != 0`) o desactiva la reducción de ruido del micrófono.
///
/// Activa por defecto. Sólo actúa cuando la sesión viaja comprimida (ver
/// `gs_session_codec`) a 48 kHz.
///
/// # Safety
/// `handle` debe ser un handle válido de `gs_session_create`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_set_noise_suppression(
    handle: *mut GsSessionHandle,
    on: c_int,
) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    inner.pipeline.set_noise_suppression(on != 0);
    GsStatus::GS_OK
}

/// Fija el tope de la tasa de bits del audio, en bits por segundo.
///
/// Se acota a 16 000–128 000. La tasa real baja por debajo del tope si la red
/// pierde paquetes y vuelve a subir cuando se recupera.
///
/// # Safety
/// `handle` debe ser un handle válido de `gs_session_create`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_set_bitrate(
    handle: *mut GsSessionHandle,
    bitrate: u32,
) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    inner.pipeline.set_bitrate_cap(bitrate);
    GsStatus::GS_OK
}

/// Tasa de bits a la que se está enviando el audio ahora mismo, o `0` si
/// viaja PCM sin comprimir (o aún no se ha enviado nada).
///
/// # Safety
/// `handle` debe ser un handle válido de `gs_session_create`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_bitrate(handle: *mut GsSessionHandle) -> u32 {
    if handle.is_null() {
        return 0;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    inner.pipeline.current_bitrate()
}

/// Códec negociado para la sesión: `1` = PCM sin comprimir, `2` = Opus,
/// `0` = aún sin negociar.
///
/// Con Opus, `gs_session_send_audio` y `gs_session_recv_audio` siguen
/// trabajando en PCM: la compresión ocurre dentro. Un `recv` puede devolver
/// más de un frame (hasta cuatro) cuando reconstruye audio perdido.
///
/// # Safety
/// `handle` debe ser un handle válido de `gs_session_create`.
#[no_mangle]
pub unsafe extern "C" fn gs_session_codec(handle: *mut GsSessionHandle) -> c_int {
    if handle.is_null() {
        return 0;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    if inner.pipeline.compressed() {
        2
    } else {
        c_int::from(inner.session.negotiated_codec().min(1))
    }
}

// ─── Audio nativo ──────────────────────────────────────────────────────

/// Estado del audio nativo de una sesión.
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct GsAudioStats {
    /// Nivel del micrófono, 0..1. Cero si no se está transmitiendo.
    pub mic_level: f32,
    /// Nivel del audio recibido, 0..1.
    pub rx_level: f32,
    /// Frames enviados desde `gs_audio_start`.
    pub tx_frames: u64,
    /// Frames recibidos desde `gs_audio_start`.
    pub rx_frames: u64,
    /// `1` si hay micrófono abierto.
    pub capture_ok: u8,
    /// `1` si hay salida de audio abierta.
    pub playback_ok: u8,
}

/// Devuelve `1` si este build trae audio nativo (`gs_audio_start` funciona).
///
/// En Android además exige que la app haya entregado su contexto al arrancar.
#[no_mangle]
pub extern "C" fn gs_audio_supported() -> c_int {
    c_int::from(cfg!(feature = "audio") && audio::platform_ready())
}

/// Arranca el audio nativo de la sesión: micrófono → red y red → altavoz.
///
/// Llamar con la sesión ya en `Active`. Mientras está arrancado, el embebedor
/// **no** debe llamar a `gs_session_send_audio` ni `gs_session_recv_audio`:
/// competiría con estos hilos por los mismos frames.
///
/// Que falte el micrófono o el altavoz no es un error: la sesión sigue,
/// `gs_audio_stats` dice cuál de los dos falta y `gs_error_last` por qué. Devuelve `GS_ERR_UNSUPPORTED`
/// si el build no trae audio nativo y `GS_ERR_INVALID_ARGUMENT` si un frame de
/// la configuración no cabe en un paquete.
///
/// # Safety
/// `handle` debe ser un handle válido de `gs_session_create`.
#[no_mangle]
pub unsafe extern "C" fn gs_audio_start(handle: *mut GsSessionHandle) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    audio_start(inner)
}

#[cfg(feature = "audio")]
fn audio_start(inner: &SessionInner) -> GsStatus {
    if !audio::platform_ready() {
        set_last_error("audio: la plataforma no ha inicializado el audio nativo");
        return GsStatus::GS_ERR_UNSUPPORTED;
    }
    if inner.audio().is_some() {
        return GsStatus::GS_OK;
    }
    // El motor se arranca sin el cerrojo cogido: abrir los devices tarda, y
    // `gs_audio_stats` no debe quedarse esperando mientras tanto.
    match audio::Engine::start(
        inner.runtime.clone(),
        inner.pipeline.clone(),
        inner.config.sample_rate,
        inner.config.channels,
        inner.config.frame_duration_ms,
    ) {
        Ok(engine) => {
            // Arrancar sin un device no es un error, pero el motivo se deja en
            // `gs_error_last` para quien quiera mostrarlo.
            if let Some(problems) = engine.shared().problems() {
                set_last_error(format!("audio: {problems}"));
            }
            *inner.audio() = Some(engine);
            GsStatus::GS_OK
        }
        Err(e) => {
            set_last_error(format!("audio: {e}"));
            GsStatus::GS_ERR_INVALID_ARGUMENT
        }
    }
}

#[cfg(not(feature = "audio"))]
fn audio_start(inner: &SessionInner) -> GsStatus {
    let _ = (&inner.config, &inner.audio);
    set_last_error("audio: este build no trae audio nativo");
    GsStatus::GS_ERR_UNSUPPORTED
}

/// Para el audio nativo. No hace nada si no estaba arrancado.
///
/// # Safety
/// `handle` debe ser un handle válido de `gs_session_create`.
#[no_mangle]
pub unsafe extern "C" fn gs_audio_stop(handle: *mut GsSessionHandle) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    // Se saca del mutex antes de soltarlo: el `Drop` espera a los hilos y no
    // debe hacerlo con el cerrojo cogido.
    let engine = inner.audio().take();
    drop(engine);
    GsStatus::GS_OK
}

/// Abre (`on != 0`) o cierra el paso del micrófono a la red.
///
/// Es independiente de `gs_session_ptt_press`, que señaliza el turno al peer:
/// esta función sólo decide si los frames capturados se envían.
///
/// # Safety
/// `handle` debe ser un handle válido de `gs_session_create`.
#[no_mangle]
pub unsafe extern "C" fn gs_audio_set_ptt(handle: *mut GsSessionHandle, on: c_int) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    inner
        .audio()
        .as_ref()
        .map_or(GsStatus::GS_ERR_INVALID_STATE, |engine| {
            engine.shared().set_ptt(on != 0);
            GsStatus::GS_OK
        })
}

/// Pausa (`on != 0`) o reanuda la escucha: con la pausa puesta el audio
/// recibido no suena, pero la sesión sigue viva y al día.
///
/// # Safety
/// `handle` debe ser un handle válido de `gs_session_create`.
#[no_mangle]
pub unsafe extern "C" fn gs_audio_set_muted(handle: *mut GsSessionHandle, on: c_int) -> GsStatus {
    if handle.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    inner
        .audio()
        .as_ref()
        .map_or(GsStatus::GS_ERR_INVALID_STATE, |engine| {
            engine.shared().set_muted(on != 0);
            GsStatus::GS_OK
        })
}

/// Lee el estado del audio nativo. Con el audio parado escribe todo a cero.
///
/// # Safety
/// `handle` debe ser válido y `out` apuntar a un `GsAudioStats` escribible.
#[no_mangle]
pub unsafe extern "C" fn gs_audio_stats(
    handle: *mut GsSessionHandle,
    out: *mut GsAudioStats,
) -> GsStatus {
    if handle.is_null() || out.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    // SAFETY: `handle` no es nulo y viene de `gs_session_create` (contrato).
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let stats = inner
        .audio()
        .as_ref()
        .map(|e| e.shared().stats())
        .unwrap_or_default();
    // SAFETY: `out` no es nulo y apunta a un `GsAudioStats` escribible.
    unsafe {
        *out = GsAudioStats {
            mic_level: stats.mic_level,
            rx_level: stats.rx_level,
            tx_frames: stats.tx_frames,
            rx_frames: stats.rx_frames,
            capture_ok: u8::from(stats.capture_ok),
            playback_ok: u8::from(stats.playback_ok),
        };
    }
    GsStatus::GS_OK
}

/// Retorna `GS_OK` — útil como smoke test de linkado.
#[no_mangle]
pub const extern "C" fn gs_ping() -> c_int {
    0
}

#[cfg(feature = "android")]
pub mod jni_bridge;

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::CString;

    #[test]
    fn version_is_nul_terminated() {
        let p = gs_version();
        assert!(!p.is_null());
        let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap();
        assert!(s.chars().next().unwrap().is_ascii_digit());
    }

    #[test]
    fn protocol_version_nonzero() {
        assert_eq!(gs_protocol_version(), 1);
    }

    #[test]
    fn default_config_is_sane() {
        let mut cfg = GsConfig {
            sample_rate: 0,
            channels: 0,
            frame_duration_ms: 0,
            max_bitrate: 0,
            codec_preferred: 0,
            capability_flags: 0,
            jitter_buffer_ms: 0,
            mtu: 0,
        };
        let st = unsafe { gs_config_default(&mut cfg) };
        assert_eq!(st, GsStatus::GS_OK);
        assert!(cfg.sample_rate >= 8000);
        assert!(cfg.mtu >= 576);
    }

    #[test]
    fn null_args_return_null_pointer_error() {
        let st = unsafe { gs_config_default(std::ptr::null_mut()) };
        assert_eq!(st, GsStatus::GS_ERR_NULL_POINTER);
    }

    #[test]
    fn create_and_destroy_session_handle() {
        let mut cfg = GsConfig {
            sample_rate: 48000,
            channels: 1,
            frame_duration_ms: 20,
            max_bitrate: 64000,
            codec_preferred: 1,
            capability_flags: 0,
            jitter_buffer_ms: 40,
            mtu: 1200,
        };
        unsafe { gs_config_default(&mut cfg) };
        let bind = CString::new("127.0.0.1").unwrap();
        let mut handle: *mut GsSessionHandle = std::ptr::null_mut();
        let st = unsafe { gs_session_create(&cfg, bind.as_ptr(), 0, &mut handle) };
        assert_eq!(st, GsStatus::GS_OK);
        assert!(!handle.is_null());

        let mut state = GsSessionState::GS_STATE_IDLE;
        let st = unsafe { gs_session_state(handle, &mut state) };
        assert_eq!(st, GsStatus::GS_OK);
        assert_eq!(state, GsSessionState::GS_STATE_IDLE);

        unsafe { gs_session_destroy(handle) };
    }

    #[test]
    fn las_estadisticas_de_audio_son_cero_con_el_audio_parado() {
        let mut cfg = GsConfig {
            sample_rate: 0,
            channels: 0,
            frame_duration_ms: 0,
            max_bitrate: 0,
            codec_preferred: 0,
            capability_flags: 0,
            jitter_buffer_ms: 0,
            mtu: 0,
        };
        unsafe { gs_config_default(&mut cfg) };
        let bind = CString::new("127.0.0.1").unwrap();
        let mut handle: *mut GsSessionHandle = std::ptr::null_mut();
        assert_eq!(
            unsafe { gs_session_create(&cfg, bind.as_ptr(), 0, &mut handle) },
            GsStatus::GS_OK
        );

        let mut stats = GsAudioStats {
            rx_frames: 99,
            ..Default::default()
        };
        assert_eq!(
            unsafe { gs_audio_stats(handle, &mut stats) },
            GsStatus::GS_OK
        );
        assert_eq!(stats.rx_frames, 0);
        // Sin audio arrancado no hay PTT que abrir, y parar es inocuo.
        assert_eq!(
            unsafe { gs_audio_set_ptt(handle, 1) },
            GsStatus::GS_ERR_INVALID_STATE
        );
        assert_eq!(
            unsafe { gs_audio_set_muted(handle, 1) },
            GsStatus::GS_ERR_INVALID_STATE
        );
        assert_eq!(unsafe { gs_audio_stop(handle) }, GsStatus::GS_OK);

        unsafe { gs_session_destroy(handle) };
    }

    /// Dos sesiones FFI conectadas por loopback con la configuración dada.
    fn connected_pair(cfg: &GsConfig) -> (*mut GsSessionHandle, *mut GsSessionHandle) {
        let bind = CString::new("127.0.0.1").unwrap();
        let (mut host, mut guest) = (std::ptr::null_mut(), std::ptr::null_mut());
        unsafe {
            assert_eq!(
                gs_session_create(cfg, bind.as_ptr(), 0, &mut host),
                GsStatus::GS_OK
            );
            assert_eq!(
                gs_session_create(cfg, bind.as_ptr(), 0, &mut guest),
                GsStatus::GS_OK
            );
        }
        let mut port = 0u16;
        unsafe { gs_session_local_port(host, &mut port) };

        let host_addr = host as usize;
        let accepting = std::thread::spawn(move || unsafe {
            gs_session_accept_any(host_addr as *mut GsSessionHandle)
        });
        let st = unsafe { gs_session_connect(guest, bind.as_ptr(), port) };
        assert_eq!(st, GsStatus::GS_OK);
        assert_eq!(accepting.join().unwrap(), GsStatus::GS_OK);
        (host, guest)
    }

    fn voice_config(codec: u8) -> GsConfig {
        GsConfig {
            sample_rate: 48_000,
            channels: 1,
            frame_duration_ms: 20,
            max_bitrate: 64_000,
            codec_preferred: codec,
            capability_flags: 0,
            jitter_buffer_ms: 60,
            mtu: 1200,
        }
    }

    /// Un frame de 20 ms de tono a 440 Hz, en PCM16 LE.
    fn tone_frame(phase: &mut f32) -> Vec<u8> {
        let mut out = Vec::with_capacity(1920);
        for _ in 0..960 {
            let s = (phase.sin() * 9000.0) as i16;
            *phase += core::f32::consts::TAU * 440.0 / 48_000.0;
            out.extend_from_slice(&s.to_le_bytes());
        }
        out
    }

    fn rms(pcm: &[u8]) -> f64 {
        let samples: Vec<f64> = pcm
            .as_chunks::<2>()
            .0
            .iter()
            .map(|b| f64::from(i16::from_le_bytes(*b)))
            .collect();
        (samples.iter().map(|s| s * s).sum::<f64>() / samples.len().max(1) as f64).sqrt()
    }

    #[test]
    fn los_tonos_de_turno_se_entregan_como_pcm() {
        let mut out = [0i16; 48 * 85];
        let mut len = 0usize;
        // SAFETY: `out` y `len` viven durante las llamadas.
        unsafe {
            assert_eq!(
                gs_tone_pcm(
                    GS_TONE_TURN_START,
                    48_000,
                    out.as_mut_ptr(),
                    out.len(),
                    &mut len
                ),
                GsStatus::GS_OK
            );
            assert_eq!(len, 48 * 40);
            assert!(out[..len].iter().any(|&s| s.abs() > 4_000));
            assert_eq!(
                gs_tone_pcm(
                    GS_TONE_TURN_END,
                    48_000,
                    out.as_mut_ptr(),
                    out.len(),
                    &mut len
                ),
                GsStatus::GS_OK
            );
            assert_eq!(len, 48 * 85);
            // Sin sitio, sin tono conocido o con una frecuencia absurda.
            assert_eq!(
                gs_tone_pcm(GS_TONE_TURN_END, 48_000, out.as_mut_ptr(), 10, &mut len),
                GsStatus::GS_ERR_BUFFER_TOO_SMALL
            );
            assert_eq!(
                gs_tone_pcm(7, 48_000, out.as_mut_ptr(), out.len(), &mut len),
                GsStatus::GS_ERR_INVALID_ARGUMENT
            );
            assert_eq!(
                gs_tone_pcm(GS_TONE_TURN_START, 1, out.as_mut_ptr(), out.len(), &mut len),
                GsStatus::GS_ERR_INVALID_ARGUMENT
            );
        }
    }

    #[test]
    fn una_llamada_sellada_solo_la_abre_el_contacto() {
        let (secret, other) = (b"token-de-sala", b"otro-token");
        let payload = b"gravital-talk://pair?room=GRVT-1234";
        let mut sealed = [0u8; 256];
        let mut sealed_len = 0usize;
        let mut plain = [0u8; 256];
        let mut plain_len = 0usize;
        // SAFETY: todos los punteros apuntan a buffers vivos del tamaño dado.
        unsafe {
            assert_eq!(
                gs_knock_seal(
                    secret.as_ptr(),
                    secret.len(),
                    payload.as_ptr(),
                    payload.len(),
                    sealed.as_mut_ptr(),
                    sealed.len(),
                    &mut sealed_len,
                ),
                GsStatus::GS_OK
            );
            assert_eq!(
                gs_knock_open(
                    secret.as_ptr(),
                    secret.len(),
                    sealed.as_ptr(),
                    sealed_len,
                    plain.as_mut_ptr(),
                    plain.len(),
                    &mut plain_len,
                ),
                GsStatus::GS_OK
            );
            assert_eq!(&plain[..plain_len], payload);
            assert_eq!(
                gs_knock_open(
                    other.as_ptr(),
                    other.len(),
                    sealed.as_ptr(),
                    sealed_len,
                    plain.as_mut_ptr(),
                    plain.len(),
                    &mut plain_len,
                ),
                GsStatus::GS_ERR_PROTOCOL
            );
            let (mut a, mut b) = ([0u8; 8], [0u8; 8]);
            assert_eq!(
                gs_contact_id(secret.as_ptr(), secret.len(), a.as_mut_ptr()),
                GsStatus::GS_OK
            );
            assert_eq!(
                gs_contact_id(other.as_ptr(), other.len(), b.as_mut_ptr()),
                GsStatus::GS_OK
            );
            assert_ne!(a, b);
            assert_eq!(sealed[5..13], a);
        }
    }

    #[test]
    fn un_panico_deja_constancia_en_el_fichero() {
        let path = std::env::temp_dir().join(format!("gs-crash-{}.txt", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let c_path = CString::new(path.to_str().unwrap()).unwrap();
        // SAFETY: `c_path` es una C-string válida durante la llamada.
        assert_eq!(
            unsafe { gs_crash_log_set_path(c_path.as_ptr()) },
            GsStatus::GS_OK
        );
        let _ = std::thread::Builder::new()
            .name("gs-prueba".into())
            .spawn(|| panic!("fallo de prueba {}", 42))
            .unwrap()
            .join();
        let noted = std::fs::read_to_string(&path).unwrap();
        // SAFETY: nulo es un argumento válido: deja de anotar.
        unsafe { gs_crash_log_set_path(ptr::null()) };
        let _ = std::fs::remove_file(&path);
        assert!(noted.contains("fallo de prueba 42"), "{noted}");
        assert!(noted.contains("gs-prueba"), "{noted}");
        assert!(noted.contains("lib.rs"), "{noted}");
    }

    #[test]
    fn con_opus_negociado_el_embebedor_sigue_viendo_pcm() {
        let cfg = voice_config(2);
        let (host, guest) = connected_pair(&cfg);
        assert_eq!(unsafe { gs_session_codec(guest) }, 2);
        assert_eq!(unsafe { gs_session_codec(host) }, 2);
        // Con un tono puro el reductor de ruido lo trataría como ruido.
        unsafe { gs_session_set_noise_suppression(guest, 0) };

        let mut phase = 0.0;
        let mut buf = vec![0u8; 8192];
        let mut level = 0.0;
        let before = unsafe {
            let mut m = GsMetrics::default();
            gs_session_metrics(guest, &mut m);
            m.bytes_sent
        };
        for i in 0..40 {
            let frame = tone_frame(&mut phase);
            assert_eq!(
                unsafe { gs_session_send_audio(guest, frame.as_ptr(), frame.len()) },
                GsStatus::GS_OK
            );
            let mut len = buf.len();
            let st =
                unsafe { gs_session_recv_audio_timeout(host, buf.as_mut_ptr(), &mut len, 1000) };
            assert_eq!(st, GsStatus::GS_OK, "frame {i}");
            // Lo que sale es PCM del mismo tamaño que entró.
            assert_eq!(len, 1920, "frame {i}");
            if i >= 20 {
                level = rms(&buf[..len]);
            }
        }
        assert!((4000.0..9000.0).contains(&level), "rms {level}");

        // Y por la red ha viajado una fracción: 40 frames PCM son 76 800 B.
        let sent = unsafe {
            let mut m = GsMetrics::default();
            gs_session_metrics(guest, &mut m);
            m.bytes_sent - before
        };
        assert!(sent < 20_000, "bytes enviados por 40 frames: {sent}");
        assert!(unsafe { gs_session_bitrate(guest) } <= 64_000);

        unsafe {
            gs_session_destroy(host);
            gs_session_destroy(guest);
        }
    }

    #[test]
    fn un_peer_que_pide_pcm_recibe_pcm_aunque_el_frame_no_quepa_en_un_paquete() {
        // 20 ms de PCM son 1920 B y el tope por paquete es 1156 B. Antes eso
        // hacía fallar cada envío; ahora se parte en dos y llega entero.
        let cfg = voice_config(1);
        let (host, guest) = connected_pair(&cfg);
        assert_eq!(unsafe { gs_session_codec(guest) }, 1);

        let mut phase = 0.0;
        let frame = tone_frame(&mut phase);
        assert_eq!(
            unsafe { gs_session_send_audio(guest, frame.as_ptr(), frame.len()) },
            GsStatus::GS_OK
        );
        let mut received = Vec::new();
        let mut buf = vec![0u8; 8192];
        while received.len() < frame.len() {
            let mut len = buf.len();
            let st =
                unsafe { gs_session_recv_audio_timeout(host, buf.as_mut_ptr(), &mut len, 1000) };
            assert_eq!(st, GsStatus::GS_OK);
            received.extend_from_slice(&buf[..len]);
        }
        assert_eq!(received, frame);

        unsafe {
            gs_session_destroy(host);
            gs_session_destroy(guest);
        }
    }

    #[test]
    fn la_ip_local_no_lleva_puerto() {
        let mut buf = [0 as c_char; 64];
        let st = unsafe { gs_discover_local_addr(buf.as_mut_ptr(), buf.len()) };
        // Sin red no hay IP que dar; con red, tiene que ser una IP pelada.
        if st == GsStatus::GS_OK {
            let s = unsafe { CStr::from_ptr(buf.as_ptr()) }.to_str().unwrap();
            assert!(s.parse::<IpAddr>().is_ok(), "no es una IP: {s}");
        }
    }
}
