//! E2E: handshake 4-way y audio enrutados a través del relay (modo sala).
//!
//! Verifica el camino que habilita el modo servidor/relay:
//!
//! 1. Ambas partes pre-fijan el mismo `session_id` (`set_preset_session_id`).
//! 2. El servidor se registra ante el relay con un `Heartbeat`.
//! 3. El `ClientHello` (con el id de sala) es enrutado al servidor.
//! 4. El handshake completo cruza el relay sin que este descifre nada.
//! 5. Un frame de audio cifrado cliente→servidor llega íntegro.

use std::sync::Arc;
use std::time::Duration;

use gravital_talk_relay::{metrics::RelayMetrics, router::Router, udp};
use gravital_talk_transport::udp::UdpConfig;
use gravital_talk_transport::{Config, Session, SessionRole, Transport, UdpTransport};
use tokio::net::UdpSocket;
use tokio::time::timeout;

/// Id de sala compartido por todas las partes (room code resuelto).
const ROOM_SESSION_ID: u32 = 0x600D_1234;

async fn bind_session() -> Arc<Session> {
    let transport = UdpTransport::bind(UdpConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        ..Default::default()
    })
    .await
    .expect("bind session transport");
    Arc::new(Session::new(
        Arc::new(transport) as Arc<dyn Transport>,
        Config::default(),
    ))
}

#[tokio::test]
async fn routed_handshake_and_audio_through_relay() {
    // 1. Relay UDP en un puerto efímero.
    let relay_socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
    let relay_addr = relay_socket.local_addr().unwrap();
    let router = Arc::new(Router::new(16, 8, RelayMetrics::new()));
    let relay_task = tokio::spawn(udp::run(relay_socket, router.clone()));

    // 2. Dos sesiones con el session_id de la sala pre-fijado.
    let server = bind_session().await;
    let client = bind_session().await;
    server.set_preset_session_id(ROOM_SESSION_ID);
    client.set_preset_session_id(ROOM_SESSION_ID);

    // 3. Handshake cruzado (el servidor registra su endpoint y espera).
    let server_hs = server.clone();
    let hs_server =
        tokio::spawn(async move { server_hs.handshake(SessionRole::Server, relay_addr).await });

    timeout(
        Duration::from_secs(10),
        client.handshake(SessionRole::Client, relay_addr),
    )
    .await
    .expect("client handshake timed out")
    .expect("client handshake failed");

    timeout(Duration::from_secs(10), hs_server)
        .await
        .expect("server handshake timed out")
        .expect("server handshake task panicked")
        .expect("server handshake failed");

    // 4. El relay conoce exactamente una sesión.
    assert_eq!(router.active_sessions(), 1, "relay should track one room");

    // 5. Audio cifrado cliente → servidor a través del relay.
    let payload: Vec<u8> = (0..160u8).collect();
    client.send_audio(&payload).await.unwrap();
    let frame = timeout(Duration::from_secs(5), server.recv_audio())
        .await
        .expect("audio frame timed out")
        .expect("recv_audio failed");
    assert_eq!(frame.payload.as_ref(), &payload[..]);

    relay_task.abort();
}
