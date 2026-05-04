//! FASE 4 — Simulación de red: handshake, pérdida de paquetes, congestión.
//!
//! Usa `SimTransport` (par de canales mpsc) para verificar que Session se
//! comporta correctamente sin infra UDP real.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use gravital_talk::{Config, Session, SessionRole, SessionState, Transport, TransportError};
use tokio::sync::{mpsc, Mutex};

// ── SimTransport ──────────────────────────────────────────────────────────────

/// Transporte basado en canales mpsc con inyección de pérdida de paquetes.
/// Usa `tokio::sync::Mutex` para que `recv` sea cancelación-segura: si el
/// futuro exterior es cancelado (p. ej. por `tokio::time::timeout`), el guard
/// se suelta sin consumir ningún paquete del canal.
///
/// La aleatoriedad para las pérdidas usa un `AtomicU32` (LCG atómico) en lugar
/// de `thread_local!`, porque las tareas tokio pueden migrar entre hilos del
/// worker pool y el estado thread-local se resetearía en cada nuevo hilo.
struct SimTransport {
    inbox: Arc<Mutex<mpsc::Receiver<(Vec<u8>, SocketAddr)>>>,
    peer_tx: mpsc::Sender<(Vec<u8>, SocketAddr)>,
    local_addr: SocketAddr,
    loss_percent: Arc<AtomicU8>,
    rng_state: Arc<std::sync::atomic::AtomicU32>,
}

impl std::fmt::Debug for SimTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SimTransport")
            .field("local_addr", &self.local_addr)
            .finish()
    }
}

impl SimTransport {
    fn pair(loss_percent: u8) -> (Arc<Self>, Arc<Self>) {
        let (tx_a, rx_a) = mpsc::channel::<(Vec<u8>, SocketAddr)>(4096);
        let (tx_b, rx_b) = mpsc::channel::<(Vec<u8>, SocketAddr)>(4096);
        let addr_a: SocketAddr = "127.0.0.1:20001".parse().unwrap();
        let addr_b: SocketAddr = "127.0.0.1:20002".parse().unwrap();
        let loss = Arc::new(AtomicU8::new(loss_percent.min(100)));
        let a = Arc::new(SimTransport {
            inbox: Arc::new(Mutex::new(rx_a)),
            peer_tx: tx_b,
            local_addr: addr_a,
            loss_percent: loss.clone(),
            rng_state: Arc::new(std::sync::atomic::AtomicU32::new(0xABCD_1234)),
        });
        let b = Arc::new(SimTransport {
            inbox: Arc::new(Mutex::new(rx_b)),
            peer_tx: tx_a,
            local_addr: addr_b,
            loss_percent: loss,
            rng_state: Arc::new(std::sync::atomic::AtomicU32::new(0x1234_ABCD)),
        });
        (a, b)
    }

    fn should_drop(&self) -> bool {
        let loss = self.loss_percent.load(Ordering::Relaxed);
        if loss == 0 {
            return false;
        }
        // Atomic LCG: state is per-transport and not tied to any OS thread.
        loop {
            let v = self.rng_state.load(Ordering::Relaxed);
            let new_v = v.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            if self.rng_state
                .compare_exchange_weak(v, new_v, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return (v % 100) < loss as u32;
            }
        }
    }
}

