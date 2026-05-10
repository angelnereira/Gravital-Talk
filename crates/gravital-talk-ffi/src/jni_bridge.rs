//! Puente JNI para Android.
//!
//! Compilado sólo con la feature `android` (`cargo ndk` lo activa automáticamente
//! si se configura via `.cargo/config.toml`).
//!
//! Clase Java esperada: `com.gravitaltalk.GravitalTalkJni`
//!
//! Cada función JNI recibe el handle de sesión como `jlong` (puntero opaco
//! a `SessionInner` en el heap nativo) y retorna un `jint` con el código de
//! estado `GsStatus`.

#![allow(non_snake_case)]

use jni::objects::{JByteArray, JClass, JFloatArray, JString};
use jni::sys::{jbyteArray, jfloatArray, jint, jlong, jstring};
use jni::JNIEnv;
use std::ffi::CString;

use crate::{
    gs_discover_public_addr, gs_session_accept, gs_session_accept_any, gs_session_close,
    gs_session_connect, gs_session_create, gs_session_destroy, gs_session_id,
    gs_session_is_peer_ptt_active, gs_session_local_port, gs_session_metrics,
    gs_session_ptt_press, gs_session_ptt_release, gs_session_recv_audio, gs_session_send_audio,
    GsConfig, GsMetrics, GsSessionHandle, GsStatus,
};

// ─── Helpers ─────────────────────────────────────────────────────────────────

#[inline]
fn status_jint(s: GsStatus) -> jint {
    s as jint
}

// ─── Funciones JNI ───────────────────────────────────────────────────────────

