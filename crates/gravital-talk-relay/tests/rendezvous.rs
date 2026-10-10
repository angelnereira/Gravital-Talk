//! E2E del servidor de encuentro sobre UDP real.
//!
//! Lo que se verifica es el papel del relay: presentar a dos extremos sin
//! reenviar nada. El invitado arranca con una dirección que no lleva a ningún
//! sitio y acaba hablando **directo** con el anfitrión; el relay no llega a
//! registrar ninguna sesión enrutada.
//!
//! La perforación en sí (que un NAT deje pasar lo que antes tiraba) no se
//! puede ver en loopback, donde no hay NAT: eso lo cubren
//! `gravital-talk-transport/tests/nat_punch.rs` con una red simulada y
//! `scripts/nat-sim.sh` con el NAT del kernel.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use gravital_talk_relay::{metrics::RelayMetrics, router::Router, udp};
use gravital_talk_transport::rendezvous::{Message, RoomKey, REGISTER_SIZE};
use gravital_talk_transport::udp::UdpConfig;
use gravital_talk_transport::{Config, Session, Transport, UdpTransport};
use tokio::net::UdpSocket;
use tokio::time::timeout;

async fn bind_session(relay: SocketAddr) -> Arc<Session> {
    let transport = UdpTransport::bind(UdpConfig {
        bind_addr: "127.0.0.1:0".parse().unwrap(),
        ..Default::default()
    })
    .await
    .expect("bind");
    let session = Session::new(
        Arc::new(transport) as Arc<dyn Transport>,
        Config {
            frame_duration_ms: 10,
            ..Config::default()
        },
    );
    session.set_room_token(Some("token-de-prueba".into()));
    session.set_rendezvous(Some(relay));
    Arc::new(session)
}

async fn spawn_relay() -> (SocketAddr, Arc<Router>) {
    let socket = Arc::new(UdpSocket::bind("127.0.0.1:0").await.unwrap());
    let addr = socket.local_addr().unwrap();
    let router = Arc::new(Router::new(16, 8, RelayMetrics::new()));
    tokio::spawn(udp::run(socket, router.clone(), None));
    (addr, router)
}

#[tokio::test]
async fn el_relay_presenta_a_los_extremos_y_no_enruta_el_audio() {
    let (relay, router) = spawn_relay().await;
    let host = bind_session(relay).await;
    let guest = bind_session(relay).await;

    let accepting = {
        let host = host.clone();
        tokio::spawn(async move { host.handshake_open().await })
    };

    // La única candidata es un puerto donde no escucha nadie: la dirección
    // buena tiene que llegar por el relay.
    let dead: SocketAddr = "127.0.0.1:9".parse().unwrap();
    timeout(Duration::from_secs(15), guest.handshake_any(&[dead]))
        .await
        .expect("el invitado no debe colgarse")
        .expect("debe conectar por la dirección aprendida");
    timeout(Duration::from_secs(5), accepting)
        .await
        .expect("el anfitrión debe terminar")
        .unwrap()
        .expect("handshake del anfitrión");

    // El invitado habla con el anfitrión, no con el relay.
    assert_eq!(guest.peers().await, vec![host.local_addr().unwrap()]);

    let frame = vec![0x42u8; 960];
    guest.send_audio(&frame).await.unwrap();
    let got = timeout(Duration::from_secs(3), host.recv_audio())
        .await
        .expect("el anfitrión debe recibir")
        .unwrap();
    assert_eq!(&got.payload[..], &frame[..]);

    // Y el relay no ha enrutado nada: sólo presentó.
    assert_eq!(router.active_sessions(), 0);
    assert_eq!(router.rendezvous().rooms(), 1);
}

#[tokio::test]
async fn el_relay_no_contesta_mas_de_lo_que_recibe() {
    let (relay, _router) = spawn_relay().await;
    let key = RoomKey::from_token(b"sala");
    let a = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let b = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let mut buf = [0u8; 1500];

    a.send_to(&Message::Register.encode(key), relay)
        .await
        .unwrap();
    let (n, _) = timeout(Duration::from_secs(2), a.recv_from(&mut buf))
        .await
        .expect("respuesta al primer registro")
        .unwrap();
    assert!(n <= REGISTER_SIZE);
    assert_eq!(
        Message::decode(&buf[..n]),
        Some((
            key,
            Message::Peers {
                you: a.local_addr().unwrap(),
                others: vec![]
            }
        ))
    );

    b.send_to(&Message::Register.encode(key), relay)
        .await
        .unwrap();
    let (n, _) = timeout(Duration::from_secs(2), b.recv_from(&mut buf))
        .await
        .expect("respuesta al segundo registro")
        .unwrap();
    assert!(n <= REGISTER_SIZE);
    assert_eq!(
        Message::decode(&buf[..n]),
        Some((
            key,
            Message::Peers {
                you: b.local_addr().unwrap(),
                others: vec![a.local_addr().unwrap()]
            }
        ))
    );

    // Un REGISTER recortado (lo que mandaría quien busca amplificar) no
    // recibe respuesta.
    let short = &Message::Register.encode(key)[..40];
    a.send_to(short, relay).await.unwrap();
    assert!(timeout(Duration::from_millis(300), a.recv_from(&mut buf))
        .await
        .is_err());
}
