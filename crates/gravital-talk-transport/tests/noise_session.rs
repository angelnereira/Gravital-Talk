//! Tests del handshake Noise (feature `noise`).
//!
//! Cubre:
//! - Noise forzado (NN sin token) end-to-end + audio cifrado.
//! - Token de sala (NNpsk0): con token correcto conecta, con token
//!   incorrecto falla y **no** hay downgrade.
//! - Modo `Auto`: cliente Noise contra servidor legacy cae a v1.

#![cfg(feature = "noise")]

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use gravital_talk_transport::session::HandshakeMode;
use gravital_talk_transport::{Config, Session, SessionRole, Transport, TransportError};
use tokio::sync::mpsc;
use tokio::time::timeout;

type Datagram = (Vec<u8>, SocketAddr);

const ADDR_A: SocketAddr = SocketAddr::V4(std::net::SocketAddrV4::new(
    std::net::Ipv4Addr::LOCALHOST,
    31001,
));
const ADDR_B: SocketAddr = SocketAddr::V4(std::net::SocketAddrV4::new(
    std::net::Ipv4Addr::LOCALHOST,
    31002,
));

#[derive(Debug)]
struct SimTransport {
    inbox: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<Datagram>>>,
    peer_tx: mpsc::UnboundedSender<Datagram>,
    local: SocketAddr,
}

fn pair() -> (Arc<SimTransport>, Arc<SimTransport>) {
    let (tx_a, rx_a) = mpsc::unbounded_channel::<Datagram>();
    let (tx_b, rx_b) = mpsc::unbounded_channel::<Datagram>();
    let a = Arc::new(SimTransport {
        inbox: Arc::new(tokio::sync::Mutex::new(rx_a)),
        peer_tx: tx_b,
        local: ADDR_A,
    });
    let b = Arc::new(SimTransport {
        inbox: Arc::new(tokio::sync::Mutex::new(rx_b)),
        peer_tx: tx_a,
        local: ADDR_B,
    });
    (a, b)
}

#[async_trait]
impl Transport for SimTransport {
    async fn send(&self, bytes: &[u8]) -> Result<usize, TransportError> {
        self.peer_tx
            .send((bytes.to_vec(), self.local))
            .map_err(|_| {
                TransportError::Io(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
            })?;
        Ok(bytes.len())
    }

    async fn send_to(&self, bytes: &[u8], _dest: SocketAddr) -> Result<usize, TransportError> {
        self.send(bytes).await
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), TransportError> {
        let mut rx = self.inbox.lock().await;
        let received = rx.recv().await;
        drop(rx);
        let (packet, from) = received.ok_or_else(|| {
            TransportError::Io(std::io::Error::from(std::io::ErrorKind::BrokenPipe))
        })?;
        let n = packet.len().min(buf.len());
        buf[..n].copy_from_slice(&packet[..n]);
        Ok((n, from))
    }

    fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        Ok(self.local)
    }

    async fn close(&self) -> Result<(), TransportError> {
        Ok(())
    }
}

fn session(transport: Arc<SimTransport>, mode: HandshakeMode, token: Option<&str>) -> Arc<Session> {
    let cfg = Config {
        handshake_mode: mode,
        ..Config::default()
    };
    let s = Arc::new(Session::new(transport as Arc<dyn Transport>, cfg));
    s.set_room_token(token.map(str::to_string));
    s
}

async fn handshake_pair(
    server: Arc<Session>,
    client: Arc<Session>,
    server_peer: SocketAddr,
    client_peer: SocketAddr,
) -> (Result<(), TransportError>, Result<(), TransportError>) {
    let hs_server =
        tokio::spawn(async move { server.handshake(SessionRole::Server, server_peer).await });
    let client_res = timeout(
        Duration::from_secs(5),
        client.handshake(SessionRole::Client, client_peer),
    )
    .await
    .expect("client handshake timeout");

    let server_res = timeout(Duration::from_secs(5), hs_server)
        .await
        .expect("server handshake timeout")
        .expect("server task panicked");
    (client_res, server_res)
}

#[tokio::test]
async fn noise_handshake_and_audio() {
    let (ta, tb) = pair();
    let server = session(ta, HandshakeMode::Noise, None);
    let client = session(tb, HandshakeMode::Noise, None);

    let (c, s) = handshake_pair(server.clone(), client.clone(), ADDR_B, ADDR_A).await;
    c.expect("client noise handshake");
    s.expect("server noise handshake");

    let payload: Vec<u8> = (0..160u8).collect();
    client.send_audio(&payload).await.unwrap();
    let frame = timeout(Duration::from_secs(2), server.recv_audio())
        .await
        .expect("audio frame timeout")
        .expect("recv_audio");
    assert_eq!(frame.payload.as_ref(), &payload[..]);
}

#[tokio::test]
async fn room_token_authenticates() {
    let (ta, tb) = pair();
    let server = session(ta, HandshakeMode::Noise, Some("sala-secreta-1"));
    let client = session(tb, HandshakeMode::Noise, Some("sala-secreta-1"));

    let (c, s) = handshake_pair(server, client, ADDR_B, ADDR_A).await;
    c.expect("cliente con token correcto");
    s.expect("servidor con token correcto");
}

#[tokio::test]
async fn wrong_token_fails_without_downgrade() {
    let (ta, tb) = pair();
    let server = session(ta, HandshakeMode::Auto, Some("sala-secreta-1"));
    let client = session(tb, HandshakeMode::Auto, Some("otro-token"));

    let (c, s) = handshake_pair(server, client, ADDR_B, ADDR_A).await;
    assert!(
        c.is_err(),
        "token incorrecto debe fallar el handshake del cliente"
    );
    assert!(s.is_err(), "el servidor debe rechazar el handshake");
    // Sin downgrade: el server Auto con token no completó v1 tampoco.
}

#[tokio::test]
async fn auto_falls_back_to_legacy_server() {
    let (ta, tb) = pair();
    // Servidor solo legacy; cliente Auto debe degradar a v1.
    let server = session(ta, HandshakeMode::Legacy, None);
    let client = session(tb, HandshakeMode::Auto, None);

    let (c, s) = handshake_pair(server.clone(), client.clone(), ADDR_B, ADDR_A).await;
    c.expect("cliente Auto debe caer a legacy");
    s.expect("servidor legacy");

    let payload: Vec<u8> = (0..160u8).collect();
    client.send_audio(&payload).await.unwrap();
    let frame = timeout(Duration::from_secs(2), server.recv_audio())
        .await
        .expect("audio frame timeout")
        .expect("recv_audio");
    assert_eq!(frame.payload.as_ref(), &payload[..]);
}
