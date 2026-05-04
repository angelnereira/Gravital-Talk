//! FASE 4 — Stress tests: volumen alto de frames, envío concurrente, métricas.
//!
//! Estos tests usan sockets UDP reales sobre loopback para garantizar que la
//! stack completa (UDP → Session → JitterBuffer) aguanta carga sostenida.

use std::sync::Arc;
use std::time::Duration;

use gravital_talk::{Config, Session, SessionRole, Transport, UdpConfig, UdpTransport};

// ── Helpers ───────────────────────────────────────────────────────────────────

async fn make_session_pair() -> (Arc<Session>, Arc<Session>, std::net::SocketAddr, std::net::SocketAddr) {
    let srv_transport = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    let cli_transport = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    let server_addr = srv_transport.local_addr().unwrap();
    let client_addr = cli_transport.local_addr().unwrap();
    let server = Arc::new(Session::new(srv_transport, Config::default()));
    let client = Arc::new(Session::new(cli_transport, Config::default()));
    (client, server, client_addr, server_addr)
}

async fn do_handshake(
    client: Arc<Session>,
    server: Arc<Session>,
    client_addr: std::net::SocketAddr,
    server_addr: std::net::SocketAddr,
) {
    let srv = server.clone();
    let sjh = tokio::spawn(async move { srv.handshake(SessionRole::Server, client_addr).await });
    let cli = client.clone();
    let cjh = tokio::spawn(async move { cli.handshake(SessionRole::Client, server_addr).await });
    cjh.await.unwrap().expect("client handshake failed");
    sjh.await.unwrap().expect("server handshake failed");
}

// ── Tests ─────────────────────────────────────────────────────────────────────

/// Envía 500 frames de audio de 160 bytes y verifica que todos llegan.
/// Ejercita el path completo: cifrado AEAD + FEC encoder + JitterBuffer.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stress_500_frames_roundtrip() {
    const FRAMES: u32 = 500;
    let (client, server, client_addr, server_addr) = make_session_pair().await;
    do_handshake(client.clone(), server.clone(), client_addr, server_addr).await;

    // Receiver en background.
    let srv = server.clone();
    let recv_handle = tokio::spawn(async move {
        let mut count = 0u32;
        while count < FRAMES {
            let result =
                tokio::time::timeout(Duration::from_secs(10), srv.recv_audio()).await;
            match result {
                Ok(Ok(_frame)) => count += 1,
                Ok(Err(e)) => panic!("recv_audio error at frame {count}: {e}"),
                Err(_) => panic!("recv_audio timed out at frame {count}/{FRAMES}"),
            }
        }
        count
    });

    // Sender: 500 frames con payload identificable.
    for i in 0..FRAMES {
        let payload = vec![(i & 0xFF) as u8; 160];
        client
            .send_audio(&payload)
            .await
            .unwrap_or_else(|e| panic!("send_audio failed at frame {i}: {e}"));
    }

    let received = recv_handle.await.unwrap();
    assert_eq!(received, FRAMES, "all {FRAMES} frames must arrive over loopback");
}

/// Envía 200 frames de tamaño máximo (MTU - overhead) sin pausa.
/// Verifica que no hay panics ni errores bajo alta carga.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stress_burst_large_frames() {
    const FRAMES: u32 = 200;
    const PAYLOAD_SIZE: usize = 900; // cerca del límite práctico de payload
    let (client, server, client_addr, server_addr) = make_session_pair().await;
    do_handshake(client.clone(), server.clone(), client_addr, server_addr).await;

    let srv = server.clone();
    let recv_handle = tokio::spawn(async move {
        let mut count = 0u32;
        while count < FRAMES {
            match tokio::time::timeout(Duration::from_secs(10), srv.recv_audio()).await {
                Ok(Ok(frame)) => {
                    assert_eq!(
                        frame.payload.len(),
                        PAYLOAD_SIZE,
                        "frame {count} has wrong size"
                    );
                    count += 1;
                }
                Ok(Err(e)) => panic!("recv error: {e}"),
                Err(_) => panic!("timeout at frame {count}/{FRAMES}"),
            }
        }
        count
    });

    let payload = vec![0xABu8; PAYLOAD_SIZE];
    for _ in 0..FRAMES {
        client.send_audio(&payload).await.unwrap();
    }

    let received = recv_handle.await.unwrap();
    assert_eq!(received, FRAMES);
}

