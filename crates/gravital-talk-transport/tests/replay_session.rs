//! E2E: la ventana anti-replay descarta un datagrama reinyectado.
//!
//! Tras el handshake, el cliente envía un frame de audio. El mismo datagrama
//! (bytes exactos, misma secuencia) se inyecta de nuevo en un canal real: el
//! servidor debe descartarlo vía `dispatch_packet` (contador
//! `replayed_dropped` == 1) sin producir un segundo frame ni error de
//! integridad.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use gravital_talk_transport::{Config, Session, SessionRole, Transport, TransportError};
use tokio::sync::mpsc;
use tokio::time::timeout;

type Datagram = (Vec<u8>, SocketAddr);

const ADDR_A: SocketAddr = SocketAddr::V4(std::net::SocketAddrV4::new(
    std::net::Ipv4Addr::LOCALHOST,
    30001,
));
const ADDR_B: SocketAddr = SocketAddr::V4(std::net::SocketAddrV4::new(
    std::net::Ipv4Addr::LOCALHOST,
    30002,
));

/// Transporte sobre canales mpsc que además recuerda el último datagrama
/// enviado (para poder reinyectarlo en la prueba de replay).
#[derive(Debug)]
struct SimTransport {
    inbox: Arc<tokio::sync::Mutex<mpsc::UnboundedReceiver<Datagram>>>,
    peer_tx: mpsc::UnboundedSender<Datagram>,
    local: SocketAddr,
    last_sent: Mutex<Option<Vec<u8>>>,
}

fn pair() -> (Arc<SimTransport>, Arc<SimTransport>) {
    let (tx_a, rx_a) = mpsc::unbounded_channel::<Datagram>();
    let (tx_b, rx_b) = mpsc::unbounded_channel::<Datagram>();
    let a = Arc::new(SimTransport {
        inbox: Arc::new(tokio::sync::Mutex::new(rx_a)),
        peer_tx: tx_b,
        local: ADDR_A,
        last_sent: Mutex::new(None),
    });
    let b = Arc::new(SimTransport {
        inbox: Arc::new(tokio::sync::Mutex::new(rx_b)),
        peer_tx: tx_a,
        local: ADDR_B,
        last_sent: Mutex::new(None),
    });
    (a, b)
}

#[async_trait]
impl Transport for SimTransport {
    async fn send(&self, bytes: &[u8]) -> Result<usize, TransportError> {
        *self.last_sent.lock().unwrap() = Some(bytes.to_vec());
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

#[tokio::test]
async fn replayed_datagram_is_dropped() {
    let (sim_server, sim_client) = pair();

    let server = Arc::new(Session::new(
        sim_server.clone() as Arc<dyn Transport>,
        Config::default(),
    ));
    let client = Arc::new(Session::new(
        sim_client.clone() as Arc<dyn Transport>,
        Config::default(),
    ));

    let hs = {
        let s = server.clone();
        tokio::spawn(async move { s.handshake(SessionRole::Server, ADDR_B).await })
    };
    timeout(
        Duration::from_secs(5),
        client.handshake(SessionRole::Client, ADDR_A),
    )
    .await
    .expect("client handshake timed out")
    .expect("client handshake failed");
    timeout(Duration::from_secs(5), hs)
        .await
        .expect("server handshake timed out")
        .expect("server handshake panicked")
        .expect("server handshake failed");

    // 1. Frame legítimo: llega y se entrega una vez.
    let payload: Vec<u8> = (0..160u8).collect();
    client.send_audio(&payload).await.unwrap();
    let frame = timeout(Duration::from_secs(2), server.recv_audio())
        .await
        .expect("audio frame timed out")
        .expect("recv_audio failed");
    assert_eq!(frame.payload.as_ref(), &payload[..]);
    assert_eq!(server.metrics().counters.replayed_dropped(), 0);

    // 2. Reinyectar el mismo datagrama (mismo header seq).
    let dup = sim_client
        .last_sent
        .lock()
        .unwrap()
        .clone()
        .expect("client sent nothing");
    sim_client
        .peer_tx
        .send((dup, ADDR_B))
        .expect("re-inject into server inbox");

    // 3. La reinyección se descarta por replay (el contador sube a 1) y no
    //    genera un frame nuevo ni errores de integridad.
    let _ = timeout(Duration::from_millis(600), server.recv_audio()).await;
    assert_eq!(
        server.metrics().counters.replayed_dropped(),
        1,
        "el datagrama reinyectado debe descartarse por la ventana anti-replay"
    );
    assert_eq!(
        server.metrics().counters.integrity_errors(),
        0,
        "el descarte es por replay, no por fallo AEAD"
    );
}
