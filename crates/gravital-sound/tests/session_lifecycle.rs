//! Test integración: handshake completo, envío, close.

use std::sync::Arc;
use std::time::Duration;

use gravital_sound::{
    Config, Session, SessionRole, SessionState, Transport, UdpConfig, UdpTransport,
};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn full_lifecycle_handshake_send_close() {
    let server_transport = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    let server_addr = server_transport.local_addr().unwrap();

    let client_transport = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    let client_addr = client_transport.local_addr().unwrap();

    let server = Arc::new(Session::new(server_transport, Config::default()));
    let client = Arc::new(Session::new(client_transport, Config::default()));

    let server_handle = {
        let s = server.clone();
        tokio::spawn(async move { s.handshake(SessionRole::Server, client_addr).await })
    };
    let client_handle = {
        let c = client.clone();
        tokio::spawn(async move { c.handshake(SessionRole::Client, server_addr).await })
    };

    client_handle.await.unwrap().unwrap();
    server_handle.await.unwrap().unwrap();

    assert_eq!(client.state().await, SessionState::Active);
    assert_eq!(server.state().await, SessionState::Active);
    assert_ne!(client.session_id(), 0);
    assert_eq!(client.session_id(), server.session_id());

    let payload = vec![0x42u8; 960];
    client.send_audio(&payload).await.unwrap();

    let frame = tokio::time::timeout(Duration::from_secs(2), server.recv_audio())
        .await
        .expect("timeout waiting for audio")
        .expect("recv_audio error");
    assert_eq!(frame.payload.as_ref(), &payload[..]);

    client.close().await.unwrap();
}
