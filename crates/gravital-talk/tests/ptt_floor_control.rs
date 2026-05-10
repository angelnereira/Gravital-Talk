//! Test integración: flujo PTT completo sobre loopback UDP.
//!
//! Verifica que:
//!   1. `ptt_press()` actualiza el flag local `is_ptt_active`.
//!   2. El peer procesa `ControlResume` vía `recv_audio()` y pone
//!      `is_peer_ptt_active = true`.
//!   3. Frames de audio enviados con PTT activo llegan al peer.
//!   4. `ptt_release()` limpia el flag local y el peer procesa `ControlPause`.

use std::sync::Arc;
use std::time::Duration;

use gravital_talk::{Config, Session, SessionRole, UdpConfig, UdpTransport};

async fn make_loopback_pair() -> (Arc<Session>, Arc<Session>) {
    let t_srv = Arc::new(
        UdpTransport::bind(UdpConfig { bind_addr: "127.0.0.1:0".parse().unwrap(), ..Default::default() })
            .await.unwrap(),
    );
    let t_cli = Arc::new(
        UdpTransport::bind(UdpConfig { bind_addr: "127.0.0.1:0".parse().unwrap(), ..Default::default() })
            .await.unwrap(),
    );
    let srv_addr = t_srv.local_addr().unwrap();
    let cli_addr = t_cli.local_addr().unwrap();

    let server = Arc::new(Session::new(t_srv, Config::default()));
    let client = Arc::new(Session::new(t_cli, Config::default()));

    let sh = { let s = server.clone(); tokio::spawn(async move { s.handshake(SessionRole::Server, cli_addr).await }) };
    let ch = { let c = client.clone(); tokio::spawn(async move { c.handshake(SessionRole::Client, srv_addr).await }) };
    sh.await.unwrap().unwrap();
    ch.await.unwrap().unwrap();

    (server, client)
}

// ── Test 1: estado local ─────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ptt_local_state_toggles() {
    let (server, client) = make_loopback_pair().await;

    assert!(!client.is_ptt_active(), "should start idle");

    client.ptt_press().await.unwrap();
    assert!(client.is_ptt_active(), "should be transmitting after press");

    client.ptt_release().await.unwrap();
    assert!(!client.is_ptt_active(), "should be idle after release");

    client.close().await.unwrap();
    server.close().await.unwrap();
}

// ── Test 2: audio llega al peer ──────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn ptt_audio_arrives_at_peer() {
    let (server, client) = make_loopback_pair().await;

    client.ptt_press().await.unwrap();

    let payload = vec![0xABu8; 960];
    client.send_audio(&payload).await.unwrap();

    // recv_audio procesa internamente ControlResume y devuelve el frame de audio.
    let frame = tokio::time::timeout(Duration::from_secs(2), server.recv_audio())
        .await
        .expect("timeout: frame should arrive within 2 s")
        .expect("recv_audio error");

    assert_eq!(frame.payload.as_ref(), &payload[..]);

    client.ptt_release().await.unwrap();
    client.close().await.unwrap();
    server.close().await.unwrap();
}

// ── Test 3: is_peer_ptt_active se actualiza vía recv_audio ──────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn peer_ptt_active_flag_updated_by_control_messages() {
    let (server, client) = make_loopback_pair().await;

    // Pump que consume exactamente un frame de audio.
    let server_clone = server.clone();
    let pump = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(3), server_clone.recv_audio())
            .await
            .expect("pump timeout")
            .expect("pump recv_audio error")
    });

    let payload = vec![0x00u8; 960];

    // press → envía FloorRequest + ControlResume, luego un frame de audio para
    // desbloquear el pump.  En loopback, el orden de llegada es el mismo de envío,
    // así que el pump procesa ControlResume antes que el AudioFrame.
    client.ptt_press().await.unwrap();
    client.send_audio(&payload).await.unwrap();

    // Esperar a que el pump reciba el frame (garantiza que ControlResume ya fue procesado).
    let _frame = pump.await.unwrap();

    assert!(
        server.is_peer_ptt_active(),
        "server should see client as transmitting after ControlResume"
    );

    // ── Release ──────────────────────────────────────────────────────────────
    let server_clone2 = server.clone();
    let pump2 = tokio::spawn(async move {
        tokio::time::timeout(Duration::from_secs(3), server_clone2.recv_audio())
            .await
            .expect("pump2 timeout")
            .expect("pump2 recv_audio error")
    });

    // release → envía FloorRelease + ControlPause, luego frame para desbloquear pump2.
    client.ptt_release().await.unwrap();
    client.send_audio(&payload).await.unwrap();

    let _frame2 = pump2.await.unwrap();

    assert!(
        !server.is_peer_ptt_active(),
        "server should see client as idle after ControlPause"
    );

    client.close().await.unwrap();
    server.close().await.unwrap();
}
