//! Plano de control gRPC del relay (feature `grpc`).
//!
//! Sirve los contratos de `proto/gravital/v1/server_control.proto` y
//! `pairing.proto`. El audio sigue en UDP/WebSocket; este servicio solo
//! gestiona salas, salud e información (ver `docs/grpc-evaluation.md`).

// El código de tonic/prost es generado: los lints de estilo no aplican.
#![allow(
    clippy::result_large_err,
    clippy::derive_partial_eq_without_eq,
    clippy::missing_const_for_fn
)]

use std::pin::Pin;
use std::sync::Arc;
use std::time::Instant;

use tokio_stream::Stream;
use tonic::{Request, Response, Status};

use crate::rooms::{self, is_valid_code};
use crate::router::{RoomEventMsg, Router};

pub mod proto {
    tonic::include_proto!("gravital.v1");
}

use proto::pairing_service_server::PairingService;
use proto::server_control_server::ServerControl;
use proto::{
    CreateRoomRequest, CreateRoomResponse, DeleteRoomRequest, DeleteRoomResponse, GetRoomRequest,
    GetRoomResponse, ListRoomsRequest, ListRoomsResponse, PairingOffer, Room, RoomEvent,
    ServerHealthRequest, ServerHealthResponse, ServerInfoRequest, ServerInfoResponse,
    WatchRoomRequest,
};

// Códigos del enum `RoomEvent.EventType` (proto).
const ETYPE_PEER_JOINED: i32 = 1;
const ETYPE_PEER_LEFT: i32 = 2;
const ETYPE_FLOOR_GRANTED: i32 = 3;
const ETYPE_FLOOR_RELEASED: i32 = 4;

/// Implementación del servicio de control.
#[derive(Clone)]
pub struct ControlService {
    router: Arc<Router>,
    started: Instant,
}

pub type RoomEventStream = Pin<Box<dyn Stream<Item = Result<RoomEvent, Status>> + Send + 'static>>;

impl ControlService {
    pub fn new(router: Arc<Router>) -> Self {
        Self {
            router,
            started: Instant::now(),
        }
    }

    fn to_proto_room(&self, code: &str, session_id: u32) -> Room {
        let peers = self.router.peer_count(session_id);
        Room {
            code: code.to_string(),
            session_id,
            peer_count: peers as i32,
            created_at_unix_ms: 0,
        }
    }

    fn event_to_proto(msg: RoomEventMsg, code: String) -> RoomEvent {
        let (etype, esid) = match msg {
            RoomEventMsg::PeerJoined { session_id } => (ETYPE_PEER_JOINED, session_id),
            RoomEventMsg::PeerLeft { session_id } => (ETYPE_PEER_LEFT, session_id),
            RoomEventMsg::FloorGranted { session_id } => (ETYPE_FLOOR_GRANTED, session_id),
            RoomEventMsg::FloorReleased { session_id } => (ETYPE_FLOOR_RELEASED, session_id),
        };
        RoomEvent {
            r#type: etype,
            room_code: code,
            session_id: esid,
            at_unix_ms: 0,
            peer_ssrc: 0,
        }
    }
}

#[tonic::async_trait]
impl ServerControl for ControlService {
    type WatchRoomStream = RoomEventStream;

    async fn get_server_info(
        &self,
        _req: Request<ServerInfoRequest>,
    ) -> Result<Response<ServerInfoResponse>, Status> {
        let resp = ServerInfoResponse {
            version: env!("CARGO_PKG_VERSION").to_string(),
            protocol_version: u32::from(gravital_talk_core::PROTOCOL_VERSION),
            max_sessions: self.router.max_sessions() as i32,
            max_peers_per_session: self.router.max_peers_per_session() as i32,
            active_sessions: self.router.active_sessions() as i64,
            active_peers: self.router.total_peers() as i64,
            uptime_secs: self.started.elapsed().as_secs().to_string(),
        };
        Ok(Response::new(resp))
    }

    async fn get_server_health(
        &self,
        _req: Request<ServerHealthRequest>,
    ) -> Result<Response<ServerHealthResponse>, Status> {
        let m = self.router.metrics();
        let dropped = m
            .dropped
            .get_metric_with_label_values(&["malformed"])
            .map(|c| c.get())
            .unwrap_or(0);
        let resp = ServerHealthResponse {
            status: "ok".to_string(),
            packets_in: m.packets_in.get(),
            packets_out: m.packets_out.get(),
            bytes_in: m.bytes_in.get(),
            bytes_out: m.bytes_out.get(),
            dropped_invalid: dropped,
            active_sessions: self.router.active_sessions() as i64,
        };
        Ok(Response::new(resp))
    }

