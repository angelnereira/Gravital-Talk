//! Endpoint HTTP de observabilidad y Rooms API.
//!
//! Rutas disponibles:
//! - `GET  /healthz`           → health check
//! - `GET  /metrics`           → métricas Prometheus
//! - `POST /api/rooms`         → registrar sala; body: `{"session_id":N}`
//! - `GET  /api/rooms`         → listar todas las salas
//! - `GET  /api/rooms/{code}`  → resolver código → session_id
//! - `DELETE /api/rooms/{code}` → eliminar sala
//! - `GET  /ws` (upgrade)     → WebSocket de audio, igual que el listener WS
//!
//! `/ws` existe para plataformas que sólo publican **un** puerto HTTP y nada
//! de UDP (Railway): la app abre ahí el WebSocket y el relay lo trata como
//! cualquier otro peer.

use std::convert::Infallible;
use std::sync::Arc;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use prometheus::Encoder;
use tokio::net::{TcpListener, UdpSocket};
use tokio_tungstenite::tungstenite::handshake::derive_accept_key;
use tokio_tungstenite::tungstenite::protocol::Role;

use crate::rate_limit::RateLimiter;
use crate::rooms;
use crate::router::Router;

/// Lo que necesita la ruta `/ws` para encaminar paquetes.
#[derive(Clone)]
pub struct WsRoute {
    /// Socket UDP del relay: destino de los paquetes hacia peers UDP.
    pub udp_socket: Arc<UdpSocket>,
    pub rate_limit: Option<Arc<RateLimiter>>,
    /// Si es `true`, la IP del cliente se toma de `X-Forwarded-For`.
    /// Sólo detrás de un proxy de confianza: el cliente puede falsearla.
    pub trust_proxy: bool,
}

/// HTTP de observabilidad sin la ruta `/ws`.
pub async fn run(listener: TcpListener, router: Arc<Router>) -> anyhow::Result<()> {
    run_with_ws(listener, router, None).await
}

/// HTTP de observabilidad; con `ws` también acepta WebSocket en `/ws`.
pub async fn run_with_ws(
    listener: TcpListener,
    router: Arc<Router>,
    ws: Option<WsRoute>,
) -> anyhow::Result<()> {
    let local = listener.local_addr()?;
    tracing::info!(
        ?local,
        ws = ws.is_some(),
        "Observability HTTP listening on /metrics + /healthz + /api/rooms"
    );

    loop {
        let (tcp, peer_addr) = listener.accept().await?;
        let router = router.clone();
        let ws = ws.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(tcp);
            let svc = service_fn(move |req| handle(req, router.clone(), ws.clone(), peer_addr));
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, svc)
                .with_upgrades()
                .await
            {
                tracing::debug!(?e, "http connection error");
            }
        });
    }
}

/// `true` si la petición pide pasar a WebSocket.
fn is_ws_upgrade<B>(req: &Request<B>) -> bool {
    let has = |name: hyper::header::HeaderName, needle: &str| {
        req.headers()
            .get_all(name)
            .iter()
            .filter_map(|v| v.to_str().ok())
            .any(|v| v.split(',').any(|t| t.trim().eq_ignore_ascii_case(needle)))
    };
    has(hyper::header::CONNECTION, "upgrade") && has(hyper::header::UPGRADE, "websocket")
}

/// IP del cliente: la del socket o, detrás de un proxy, la primera de
/// `X-Forwarded-For`.
fn client_addr<B>(
    req: &Request<B>,
    peer_addr: std::net::SocketAddr,
    trust_proxy: bool,
) -> std::net::SocketAddr {
    if !trust_proxy {
        return peer_addr;
    }
    req.headers()
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .and_then(|ip| ip.trim().parse::<std::net::IpAddr>().ok())
        .map_or(peer_addr, |ip| {
            std::net::SocketAddr::new(ip, peer_addr.port())
        })
}