/// Dos senders concurrentes enviando al mismo server.
/// Verifica que la serialización interna del Session (Mutex en send_audio) no
/// produce deadlock ni corrupción.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stress_concurrent_senders() {
    const FRAMES_PER_SENDER: u32 = 100;
    const SENDERS: u32 = 4;
    const TOTAL: u32 = FRAMES_PER_SENDER * SENDERS;

    let (client, server, client_addr, server_addr) = make_session_pair().await;
    do_handshake(client.clone(), server.clone(), client_addr, server_addr).await;

    let srv = server.clone();
    let recv_handle = tokio::spawn(async move {
        let mut count = 0u32;
        while count < TOTAL {
            match tokio::time::timeout(Duration::from_secs(10), srv.recv_audio()).await {
                Ok(Ok(_)) => count += 1,
                Ok(Err(e)) => panic!("recv error: {e}"),
                Err(_) => panic!("timeout at frame {count}/{TOTAL}"),
            }
        }
        count
    });

    // Lanzar SENDERS tareas que envían concurrentemente.
    let mut send_handles = Vec::new();
    for sender_id in 0..SENDERS {
        let cli = client.clone();
        send_handles.push(tokio::spawn(async move {
            for i in 0..FRAMES_PER_SENDER {
                let payload = vec![(sender_id * 100 + i) as u8; 160];
                cli.send_audio(&payload)
                    .await
                    .unwrap_or_else(|e| panic!("sender {sender_id} failed at frame {i}: {e}"));
            }
        }));
    }

    for h in send_handles {
        h.await.unwrap();
    }

    let received = recv_handle.await.unwrap();
    assert_eq!(received, TOTAL, "all {TOTAL} frames from {SENDERS} senders must arrive");
}

/// Verifica que las métricas de la sesión reflejan el tráfico enviado y que
/// el bitrate estimado está dentro de los límites configurados.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stress_metrics_consistent_after_traffic() {
    const FRAMES: u32 = 100;
    let (client, server, client_addr, server_addr) = make_session_pair().await;
    do_handshake(client.clone(), server.clone(), client_addr, server_addr).await;

    let srv = server.clone();
    let recv_handle = tokio::spawn(async move {
        let mut count = 0u32;
        while count < FRAMES {
            match tokio::time::timeout(Duration::from_secs(5), srv.recv_audio()).await {
                Ok(Ok(_)) => count += 1,
                _ => break,
            }
        }
        count
    });

    for _ in 0..FRAMES {
        client.send_audio(&vec![0u8; 160]).await.unwrap();
    }
    recv_handle.await.unwrap();

    // Después del tráfico, el bitrate del controlador de congestión debe estar
    // dentro del rango permitido por la Config por defecto.
    let cli_bitrate = client.current_bitrate();
    let srv_bitrate = server.current_bitrate();

    assert!(
        cli_bitrate >= 8_000 && cli_bitrate <= 64_000,
        "client bitrate {cli_bitrate} out of [8k, 64k] range"
    );
    assert!(
        srv_bitrate >= 8_000 && srv_bitrate <= 64_000,
        "server bitrate {srv_bitrate} out of [8k, 64k] range"
    );

    // Las métricas del cliente deben reflejar los paquetes enviados.
    let snap = client.metrics().snapshot(0.0);
    assert!(
        snap.packets_sent >= FRAMES as u64,
        "expected at least {FRAMES} packets sent, got {}",
        snap.packets_sent
    );
}

/// Bidireccional: cliente envía audio al servidor Y el servidor envía audio al
/// cliente simultáneamente. Verifica que ambas direcciones funcionan bajo carga.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stress_bidirectional_audio() {
    const FRAMES: u32 = 100;
    let (client, server, client_addr, server_addr) = make_session_pair().await;
    do_handshake(client.clone(), server.clone(), client_addr, server_addr).await;

    // Server recibe del cliente.
    let srv_recv = server.clone();
    let h_srv_recv = tokio::spawn(async move {
        let mut n = 0u32;
        while n < FRAMES {
            match tokio::time::timeout(Duration::from_secs(5), srv_recv.recv_audio()).await {
                Ok(Ok(_)) => n += 1,
                _ => break,
            }
        }
        n
    });

    // Cliente recibe del servidor.
    let cli_recv = client.clone();
    let h_cli_recv = tokio::spawn(async move {
        let mut n = 0u32;
        while n < FRAMES {
            match tokio::time::timeout(Duration::from_secs(5), cli_recv.recv_audio()).await {
                Ok(Ok(_)) => n += 1,
                _ => break,
            }
        }
        n
    });

    // Cliente envía al servidor.
    let cli = client.clone();
    let h_cli_send = tokio::spawn(async move {
        for _ in 0..FRAMES {
            cli.send_audio(&vec![0x11u8; 160]).await.unwrap();
        }
    });

    // Servidor envía al cliente.
    let srv = server.clone();
    let h_srv_send = tokio::spawn(async move {
        for _ in 0..FRAMES {
            srv.send_audio(&vec![0x22u8; 160]).await.unwrap();
        }
    });

    h_cli_send.await.unwrap();
    h_srv_send.await.unwrap();

    let received_by_server = h_srv_recv.await.unwrap();
    let received_by_client = h_cli_recv.await.unwrap();

    assert_eq!(received_by_server, FRAMES, "server should receive all frames from client");
    assert_eq!(received_by_client, FRAMES, "client should receive all frames from server");
}
