//! Regresión: una ruta de relay con tráfico reciente no debe morir por TTL.
//!
//! `Router::evict_idle` borra las rutas cuyo `last_activity` es más viejo que
//! `max_age_secs` (300 s por defecto en producción). `last_activity` sólo se
//! actualiza dentro de `route()`, es decir cuando el relay recibe un datagrama.
//!
//! Eso crea una dependencia: si el único tráfico de una sesión son heartbeats
//! que el cliente deja de enviar, la ruta muere aunque la sesión siga viva.
//! Con PTT la mayo parte del tiempo hay silencio, así que un host esperando
//! peers puede quedarse sin ruta entre ráfagas de audio.
//!
//! Estos tests fijan la semántica de `evict_idle` a escala (TTL de 1 s en vez
//! de 300) para que un cambio accidental en la lógica de `last_activity` se
//! detecte rápido. El keepalive correspondiente lo aporta `Session`: el cliente
//! renueva la ruta periódicamente.

use std::net::SocketAddr;
use std::time::Duration;

use gravital_talk_relay::metrics::RelayMetrics;
use gravital_talk_relay::router::{RouteDecision, Router, SessionEndpoint};

/// TTL de los tests. En producción es 300 s (`session_ttl_secs`).
const TEST_TTL: u64 = 1;
/// Espera que supera el TTL, para simular "la sesión estuvo en silencio".
const PAST_TTL: Duration = Duration::from_millis(1_200);

fn endpoint(port: u16) -> SessionEndpoint {
    SessionEndpoint::Udp(SocketAddr::from(([127, 0, 0, 1], port)))
}

#[test]
fn idle_route_is_evicted() {
    let router = Router::new(10, 10, RelayMetrics::new());
    router.route(1_000, endpoint(34_001));
    assert_eq!(router.active_sessions(), 1);

    std::thread::sleep(PAST_TTL);

    assert_eq!(
        router.evict_idle(TEST_TTL),
        1,
        "una ruta sin tráfico debe eliminarse por TTL"
    );
    assert_eq!(router.active_sessions(), 0);
    assert_eq!(router.peer_count(1_000), 0);
}

#[test]
fn traffic_refreshes_last_activity_so_the_route_survives() {
    let router = Router::new(10, 10, RelayMetrics::new());
    let sid = 2_000u32;
    let peer = endpoint(34_002);
    router.route(sid, peer.clone());

    // Cada datagrama entrante (audio o heartbeat de keepalive) refresca
    // `last_activity`. Se repite con esperas menores que el TTL.
    for _ in 0..3 {
        std::thread::sleep(Duration::from_millis(600));
        let decision = router.route(sid, peer.clone());
        assert!(
            matches!(
                decision,
                RouteDecision::Registered | RouteDecision::Broadcast(_)
            ),
            "un peer conocido debe registrarse o recibir broadcast, no fallar"
        );
    }

    // Inmediatamente después del último tráfico la ruta sigue viva.
    assert_eq!(
        router.evict_idle(TEST_TTL),
        0,
        "una ruta con tráfico reciente no debe eliminarse"
    );
    assert_eq!(router.active_sessions(), 1);
    assert_eq!(router.peer_count(sid), 1);
}

#[test]
fn new_peer_joins_and_existing_one_keeps_its_route() {
    let router = Router::new(10, 10, RelayMetrics::new());
    let sid = 3_000u32;

    router.route(sid, endpoint(34_003));
    let decision = router.route(sid, endpoint(34_004));

    match decision {
        RouteDecision::Broadcast(existing) => {
            assert_eq!(existing.len(), 1, "debe recibir el peer ya registrado");
            assert_eq!(router.peer_count(sid), 2);
        }
        other => panic!("un peer nuevo debe recibir broadcast, obtuve {other:?}"),
    }
}
