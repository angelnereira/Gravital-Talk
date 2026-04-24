//! Gravital Sound — superficie FFI C.
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
use std::sync::Arc;

use gravital_sound::{
    Config as RustConfig, LatencyClass, MetricsSnapshot, Session, SessionRole, SessionState,
    UdpConfig, UdpTransport,
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
    runtime: tokio::runtime::Runtime,
    session: Arc<Session>,
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
            capability_flags: c.capability_flags,
            jitter_buffer_ms: c.jitter_buffer_ms,
            mtu: c.mtu as usize,
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
    u32::from(gravital_sound::PROTOCOL_VERSION)
}

/// Versión del ABI C.
#[no_mangle]
pub extern "C" fn gs_abi_version() -> u32 {
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
        Ok(t) => Arc::new(t) as Arc<dyn gravital_sound::Transport>,
        Err(e) => {
            set_last_error(format!("bind failed: {e}"));
            return GsStatus::GS_ERR_IO;
        }
    };

    let session = Arc::new(Session::new(transport, rust_cfg));
    let inner = Box::new(SessionInner { runtime, session });
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
    let _ = unsafe { Box::from_raw(handle as *mut SessionInner) };
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
    if handle.is_null() || buf.is_null() || len_inout.is_null() {
        return GsStatus::GS_ERR_NULL_POINTER;
    }
    let inner = unsafe { &*(handle as *mut SessionInner) };
    let cap = unsafe { *len_inout };
    let session = inner.session.clone();
    let frame = match inner
        .runtime
        .block_on(async move { session.recv_audio().await })
    {
        Ok(f) => f,
        Err(e) => {
            set_last_error(format!("recv_audio: {e}"));
            return GsStatus::GS_ERR_IO;
        }
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
    let session = inner.session.clone();
    let _ = inner.runtime.block_on(async move { session.close().await });
    GsStatus::GS_OK
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

/// Retorna `GS_OK` — útil como smoke test de linkado.
#[no_mangle]
pub extern "C" fn gs_ping() -> c_int {
    0
}

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
}