/// Contesta el upgrade y deja el WebSocket en manos de [`crate::ws::serve`].
fn upgrade_ws(
    mut req: Request<hyper::body::Incoming>,
    router: Arc<Router>,
    route: WsRoute,
    peer_addr: std::net::SocketAddr,
) -> Response<Full<Bytes>> {
    let Some(key) = req.headers().get(hyper::header::SEC_WEBSOCKET_KEY) else {
        return json_response(
            StatusCode::BAD_REQUEST,
            r#"{"error":"missing Sec-WebSocket-Key"}"#,
        );
    };
    let accept = derive_accept_key(key.as_bytes());
    let client = client_addr(&req, peer_addr, route.trust_proxy);
    tokio::spawn(async move {
        match hyper::upgrade::on(&mut req).await {
            Ok(upgraded) => {
                let io = TokioIo::new(upgraded);
                let ws =
                    tokio_tungstenite::WebSocketStream::from_raw_socket(io, Role::Server, None)
                        .await;
                if let Err(e) =
                    crate::ws::serve(ws, client, route.udp_socket, router, route.rate_limit).await
                {
                    tracing::warn!(?client, ?e, "ws (/ws) connection error");
                }
            }
            Err(e) => tracing::debug!(?client, ?e, "ws upgrade failed"),
        }
    });
    Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header(hyper::header::CONNECTION, "Upgrade")
        .header(hyper::header::UPGRADE, "websocket")
        .header(hyper::header::SEC_WEBSOCKET_ACCEPT, accept)
        .body(Full::new(Bytes::new()))
        .unwrap()
}

async fn handle(
    req: Request<hyper::body::Incoming>,
    router: Arc<Router>,
    ws: Option<WsRoute>,
    peer_addr: std::net::SocketAddr,
) -> Result<Response<Full<Bytes>>, Infallible> {
    if req.uri().path() == "/ws" {
        return Ok(match ws {
            Some(route) if is_ws_upgrade(&req) => upgrade_ws(req, router, route, peer_addr),
            Some(_) => json_response(
                StatusCode::UPGRADE_REQUIRED,
                r#"{"error":"websocket upgrade required"}"#,
            ),
            None => json_response(StatusCode::NOT_FOUND, r#"{"error":"not found"}"#),
        });
    }
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let query_token: Option<String> = req.uri().query().and_then(|q| {
        q.split('&')
            .find_map(|kv| kv.strip_prefix("token=").map(str::to_string))
    });

    // Collect body (needed for POST).
    let body_bytes = req
        .into_body()
        .collect()
        .await
        .map_or_else(|_| Bytes::new(), |b| b.to_bytes());

    let resp = if path == "/healthz" {
        json_response(StatusCode::OK, "\"ok\"")
    } else if path == "/metrics" {
        let metric_families = router.metrics().registry.gather();
        let mut buf = Vec::new();
        let encoder = prometheus::TextEncoder::new();
        if let Err(e) = encoder.encode(&metric_families, &mut buf) {
            tracing::warn!(?e, "metrics encode failed");
        }
        Response::builder()
            .status(StatusCode::OK)
            .header("content-type", encoder.format_type())
            .body(Full::new(Bytes::from(buf)))
            .unwrap()
    } else if path == "/api/rooms" {
        match method {
            Method::GET => handle_list_rooms(&router),
            Method::POST => handle_create_room(&router, &body_bytes),
            _ => method_not_allowed(),
        }
    } else if let Some(code) = path.strip_prefix("/api/rooms/") {
        match method {
            Method::GET => handle_get_room(&router, code, query_token.as_deref()),
            Method::DELETE => handle_delete_room(&router, code),
            _ => method_not_allowed(),
        }
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from_static(
                b"{\"error\":\"not found\"}\n",
            )))
            .unwrap()
    };

    Ok(resp)
}

