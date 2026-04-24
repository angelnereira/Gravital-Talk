//! Gravital Sound — SDK WASM.
//!
//! Dentro del browser no tenemos UDP directo, así que el transport
//! WebSocket lo maneja JavaScript (ver `js/websocket-transport.ts`); el
//! WASM sólo hace encode/decode del protocolo y expone una API de alto
//! nivel.

#![forbid(unsafe_op_in_unsafe_fn)]
#![allow(clippy::new_without_default)]

use gravital_sound_core::{
    header::{Flags, PacketHeader},
    message::{HandshakeConfirm, HandshakeInit, MessageType},
    packet::{PacketBuilder, PacketView},
    PROTOCOL_VERSION,
};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {}

/// Versión del crate.
#[wasm_bindgen(js_name = version)]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").into()
}

/// Versión del protocolo wire.
#[wasm_bindgen(js_name = protocolVersion)]
pub fn protocol_version() -> u32 {
    u32::from(PROTOCOL_VERSION)
}

/// Construye un paquete completo (header + payload + len + checksum) en un
/// `Uint8Array` para enviarlo por WebSocket.
#[wasm_bindgen(js_name = encodePacket)]
pub fn encode_packet(
    msg_type: u8,
    session_id: u32,
    sequence: u32,
    timestamp: f64,
    payload: &[u8],
) -> Result<Vec<u8>, JsValue> {
    let header = PacketHeader {
        version: PROTOCOL_VERSION,
        flags: Flags::empty(),
        msg_type,
        session_id,
        sequence,
        timestamp: timestamp as u64,
    };
    let mut out = vec![0u8; 1200];
    let n = PacketBuilder::new(header, payload)
        .encode(&mut out)
        .map_err(|e| JsValue::from_str(&format!("encode: {e}")))?;
    out.truncate(n);
    Ok(out)
}

/// Resultado del decode: payload y metadatos del header.
#[derive(Serialize)]
pub struct DecodedPacket {
    pub msg_type: u8,
    pub session_id: u32,
    pub sequence: u32,
    pub timestamp: f64,
    pub payload: Vec<u8>,
}

/// Decodifica un paquete entrante. Devuelve un objeto JS con los campos
/// útiles + `payload` como `Uint8Array`.
#[wasm_bindgen(js_name = decodePacket)]
pub fn decode_packet(bytes: &[u8]) -> Result<JsValue, JsValue> {
    let view = PacketView::decode(bytes).map_err(|e| JsValue::from_str(&format!("{e}")))?;
    let header = view.header();
    let decoded = DecodedPacket {
        msg_type: header.msg_type,
        session_id: header.session_id,
        sequence: header.sequence,
        timestamp: header.timestamp as f64,
        payload: view.payload().to_vec(),
    };
    serde_wasm_bindgen::to_value(&decoded).map_err(Into::into)
}

/// Helper para construir un `HANDSHAKE_INIT` listo para enviar.
#[wasm_bindgen(js_name = buildHandshakeInit)]
pub fn build_handshake_init(
    sample_rate: u32,
    channels: u8,
    frame_duration_ms: u8,
    max_bitrate: u32,
    capability_flags: u32,
    nonce: u32,
) -> Result<Vec<u8>, JsValue> {
    let init = HandshakeInit {
        protocol_version: PROTOCOL_VERSION,
        codec_preferred: 0x01,
        sample_rate,
        channels,
        frame_duration_ms,
        max_bitrate,
        capability_flags,
        nonce,
    };
    let mut buf = [0u8; HandshakeInit::SIZE];
    init.encode(&mut buf)
        .map_err(|e| JsValue::from_str(&format!("{e}")))?;
    let packet = encode_packet(
        MessageType::HandshakeInit.code(),
        0,
        0,
        0.0,
        &buf,
    )?;
    Ok(packet)
}

/// Helper para construir un `HANDSHAKE_CONFIRM`.
#[wasm_bindgen(js_name = buildHandshakeConfirm)]
pub fn build_handshake_confirm(session_id: u32, sequence: u32) -> Result<Vec<u8>, JsValue> {
    let confirm = HandshakeConfirm { session_id };
    let mut buf = [0u8; HandshakeConfirm::SIZE];
    confirm
        .encode(&mut buf)
        .map_err(|e| JsValue::from_str(&format!("{e}")))?;
    encode_packet(
        MessageType::HandshakeConfirm.code(),
        session_id,
        sequence,
        0.0,
        &buf,
    )
}

/// Códigos de `msg_type` expuestos como constantes para JavaScript.
#[wasm_bindgen]
pub struct MsgType;

#[wasm_bindgen]
impl MsgType {
    #[wasm_bindgen(getter = HANDSHAKE_INIT)]
    pub fn handshake_init() -> u8 {
        MessageType::HandshakeInit.code()
    }
    #[wasm_bindgen(getter = HANDSHAKE_ACCEPT)]
    pub fn handshake_accept() -> u8 {
        MessageType::HandshakeAccept.code()
    }
    #[wasm_bindgen(getter = HANDSHAKE_CONFIRM)]
    pub fn handshake_confirm() -> u8 {
        MessageType::HandshakeConfirm.code()
    }
    #[wasm_bindgen(getter = AUDIO_FRAME)]
    pub fn audio_frame() -> u8 {
        MessageType::AudioFrame.code()
    }
    #[wasm_bindgen(getter = HEARTBEAT)]
    pub fn heartbeat() -> u8 {
        MessageType::Heartbeat.code()
    }
    #[wasm_bindgen(getter = HEARTBEAT_ACK)]
    pub fn heartbeat_ack() -> u8 {
        MessageType::HeartbeatAck.code()
    }
    #[wasm_bindgen(getter = CLOSE)]
    pub fn close() -> u8 {
        MessageType::Close.code()
    }
}
