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

/// Un tercer host envía ruido al puerto del server mientras el handshake
/// está en curso; la sesión debe ignorar esos datagramas y completar el
/// handshake sólo con el peer esperado.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn rejects_unexpected_peer_during_handshake() {
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

    // Attacker: socket UDP ajeno que bombardea al server con ruido.
    let attacker = tokio::net::UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let attacker_handle = {
        let addr = server_addr;
        tokio::spawn(async move {
            for _ in 0..40u32 {
                let _ = attacker.send_to(b"GARBAGE\0\0\0", addr).await;
                tokio::time::sleep(std::time::Duration::from_millis(25)).await;
            }
        })
    };

    let s = server.clone();
    let sjh = tokio::spawn(async move { s.handshake(SessionRole::Server, client_addr).await });
    let c = client.clone();
    let cjh = tokio::spawn(async move { c.handshake(SessionRole::Client, server_addr).await });

    cjh.await.unwrap().unwrap();
    sjh.await.unwrap().unwrap();
    let _ = attacker_handle.await;

    assert_ne!(client.session_id(), 0);
    assert_eq!(client.session_id(), server.session_id());
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
