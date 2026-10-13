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

/// Mientras el cliente espera la respuesta a su saludo le llegan datagramas
/// que no lo son: las sondas con las que el anfitrión busca camino entre
/// redes. No pueden gastar sus reintentos: la respuesta tarda en cruzar una
/// red real más de lo que tardan en llegar nueve sondas.
#[tokio::test]
async fn las_sondas_ajenas_no_agotan_el_handshake_del_cliente() {
    let (ta, tb) = pair();
    let stray = ta.peer_tx.clone();
    let server = session(ta, HandshakeMode::Noise, Some("sala"));
    let client = session(tb, HandshakeMode::Noise, Some("sala"));

    let hs_client =
        tokio::spawn(async move { client.handshake(SessionRole::Client, ADDR_A).await });
    // Treinta sondas en 150 ms, del mismo origen que el anfitrión.
    for _ in 0..30 {
        stray.send((b"GTRV-sonda".to_vec(), ADDR_A)).unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    // Y sólo entonces contesta el anfitrión, como tras un viaje por la red.
    let hs_server =
        tokio::spawn(async move { server.handshake(SessionRole::Server, ADDR_B).await });

    timeout(Duration::from_secs(5), hs_client)
        .await
        .expect("el cliente no terminó")
        .expect("tarea del cliente")
        .expect("el handshake debe sobrevivir a las sondas");
    timeout(Duration::from_secs(5), hs_server)
        .await
        .expect("el servidor no terminó")
        .expect("tarea del servidor")
        .expect("handshake del servidor");
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
