//! Verifica que el handshake sea estrictamente 3-way: INIT → ACCEPT → CONFIRM.

use std::sync::Arc;

use gravital_sound::{Config, Session, SessionRole, Transport, UdpConfig, UdpTransport};

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn three_way_handshake_completes() {
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

    let s = server.clone();
    let sjh = tokio::spawn(async move { s.handshake(SessionRole::Server, client_addr).await });
    let c = client.clone();
    let cjh = tokio::spawn(async move { c.handshake(SessionRole::Client, server_addr).await });

    cjh.await.unwrap().unwrap();
    sjh.await.unwrap().unwrap();

    let sid = client.session_id();
    assert_ne!(sid, 0);
    assert_eq!(sid, server.session_id());
}

#[tokio::test]
async fn client_timeout_without_server() {
    let client_transport = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    let client = Session::new(client_transport, Config::default());
    // Puerto sin escucha.
    let err = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        client.handshake(SessionRole::Client, "127.0.0.1:1".parse().unwrap()),
    )
    .await
    .unwrap()
    .expect_err("should fail");
    let msg = format!("{err}");
    assert!(
        msg.contains("timed out") || msg.contains("handshake") || msg.contains("I/O"),
        "unexpected error: {msg}"
    );
}
