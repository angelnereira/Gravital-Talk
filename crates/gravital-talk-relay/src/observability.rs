//! Endpoint HTTP de observabilidad: `/metrics` (Prometheus) y `/healthz`.

use std::convert::Infallible;
use std::sync::Arc;

use http_body_util::Full;
use hyper::body::Bytes;
use hyper::service::service_fn;
use hyper::{Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use prometheus::Encoder;
use tokio::net::TcpListener;

use crate::router::Router;

pub async fn run(listener: TcpListener, router: Arc<Router>) -> anyhow::Result<()> {
    let local = listener.local_addr()?;
    tracing::info!(
        ?local,
        "Observability HTTP listening on /metrics + /healthz"
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
    let path = req.uri().path();
    let resp = match path {
        "/healthz" => Response::builder()
            .status(StatusCode::OK)
            .header("content-type", "text/plain")
            .body(Full::new(Bytes::from_static(b"OK\n")))
            .unwrap(),
        "/metrics" => {
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
        }
        _ => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Full::new(Bytes::from_static(b"not found\n")))
            .unwrap(),
    };
    Ok(resp)
}