#[async_trait]
impl Transport for SimTransport {
    async fn send(&self, bytes: &[u8]) -> Result<usize, TransportError> {
        let n = bytes.len();
        if self.should_drop() {
            return Ok(n);
        }
        self.peer_tx
            .send((bytes.to_vec(), self.local_addr))
            .await
            .map_err(|_| {
                TransportError::Io(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "sim channel closed",
                ))
            })?;
        Ok(n)
    }

    async fn send_to(&self, bytes: &[u8], _dest: SocketAddr) -> Result<usize, TransportError> {
        self.send(bytes).await
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), TransportError> {
        let mut rx = self.inbox.lock().await;
        let (packet, from) = rx.recv().await.ok_or_else(|| {
            TransportError::Io(std::io::Error::new(
                std::io::ErrorKind::BrokenPipe,
                "sim channel closed",
            ))
        })?;
        let n = packet.len().min(buf.len());
        buf[..n].copy_from_slice(&packet[..n]);
        Ok((n, from))
    }

    fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        Ok(self.local_addr)
    }

    async fn close(&self) -> Result<(), TransportError> {
        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn build_session_pair(loss_percent: u8) -> (Arc<Session>, Arc<Session>) {
    let (ta, tb) = SimTransport::pair(loss_percent);
    let client = Arc::new(Session::new(ta, Config::default()));
    let server = Arc::new(Session::new(tb, Config::default()));
    (client, server)
}

async fn handshake_pair(client: Arc<Session>, server: Arc<Session>) {
    let client_addr: SocketAddr = "127.0.0.1:20001".parse().unwrap();
    let server_addr: SocketAddr = "127.0.0.1:20002".parse().unwrap();
    let srv = server.clone();
    let sjh = tokio::spawn(async move { srv.handshake(SessionRole::Server, client_addr).await });
    let cli = client.clone();
    let cjh = tokio::spawn(async move { cli.handshake(SessionRole::Client, server_addr).await });
    cjh.await.unwrap().expect("client handshake");
    sjh.await.unwrap().expect("server handshake");
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Canal perfecto: handshake completa y sesión queda Active.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sim_handshake_perfect_channel() {
    let (client, server) = build_session_pair(0).await;
    handshake_pair(client.clone(), server.clone()).await;
    assert_eq!(client.state().await, SessionState::Active);
    assert_eq!(server.state().await, SessionState::Active);
    assert_ne!(client.session_id(), 0);
    assert_eq!(client.session_id(), server.session_id());
}

/// 20% pérdida: el handshake completa (el cliente reintenta con backoff).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sim_handshake_survives_20pct_loss() {
    let (client, server) = build_session_pair(20).await;
    let r = tokio::time::timeout(
        Duration::from_secs(15),
        handshake_pair(client.clone(), server.clone()),
    )
    .await;
    assert!(r.is_ok(), "handshake should complete within 15s under 20% loss");
    assert_eq!(client.state().await, SessionState::Active);
}

/// Sin pérdida: enviar N frames de forma secuencial (send → recv one-by-one).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sim_audio_roundtrip_no_loss() {
    const FRAMES: u32 = 20;
    let (client, server) = build_session_pair(0).await;
    handshake_pair(client.clone(), server.clone()).await;

    for i in 0..FRAMES {
        let payload = vec![i as u8; 160];
        client.send_audio(&payload).await.unwrap();
    }

    for _ in 0..FRAMES {
        let frame = tokio::time::timeout(Duration::from_secs(5), server.recv_audio())
            .await
            .expect("recv_audio timed out")
            .expect("recv_audio error");
        assert_eq!(frame.payload.len(), 160);
    }
}

/// 10% pérdida: la sesión no falla; la mayoría de frames llegan.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sim_audio_survives_10pct_loss() {
    const FRAMES: u32 = 40;
    let (client, server) = build_session_pair(10).await;
    handshake_pair(client.clone(), server.clone()).await;

    for i in 0..FRAMES {
        client.send_audio(&vec![i as u8; 160]).await.unwrap();
        tokio::time::sleep(Duration::from_millis(1)).await;
    }

    let mut received = 0u32;
    for _ in 0..FRAMES {
        let r = tokio::time::timeout(Duration::from_millis(300), server.recv_audio()).await;
        if let Ok(Ok(_)) = r {
            received += 1;
        }
    }
    let min_expected = (FRAMES * 70) / 100;
    assert!(
        received >= min_expected,
        "expected ≥{min_expected} frames under 10% loss, got {received}"
    );
}

/// Bitrate dentro de límites después de handshake y 8 frames enviados.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn congestion_controller_active_after_handshake() {
    let (client, server) = build_session_pair(0).await;
    handshake_pair(client.clone(), server.clone()).await;
    let bitrate = client.current_bitrate();
    assert!(bitrate >= 8_000 && bitrate <= 64_000, "bitrate out of range: {bitrate}");
    for _ in 0..8 {
        client.send_audio(&vec![0x42u8; 160]).await.unwrap();
    }
    let _ = server.current_bitrate();
}
