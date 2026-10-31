//! E2E: una sesión que habla con el relay por WebSocket (`/ws`, a través de
//! `gs_ws_bridge_start`) y otra que habla por UDP comparten sala: handshake y
//! audio cruzan el relay en los dos sentidos.
//!
//! Es el camino de la app contra un relay en Railway, donde sólo se publica
//! el puerto HTTP.

use std::sync::Arc;
use std::time::Duration;

use gravital_talk_ffi::{gs_ws_bridge_start, gs_ws_bridge_stop, GsStatus};
use gravital_talk_relay::{metrics::RelayMetrics, observability, router::Router, udp};
use gravital_talk_transport::udp::UdpConfig;
use gravital_talk_transport::{Config, Session, SessionRole, Transport, UdpTransport};
use tokio::net::{TcpListener, UdpSocket};
use tokio::time::timeout;

const ROOM_SESSION_ID: u32 = 0x0B5E_7A1C;

async fn bind_session() -> Arc<Session> {
    let transport = UdpTransport::bind(UdpConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        ..Default::default()
    })
    .await
    .expect("bind session transport");
    let session = Arc::new(Session::new(
        Arc::new(transport) as Arc<dyn Transport>,
        Config::default(),
    ));
    session.set_preset_session_id(ROOM_SESSION_ID);
    session
}

#[test]
fn sesion_por_websocket_y_sesion_por_udp_en_la_misma_sala() {
    let rt = tokio::runtime::Runtime::new().unwrap();

    // Relay: UDP + HTTP con la ruta `/ws`.
    let (relay_udp, http_addr) = rt.block_on(async {
        let udp_socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
        let relay_udp = udp_socket.local_addr().unwrap();
        let router = Arc::new(Router::new(16, 8, RelayMetrics::new()));
        tokio::spawn(udp::run(udp_socket.clone(), router.clone(), None));
        let http = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let http_addr = http.local_addr().unwrap();
        tokio::spawn(observability::run_with_ws(
            http,
            router,
            Some(observability::WsRoute {
                udp_socket,
                rate_limit: None,
                trust_proxy: false,
            }),
        ));
        (relay_udp, http_addr)
    });

    // El cliente llega al relay por el puente WebSocket.
    let url = std::ffi::CString::new(format!("ws://{http_addr}/ws")).unwrap();
    let mut bridge_port = 0u16;
    // SAFETY: `url` es una C-string válida y `bridge_port` un `u16` escribible.
    let st = unsafe { gs_ws_bridge_start(url.as_ptr(), &mut bridge_port) };
    assert_eq!(st, GsStatus::GS_OK);
    let bridge: std::net::SocketAddr = ([127, 0, 0, 1], bridge_port).into();

    rt.block_on(async {
        let server = bind_session().await;
        let client = bind_session().await;

        let server_hs = server.clone();
        let hs_server =
            tokio::spawn(async move { server_hs.handshake(SessionRole::Server, relay_udp).await });
        timeout(
            Duration::from_secs(10),
            client.handshake(SessionRole::Client, bridge),
        )
        .await
        .expect("handshake del cliente a tiempo")
        .expect("handshake del cliente");
        timeout(Duration::from_secs(10), hs_server)
            .await
            .expect("handshake del servidor a tiempo")
            .unwrap()
            .expect("handshake del servidor");

        // WebSocket → UDP.
        let a: Vec<u8> = (0..160u8).collect();
        client.send_audio(&a).await.unwrap();
        let got = timeout(Duration::from_secs(5), server.recv_audio())
            .await
            .expect("audio ws→udp a tiempo")
            .unwrap();
        assert_eq!(got.payload.as_ref(), &a[..]);

        // UDP → WebSocket.
        let b: Vec<u8> = (0..160u8).rev().collect();
        server.send_audio(&b).await.unwrap();
        let got = timeout(Duration::from_secs(5), client.recv_audio())
            .await
            .expect("audio udp→ws a tiempo")
            .unwrap();
        assert_eq!(got.payload.as_ref(), &b[..]);
    });

    assert_eq!(gs_ws_bridge_stop(bridge_port), GsStatus::GS_OK);
}
