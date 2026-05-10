//! Loop UDP del relay: recibe datagramas, los enruta y los reenvía.
//!
//! Además de routing básico, este módulo implementa floor arbitration:
//! intercepta FloorRequest/FloorRelease y responde con Grant/Deny/Taken
//! sin reenviar el paquete original a los peers.

use std::net::SocketAddr;
use std::sync::Arc;

use bytes::Bytes;
use gravital_talk_core::header::PacketHeader;
use gravital_talk_core::message::MessageType;
use gravital_talk_core::packet::{PacketBuilder, PacketView};
use tokio::net::UdpSocket;

use crate::router::{FloorDecision, RouteDecision, Router, SessionEndpoint};

pub async fn run(socket: Arc<UdpSocket>, router: Arc<Router>) -> anyhow::Result<()> {
    let mut buf = vec![0u8; 1500];
    let local = socket.local_addr()?;
    tracing::info!(?local, "UDP relay listening");

    loop {
        let (n, from) = socket.recv_from(&mut buf).await?;
        let data = &buf[..n];

        router.metrics().packets_in.inc();
        router.metrics().bytes_in.inc_by(n as u64);

        let view = match PacketView::decode(data) {
            Ok(v) => v,
            Err(_) => {
                router.metrics().dropped.with_label_values(&["malformed"]).inc();
                continue;
            }
        };

        let session_id = view.header().session_id;
        let msg_type = view.header().msg_type;

        // ── Floor Control: el relay actúa como árbitro ────────────────────
        match MessageType::from_code(msg_type) {
            Ok(MessageType::FloorRequest) => {
                // Primero registrar el peer si es nuevo.
                let _ = router.route(session_id, SessionEndpoint::Udp(from));

                match router.floor_request(session_id, from) {
                    FloorDecision::Granted { others } => {
                        tracing::debug!(session_id, ?from, "floor granted");
                        // Enviar FLOOR_GRANT al solicitante.
                        send_floor_msg(
                            &socket,
                            MessageType::FloorGrant,
                            session_id,
                            from,
                            &router,
                        ).await;
                        // Notificar a los demás con FLOOR_TAKEN.
                        for other in others {
                            send_floor_msg_to(
                                &socket,
                                MessageType::FloorTaken,
                                session_id,
                                other,
                                &router,
                            ).await;
                        }
                    }
                    FloorDecision::Denied => {
                        tracing::debug!(session_id, ?from, "floor denied");
                        send_floor_msg(&socket, MessageType::FloorDeny, session_id, from, &router)
                            .await;
                    }
                    FloorDecision::Unknown => {
                        // Peer no registrado aún; ignorar.
                    }
                    _ => {}
                }
                continue; // No reenviar FloorRequest a otros peers.
            }
            Ok(MessageType::FloorRelease) => {
                match router.floor_release(session_id, from) {
                    FloorDecision::Released { others } => {
                        tracing::debug!(session_id, ?from, "floor released");
                        // Notificar a los demás que el floor quedó libre.
                        for other in others {
                            send_floor_msg_to(
                                &socket,
                                MessageType::FloorRelease,
                                session_id,
                                other,
                                &router,
                            ).await;
                        }
                    }
                    _ => {}
                }
                continue; // No reenviar FloorRelease; el relay ya lo procesó.
            }
            _ => {}
        }

        // ── Routing normal para todos los demás mensajes ──────────────────
        match router.route(session_id, SessionEndpoint::Udp(from)) {
            RouteDecision::Broadcast(targets) => {
                let payload = Bytes::copy_from_slice(data);
                for target in targets {
                    forward_to(&socket, target, payload.clone(), &router).await;
                }
            }
            RouteDecision::Registered | RouteDecision::Dropped => {}
        }
    }
}

/// Construye y envía un mensaje de floor control de 4 bytes de payload (SSRC=from).
async fn send_floor_msg(
    socket: &Arc<UdpSocket>,
    msg_type: MessageType,
    session_id: u32,
    to: SocketAddr,
    router: &Arc<Router>,
) {
    let mut ssrc_buf = [0u8; 4];
    ssrc_buf.copy_from_slice(&session_id.to_be_bytes());
    send_floor_packet(socket, msg_type, session_id, &ssrc_buf, to, router).await;
}

async fn send_floor_msg_to(
    socket: &Arc<UdpSocket>,
    msg_type: MessageType,
    session_id: u32,
    target: SessionEndpoint,
    router: &Arc<Router>,
) {
    let mut ssrc_buf = [0u8; 4];
    ssrc_buf.copy_from_slice(&session_id.to_be_bytes());
    let header = PacketHeader::new(msg_type.code(), session_id, 0, 0);
    let mut out = vec![0u8; 64];
    if let Ok(n) = PacketBuilder::new(header, &ssrc_buf).encode(&mut out) {
        let payload = Bytes::copy_from_slice(&out[..n]);
        forward_to(socket, target, payload, router).await;
    }
}

async fn send_floor_packet(
    socket: &Arc<UdpSocket>,
    msg_type: MessageType,
    session_id: u32,
    ssrc_payload: &[u8],
    to: SocketAddr,
    router: &Arc<Router>,
) {
    let header = PacketHeader::new(msg_type.code(), session_id, 0, 0);
    let mut out = vec![0u8; 64];
    if let Ok(n) = PacketBuilder::new(header, ssrc_payload).encode(&mut out) {
        match socket.send_to(&out[..n], to).await {
            Ok(sent) => {
                router.metrics().packets_out.inc();
                router.metrics().bytes_out.inc_by(sent as u64);
            }
            Err(e) => {
                tracing::warn!(?to, ?e, "failed to send floor control msg");
            }
        }
    }
}

async fn forward_to(
    socket: &Arc<UdpSocket>,
    target: SessionEndpoint,
    data: Bytes,
    router: &Arc<Router>,
) {
    match target {
        SessionEndpoint::Udp(addr) => match socket.send_to(&data, addr).await {
            Ok(n) => {
                router.metrics().packets_out.inc();
                router.metrics().bytes_out.inc_by(n as u64);
            }
            Err(e) => {
                tracing::warn!(?addr, ?e, "failed to forward to UDP peer");
                router.metrics().dropped.with_label_values(&["send_error"]).inc();
            }
        },
        SessionEndpoint::WebSocket(tx) => {
            if tx.send(data.clone()).is_err() {
                router.metrics().dropped.with_label_values(&["ws_disconnected"]).inc();
            } else {
                router.metrics().packets_out.inc();
                router.metrics().bytes_out.inc_by(data.len() as u64);
            }
        }
    }
}
