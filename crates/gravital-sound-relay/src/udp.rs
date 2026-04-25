//! Loop UDP del relay: recibe datagramas, los enruta y los reenvía.

use std::sync::Arc;

use bytes::Bytes;
use gravital_sound_core::packet::PacketView;
use tokio::net::UdpSocket;

use crate::router::{RouteDecision, Router, SessionEndpoint};

pub async fn run(socket: Arc<UdpSocket>, router: Arc<Router>) -> anyhow::Result<()> {
    let mut buf = vec![0u8; 1500];
    let local = socket.local_addr()?;
    tracing::info!(?local, "UDP relay listening");

    loop {
        let (n, from) = socket.recv_from(&mut buf).await?;
        let data = &buf[..n];

        router.metrics().packets_in.inc();
        router.metrics().bytes_in.inc_by(n as u64);

        let session_id = match PacketView::decode(data) {
            Ok(view) => view.header().session_id,
            Err(_) => {
                router
                    .metrics()
                    .dropped
                    .with_label_values(&["malformed"])
                    .inc();
                continue;
            }
        };

        match router.route(session_id, SessionEndpoint::Udp(from)) {
            RouteDecision::Forward(target) => {
                forward_to(&socket, target, Bytes::copy_from_slice(data), &router).await;
            }
            RouteDecision::Registered | RouteDecision::Dropped => {}
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
                router
                    .metrics()
                    .dropped
                    .with_label_values(&["send_error"])
                    .inc();
            }
        },
        SessionEndpoint::WebSocket(tx) => {
            if tx.send(data.clone()).is_err() {
                router
                    .metrics()
                    .dropped
                    .with_label_values(&["ws_disconnected"])
                    .inc();
            } else {
                router.metrics().packets_out.inc();
                router.metrics().bytes_out.inc_by(data.len() as u64);
            }
        }
    }
}