fn handle_create_room(router: &Router, body: &[u8]) -> Response<Full<Bytes>> {
    let session_id = match extract_session_id(body) {
        Some(id) if id != 0 => id,
        _ => {
            return json_response(
                StatusCode::BAD_REQUEST,
                r#"{"error":"body must be {\"session_id\":N} with N > 0"}"#,
            );
        }
    };
    let token = extract_token(body);

    let code = rooms::generate_code();
    if router.register_room(code.clone(), session_id, token.as_deref()) {
        let body = format!(r#"{{"code":"{code}","session_id":{session_id}}}"#);
        json_response(StatusCode::CREATED, &body)
    } else {
        json_response(
            StatusCode::CONFLICT,
            r#"{"error":"room code collision, retry"}"#,
        )
    }
}

fn handle_get_room(router: &Router, code: &str, token: Option<&str>) -> Response<Full<Bytes>> {
    if !rooms::is_valid_code(code) {
        return json_response(
            StatusCode::BAD_REQUEST,
            r#"{"error":"invalid room code format"}"#,
        );
    }
    if router.room_requires_token(code) && token.is_none() {
        return json_response(
            StatusCode::UNAUTHORIZED,
            r#"{"error":"room requires token"}"#,
        );
    }
    router.resolve_room(code, token).map_or_else(
        || json_response(StatusCode::NOT_FOUND, r#"{"error":"room not found"}"#),
        |session_id| {
            let peer_count = router.peer_count(session_id);
            let body = format!(
                r#"{{"code":"{code}","session_id":{session_id},"peer_count":{peer_count}}}"#
            );
            json_response(StatusCode::OK, &body)
        },
    )
}

fn handle_delete_room(router: &Router, code: &str) -> Response<Full<Bytes>> {
    if router.remove_room(code) {
        json_response(StatusCode::OK, r#"{"deleted":true}"#)
    } else {
        json_response(StatusCode::NOT_FOUND, r#"{"error":"room not found"}"#)
    }
}

fn handle_list_rooms(router: &Router) -> Response<Full<Bytes>> {
    let rooms = router.list_rooms();
    let entries: Vec<String> = rooms
        .iter()
        .map(|(code, sid, peers)| {
            format!(r#"{{"code":"{code}","session_id":{sid},"peer_count":{peers}}}"#)
        })
        .collect();
    let body = format!("[{}]\n", entries.join(","));
    json_response(StatusCode::OK, &body)
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn json_response(status: StatusCode, body: &str) -> Response<Full<Bytes>> {
    let mut s = body.to_string();
    if !s.ends_with('\n') {
        s.push('\n');
    }
    Response::builder()
        .status(status)
        .header("content-type", "application/json")
        .body(Full::new(Bytes::from(s)))
        .unwrap()
}

fn method_not_allowed() -> Response<Full<Bytes>> {
    json_response(
        StatusCode::METHOD_NOT_ALLOWED,
        r#"{"error":"method not allowed"}"#,
    )
}

/// Extrae `session_id` de un body JSON mínimo como `{"session_id":12345}`.
fn extract_session_id(body: &[u8]) -> Option<u32> {
    let s = std::str::from_utf8(body).ok()?;
    let pos = s.find("session_id")?;
    let after_key = &s[pos + "session_id".len()..];
    let colon = after_key.find(':')? + 1;
    let num_str = after_key[colon..].trim_start();
    let end = num_str
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(num_str.len());
    if end == 0 {
        return None;
    }
    num_str[..end].parse().ok()
}

/// Extrae `"token":"..."` opcional del body JSON si existe.
fn extract_token(body: &[u8]) -> Option<String> {
    let s = std::str::from_utf8(body).ok()?;
    let pos = s.find("\"token\"")?;
    let after_key = &s[pos + "\"token\"".len()..];
    let colon = after_key.find(':')? + 1;
    let mut rest = after_key[colon..].trim_start();
    rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_session_id_basic() {
        assert_eq!(extract_session_id(b"{\"session_id\":42}"), Some(42));
        assert_eq!(extract_session_id(b"{ \"session_id\" : 999 }"), Some(999));
        assert_eq!(extract_session_id(b"{\"session_id\":0}"), Some(0));
    }

    #[test]
    fn extract_session_id_missing() {
        assert!(extract_session_id(b"{}").is_none());
        assert!(extract_session_id(b"invalid").is_none());
        assert!(extract_session_id(b"{\"session_id\":\"abc\"}").is_none());
    }
}
