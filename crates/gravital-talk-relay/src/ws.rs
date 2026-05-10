//! WebSocket bridge para peers que no pueden hacer UDP (browser).

use std::sync::Arc;

use bytes::Bytes;
use futures_util::{SinkExt, StreamExt};
use gravital_talk_core::packet::PacketView;
use tokio::net::{TcpListener, UdpSocket};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

use crate::router::{RouteDecision, Router, SessionEndpoint};

pub async fn run(
    listener: TcpListener,
    udp_socket: Arc<UdpSocket>,
    router: Arc<Router>,
) -> anyhow::Result<()> {
    let local = listener.local_addr()?;
    tracing::info!(?local, "WebSocket relay listening");

    loop {
        let (stream, peer_addr) = listener.accept().await?;
        let router = router.clone();
        let udp_socket = udp_socket.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_connection(stream, peer_addr, udp_socket, router).await {
                tracing::warn!(?peer_addr, ?e, "ws connection error");
            }
        });
    }
}

async fn handle_connection(
    stream: tokio::net::TcpStream,
    peer_addr: std::net::SocketAddr,
    udp_socket: Arc<UdpSocket>,
    router: Arc<Router>,
) -> anyhow::Result<()> {
    let ws = tokio_tungstenite::accept_async(stream).await?;
    router.metrics().ws_connections.inc();
    let (mut ws_sink, mut ws_stream) = ws.split();
    let (tx, mut rx) = mpsc::unbounded_channel::<Bytes>();

    let metrics = router.metrics().clone();
    let writer = tokio::spawn(async move {
        while let Some(data) = rx.recv().await {
            if ws_sink.send(Message::Binary(data.to_vec())).await.is_err() {
                break;
            }
        }
        let _ = ws_sink.close().await;
        metrics.ws_connections.dec();
    });

    while let Some(msg) = ws_stream.next().await {
        match msg? {
            Message::Binary(data) => {
                router.metrics().packets_in.inc();
                router.metrics().bytes_in.inc_by(data.len() as u64);
                let bytes = Bytes::from(data.to_vec());

                let session_id = match PacketView::decode(&bytes) {
                    Ok(view) => view.header().session_id,
                    Err(_) => {
                        router.metrics().dropped.with_label_values(&["malformed"]).inc();
                        continue;
                    }
                };

                let from_ep = SessionEndpoint::WebSocket(tx.clone());
                match router.route(session_id, from_ep) {
                    RouteDecision::Broadcast(targets) => {
                        for target in targets {
                            forward(&udp_socket, target, bytes.clone(), &router).await;
                        }
                    }
                    RouteDecision::Registered | RouteDecision::Dropped => {}
                }
            }
            Message::Close(_) => break,
            Message::Ping(_) => {}
            _ => {}
        }
    }

    drop(tx);
    let _ = writer.await;
    let _ = peer_addr;
    Ok(())
}

async fn forward(udp: &Arc<UdpSocket>, target: SessionEndpoint, data: Bytes, router: &Arc<Router>) {
    match target {
        SessionEndpoint::Udp(addr) => match udp.send_to(&data, addr).await {
            Ok(n) => {
                router.metrics().packets_out.inc();
                router.metrics().bytes_out.inc_by(n as u64);
            }
            Err(e) => {
                tracing::warn!(?addr, ?e, "ws→udp forward failed");
            }
        },
        SessionEndpoint::WebSocket(peer_tx) => {
            if peer_tx.send(data.clone()).is_err() {
                router.metrics().dropped.with_label_values(&["ws_disconnected"]).inc();
            } else {
                router.metrics().packets_out.inc();
                router.metrics().bytes_out.inc_by(data.len() as u64);
            }
        }
    }
}