/// `GravitalTalkJni.nativeVersion(): String`
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativeVersion(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    env.new_string(env!("CARGO_PKG_VERSION"))
        .map(|s| s.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// `GravitalTalkJni.nativeCreate(sampleRate: Int, channels: Int, bindPort: Int): Long`
///
/// Retorna el handle de sesión (puntero nativo) como `Long`, o `0` en error.
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativeCreate(
    _env: JNIEnv,
    _class: JClass,
    sample_rate: jint,
    channels: jint,
    bind_port: jint,
) -> jlong {
    let cfg = GsConfig {
        sample_rate: sample_rate as u32,
        channels: channels as u8,
        frame_duration_ms: 20,
        max_bitrate: 64_000,
        codec_preferred: 0x01, // PCM
        capability_flags: 0,
        jitter_buffer_ms: 60,
        mtu: 1200,
    };
    let bind = match CString::new("0.0.0.0") {
        Ok(s) => s,
        Err(_) => return 0,
    };
    let mut handle: *mut GsSessionHandle = std::ptr::null_mut();
    let status =
        unsafe { gs_session_create(&cfg, bind.as_ptr(), bind_port as u16, &mut handle) };
    if status == GsStatus::GS_OK {
        handle as jlong
    } else {
        0
    }
}

/// `GravitalTalkJni.nativeDestroy(handle: Long)`
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativeDestroy(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) {
    if handle != 0 {
        unsafe { gs_session_destroy(handle as *mut GsSessionHandle) };
    }
}

/// `GravitalTalkJni.nativeConnect(handle: Long, host: String, port: Int): Int`
///
/// Handshake como cliente. Bloqueante hasta completar o fallar.
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativeConnect(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    host: JString,
    port: jint,
) -> jint {
    if handle == 0 {
        return status_jint(GsStatus::GS_ERR_NULL_POINTER);
    }
    let host_str: String = match env.get_string(&host) {
        Ok(s) => s.into(),
        Err(_) => return status_jint(GsStatus::GS_ERR_INVALID_ARGUMENT),
    };
    let c_host = match CString::new(host_str) {
        Ok(s) => s,
        Err(_) => return status_jint(GsStatus::GS_ERR_INVALID_ARGUMENT),
    };
    let st = unsafe {
        gs_session_connect(handle as *mut GsSessionHandle, c_host.as_ptr(), port as u16)
    };
    status_jint(st)
}

/// `GravitalTalkJni.nativeAccept(handle: Long, peerHost: String, peerPort: Int): Int`
///
/// Handshake como servidor. Bloqueante.
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativeAccept(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    peer_host: JString,
    peer_port: jint,
) -> jint {
    if handle == 0 {
        return status_jint(GsStatus::GS_ERR_NULL_POINTER);
    }
    let host_str: String = match env.get_string(&peer_host) {
        Ok(s) => s.into(),
        Err(_) => return status_jint(GsStatus::GS_ERR_INVALID_ARGUMENT),
    };
    let c_host = match CString::new(host_str) {
        Ok(s) => s,
        Err(_) => return status_jint(GsStatus::GS_ERR_INVALID_ARGUMENT),
    };
    let st = unsafe {
        gs_session_accept(handle as *mut GsSessionHandle, c_host.as_ptr(), peer_port as u16)
    };
    status_jint(st)
}

/// `GravitalTalkJni.nativeSendAudio(handle: Long, pcm: ByteArray): Int`
///
/// Envía un frame PCM 16-bit LE, mono 48 kHz, 20 ms.
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativeSendAudio(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    pcm: JByteArray,
) -> jint {
    if handle == 0 {
        return status_jint(GsStatus::GS_ERR_NULL_POINTER);
    }
    let bytes: Vec<u8> = match env.convert_byte_array(&pcm) {
        Ok(b) => b,
        Err(_) => return status_jint(GsStatus::GS_ERR_INVALID_ARGUMENT),
    };
    let st = unsafe {
        gs_session_send_audio(handle as *mut GsSessionHandle, bytes.as_ptr(), bytes.len())
    };
    status_jint(st)
}

/// `GravitalTalkJni.nativeRecvAudio(handle: Long): ByteArray?`
///
/// Bloquea hasta recibir el próximo frame. Retorna `null` en error.
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativeRecvAudio(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jbyteArray {
    if handle == 0 {
        return std::ptr::null_mut();
    }
    let mut buf = vec![0u8; 4096];
    let mut len = buf.len();
    let st =
        unsafe { gs_session_recv_audio(handle as *mut GsSessionHandle, buf.as_mut_ptr(), &mut len) };
    if st != GsStatus::GS_OK {
        return std::ptr::null_mut();
    }
    env.byte_array_from_slice(&buf[..len])
        .map(|a| a.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// `GravitalTalkJni.nativePttPress(handle: Long): Int`
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativePttPress(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    if handle == 0 {
        return status_jint(GsStatus::GS_ERR_NULL_POINTER);
    }
    status_jint(unsafe { gs_session_ptt_press(handle as *mut GsSessionHandle) })
}

/// `GravitalTalkJni.nativePttRelease(handle: Long): Int`
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativePttRelease(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    if handle == 0 {
        return status_jint(GsStatus::GS_ERR_NULL_POINTER);
    }
    status_jint(unsafe { gs_session_ptt_release(handle as *mut GsSessionHandle) })
}

/// `GravitalTalkJni.nativeIsPeerPttActive(handle: Long): Int` — 1 si activo, 0 si no.
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativeIsPeerPttActive(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    if handle == 0 {
        return 0;
    }
    unsafe { gs_session_is_peer_ptt_active(handle as *mut GsSessionHandle) }
}

/// `GravitalTalkJni.nativeGetMetrics(handle: Long): FloatArray`
///
/// Retorna `[rtt_ms, jitter_ms, loss_percent, estimated_mos]`.
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativeGetMetrics(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jfloatArray {
    let empty =
        || env.new_float_array(0).map(|a: JFloatArray| a.into_raw()).unwrap_or(std::ptr::null_mut());
    if handle == 0 {
        return empty();
    }
    let mut m = GsMetrics::default();
    if unsafe { gs_session_metrics(handle as *mut GsSessionHandle, &mut m) } != GsStatus::GS_OK {
        return empty();
    }
    match env.new_float_array(4) {
        Ok(arr) => {
            let vals = [m.rtt_ms, m.jitter_ms, m.loss_percent, m.estimated_mos];
            let _ = env.set_float_array_region(&arr, 0, &vals);
            arr.into_raw()
        }
        Err(_) => empty(),
    }
}

/// `GravitalTalkJni.nativeGetSessionId(handle: Long): Int`
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativeGetSessionId(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    if handle == 0 {
        return 0;
    }
    let mut id: u32 = 0;
    let st = unsafe { gs_session_id(handle as *mut GsSessionHandle, &mut id) };
    if st == GsStatus::GS_OK { id as jint } else { 0 }
}

/// `GravitalTalkJni.nativeClose(handle: Long): Int`
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativeClose(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    if handle == 0 {
        return status_jint(GsStatus::GS_ERR_NULL_POINTER);
    }
    status_jint(unsafe { gs_session_close(handle as *mut GsSessionHandle) })
}

/// `GravitalTalkJni.nativeGetLocalPort(handle: Long): Int`
///
/// Retorna el puerto UDP local del socket (>0), o 0 en error.
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativeGetLocalPort(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    if handle == 0 {
        return 0;
    }
    let mut port: u16 = 0;
    let st = unsafe { gs_session_local_port(handle as *mut GsSessionHandle, &mut port) };
    if st == GsStatus::GS_OK { port as jint } else { 0 }
}

/// `GravitalTalkJni.nativeDiscoverPublicAddr(bindPort: Int): String?`
///
/// Descubre la IP pública via STUN. Retorna `"ip:port"` o `null` si falla.
/// Bloqueante (~5 s de timeout por servidor).
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativeDiscoverPublicAddr(
    mut env: JNIEnv,
    _class: JClass,
    bind_port: jint,
) -> jstring {
    let mut buf = [0i8; 64];
    let st = unsafe {
        gs_discover_public_addr(bind_port as u16, buf.as_mut_ptr(), buf.len())
    };
    if st != GsStatus::GS_OK {
        return std::ptr::null_mut();
    }
    // Convertir C-string a Java String.
    let c_str = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
    let s = match c_str.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    env.new_string(s)
        .map(|js| js.into_raw())
        .unwrap_or(std::ptr::null_mut())
}

/// `GravitalTalkJni.nativeAcceptAny(handle: Long): Int`
///
/// Handshake servidor que acepta el primer cliente de cualquier dirección.
/// Bloqueante hasta completar o error.
#[no_mangle]
pub extern "system" fn Java_com_gravitaltalk_GravitalTalkJni_nativeAcceptAny(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    if handle == 0 {
        return status_jint(GsStatus::GS_ERR_NULL_POINTER);
    }
    status_jint(unsafe { gs_session_accept_any(handle as *mut GsSessionHandle) })
}
