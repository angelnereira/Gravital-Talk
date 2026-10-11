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
    let inner = Box::new(SessionInner {
        audio: Mutex::new(None),
        runtime: Arc::new(runtime),
        session,
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
    let session = inner.session.clone();
    let payload = slice.to_vec();
    match inner
        .runtime
        .block_on(async move { session.send_audio(&payload).await })
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
    let session = inner.session.clone();

    let recv = async move {
        match timeout_ms {
            Some(ms) => {
                let dur = std::time::Duration::from_millis(u64::from(ms));
                tokio::time::timeout(dur, session.recv_audio()).await
            }
            None => Ok(session.recv_audio().await),
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
    let n = frame.payload.len();
    if n > cap {
        unsafe { *len_inout = n };
        return GsStatus::GS_ERR_BUFFER_TOO_SMALL;
    }
    unsafe {
        ptr::copy_nonoverlapping(frame.payload.as_ptr(), buf, n);
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
#[no_mangle]
pub const extern "C" fn gs_audio_supported() -> c_int {
    cfg!(feature = "audio") as c_int
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
    if inner.audio().is_some() {
        return GsStatus::GS_OK;
    }
    // El motor se arranca sin el cerrojo cogido: abrir los devices tarda, y
    // `gs_audio_stats` no debe quedarse esperando mientras tanto.
    match audio::Engine::start(
        inner.runtime.clone(),
        inner.session.clone(),
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
        assert_eq!(unsafe { gs_audio_stop(handle) }, GsStatus::GS_OK);

        unsafe { gs_session_destroy(handle) };
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
