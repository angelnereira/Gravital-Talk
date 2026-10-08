//! Smoke test del plano de control gRPC (feature `grpc`).
//!
//! Levanta el servicio real (Router + ControlService) en un puerto efímero
//! y lo ejercita como cliente tonic: crear sala, resolverla por código,
//! listar, health e info.

#![cfg(feature = "grpc")]

use std::sync::Arc;

use gravital_talk_relay::grpc::proto::server_control_client::ServerControlClient;
use gravital_talk_relay::grpc::proto::{
    CreateRoomRequest, GetRoomRequest, ListRoomsRequest, ServerHealthRequest, ServerInfoRequest,
};
use gravital_talk_relay::{grpc, metrics::RelayMetrics, router::Router};
use tokio::net::TcpListener;

const SESSION_ID: u32 = 0x600D_1337;

#[tokio::test]
async fn control_plane_roundtrip() {
    // Listener efímero reutilizado por el servidor (sin carrera de puertos).
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let router = Arc::new(Router::new(16, 8, RelayMetrics::new()));
    let serve_router = router.clone();
    let server_task = tokio::spawn(async move {
        grpc::serve_with_listener(listener, serve_router)
            .await
            .unwrap();
    });

    // Client tonic (con timeout para no colgarse si el server no arranca).
    let mut client = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        ServerControlClient::connect(format!("http://{addr}")),
    )
    .await
    .expect("connect timed out")
    .expect("connect gRPC client");

    // Info y health.
    let info = client
        .get_server_info(ServerInfoRequest {})
        .await
        .unwrap()
        .into_inner();
    assert_eq!(info.protocol_version, 1);
    assert!(!info.version.is_empty());

    let health = client
        .get_server_health(ServerHealthRequest {})
        .await
        .unwrap()
        .into_inner();
    assert_eq!(health.status, "ok");

    // Crear sala → GetRoom → ListRooms.
    let created = client
        .create_room(CreateRoomRequest {
            session_id: SESSION_ID,
            display_name: String::new(),
        })
        .await
        .unwrap()
        .into_inner();
    let room = created.room.expect("room created");
    assert!(!room.code.is_empty());
    assert_eq!(room.session_id, SESSION_ID);

    let resolved = client
        .get_room(GetRoomRequest {
            code: room.code.clone(),
        })
        .await
        .unwrap()
        .into_inner()
        .room
        .expect("room resolved");
    assert_eq!(resolved.session_id, SESSION_ID);

    let list = client
        .list_rooms(ListRoomsRequest {})
        .await
        .unwrap()
        .into_inner();
    assert!(
        list.rooms.iter().any(|r| r.code == room.code),
        "la sala recién creada debe aparecer en el listado"
    );

    // El router real también distingue la sala creada vía gRPC.
    assert_eq!(router.resolve_room(&room.code), Some(SESSION_ID));

    drop(client);
    server_task.abort();
}
