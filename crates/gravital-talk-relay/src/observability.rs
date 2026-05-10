//! Endpoint HTTP de observabilidad y Rooms API.
//!
//! Rutas disponibles:
//! - `GET  /healthz`           → health check
//! - `GET  /metrics`           → métricas Prometheus
//! - `POST /api/rooms`         → registrar sala; body: `{"session_id":N}`
//! - `GET  /api/rooms`         → listar todas las salas
//! - `GET  /api/rooms/{code}`  → resolver código → session_id
//! - `DELETE /api/rooms/{code}` → eliminar sala

use std::convert::Infallible;
use std::sync::Arc;

use http_body_util::{BodyExt, Full};
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper::{Method, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use prometheus::Encoder;
use tokio::net::TcpListener;

use crate::rooms;
use crate::router::Router;

pub async fn run(listener: TcpListener, router: Arc<Router>) -> anyhow::Result<()> {
    let local = listener.local_addr()?;
    tracing::info!(
        ?local,
        "Observability HTTP listening on /metrics + /healthz + /api/rooms"
    );

    loop {
        let (tcp, _) = listener.accept().await?;
        let router = router.clone();
        tokio::spawn(async move {
            let io = TokioIo::new(tcp);
            let svc = service_fn(move |req| handle(req, router.clone()));
            if let Err(e) = hyper::server::conn::http1::Builder::new()
                .serve_connection(io, svc)
                .await
            {
                tracing::debug!(?e, "http connection error");
            }
        });
    }
}

async fn handle(
    req: Request<hyper::body::Incoming>,
    router: Arc<Router>,
) -> Result<Response<Full<Bytes>>, Infallible> {
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // Collect body (needed for POST).
    let body_bytes = match req.into_body().collect().await {
        Ok(b) => b.to_bytes(),
        Err(_) => Bytes::new(),
    };

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
            Method::GET => handle_get_room(&router, code),
            Method::DELETE => handle_delete_room(&router, code),
            _ => method_not_allowed(),
        }
    } else {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .header("content-type", "application/json")
            .body(Full::new(Bytes::from_static(b"{\"error\":\"not found\"}\n")))
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

    let code = rooms::generate_code();
    if router.register_room(code.clone(), session_id) {
        let body = format!(r#"{{"code":"{code}","session_id":{session_id}}}"#);
        json_response(StatusCode::CREATED, &body)
    } else {
        json_response(
            StatusCode::CONFLICT,
            r#"{"error":"room code collision, retry"}"#,
        )
    }
}

fn handle_get_room(router: &Router, code: &str) -> Response<Full<Bytes>> {
    if !rooms::is_valid_code(code) {
        return json_response(
            StatusCode::BAD_REQUEST,
            r#"{"error":"invalid room code format"}"#,
        );
    }
    match router.resolve_room(code) {
        Some(session_id) => {
            let peer_count = router.peer_count(session_id);
            let body = format!(
                r#"{{"code":"{code}","session_id":{session_id},"peer_count":{peer_count}}}"#
            );
            json_response(StatusCode::OK, &body)
        }
        None => json_response(StatusCode::NOT_FOUND, r#"{"error":"room not found"}"#),
    }
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