    async fn create_room(
        &self,
        req: Request<CreateRoomRequest>,
    ) -> Result<Response<CreateRoomResponse>, Status> {
        let r = req.into_inner();
        if r.session_id == 0 {
            return Err(Status::invalid_argument("session_id must be > 0"));
        }
        let code = rooms::generate_code();
        if !self.router.register_room(code.clone(), r.session_id) {
            return Err(Status::already_exists("room code collision, retry"));
        }
        Ok(Response::new(CreateRoomResponse {
            room: Some(self.to_proto_room(&code, r.session_id)),
        }))
    }

    async fn get_room(
        &self,
        req: Request<GetRoomRequest>,
    ) -> Result<Response<GetRoomResponse>, Status> {
        let code = req.into_inner().code;
        self.router.resolve_room(&code).map_or_else(
            || Err(Status::not_found("room not found")),
            |sid| {
                Ok(Response::new(GetRoomResponse {
                    room: Some(self.to_proto_room(&code, sid)),
                }))
            },
        )
    }

    async fn list_rooms(
        &self,
        _req: Request<ListRoomsRequest>,
    ) -> Result<Response<ListRoomsResponse>, Status> {
        let rooms = self
            .router
            .list_rooms()
            .into_iter()
            .map(|(code, sid, _peers)| self.to_proto_room(&code, sid))
            .collect();
        Ok(Response::new(ListRoomsResponse { rooms }))
    }

    async fn delete_room(
        &self,
        req: Request<DeleteRoomRequest>,
    ) -> Result<Response<DeleteRoomResponse>, Status> {
        let deleted = self.router.remove_room(&req.into_inner().code);
        Ok(Response::new(DeleteRoomResponse { deleted }))
    }

    async fn watch_room(
        &self,
        req: Request<WatchRoomRequest>,
    ) -> Result<Response<Self::WatchRoomStream>, Status> {
        let code = req.into_inner().room_code;
        let session_id = self
            .router
            .resolve_room(&code)
            .ok_or_else(|| Status::not_found("room not found"))?;

        let mut rx = self.router.subscribe_events();
        let filter = code;
        let sid_filter = session_id;

        let stream = async_stream::stream! {
            while let Ok(msg) = rx.recv().await {
                let ev = Self::event_to_proto(msg, filter.clone());
                if ev.session_id == sid_filter {
                    yield Ok(ev);
                }
            }
        };
        Ok(Response::new(Box::pin(stream)))
    }
}

#[tonic::async_trait]
impl PairingService for ControlService {
    async fn publish_offer(
        &self,
        req: Request<PairingOffer>,
    ) -> Result<Response<PairingOffer>, Status> {
        // Punto de anclaje de señalización P2P: por ahora el relay solo
        // valida el formato de la URI; el almacenamiento de ofertas llega
        // con el emparejamiento persistente (roadmap 0.4).
        let offer = req.into_inner();
        if !offer.uri.starts_with("gravital-talk://") {
            return Err(Status::invalid_argument("invalid pairing URI"));
        }
        Ok(Response::new(offer))
    }

    async fn resolve_offer(
        &self,
        req: Request<PairingOffer>,
    ) -> Result<Response<PairingOffer>, Status> {
        Ok(Response::new(req.into_inner()))
    }
}

#[allow(unused_imports)]
use proto::pairing_service_server::PairingService as _PairingServiceTrait;

/// Levanta el plano de control gRPC (ServerControl + PairingService).
pub async fn serve(addr: std::net::SocketAddr, router: Arc<Router>) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    serve_with_listener(listener, router).await
}

/// Igual que [`serve`] pero reutilizando un listener ya bindeado
/// (permite puertos efímeros sin carrera).
pub async fn serve_with_listener(
    listener: tokio::net::TcpListener,
    router: Arc<Router>,
) -> anyhow::Result<()> {
    use proto::pairing_service_server::PairingServiceServer;
    use proto::server_control_server::ServerControlServer;
    use tokio_stream::wrappers::TcpListenerStream;

    let incoming = TcpListenerStream::new(listener);
    let control = ControlService::new(router);
    tonic::transport::Server::builder()
        .add_service(ServerControlServer::new(control.clone()))
        .add_service(PairingServiceServer::new(control))
        .serve_with_incoming(incoming)
        .await
        .map_err(Into::into)
}

/// Valida un código de sala (para reutilizar la política del REST).
pub fn valid_room_code(code: &str) -> bool {
    is_valid_code(code)
}
