//! Binario `gs-relay` — relay productivo de Gravital Talk.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use gravital_talk_relay::{
    config::RelayConfig, metrics::RelayMetrics, observability, rate_limit::RateLimiter,
    router::Router, udp, ws,
};
use tokio::net::{TcpListener, UdpSocket};
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "gs-relay", version, about)]
struct Args {
    /// Ruta a un TOML de configuración. Si no se pasa, usa defaults.
    #[arg(long)]
    config: Option<PathBuf>,
    /// Override del bind UDP.
    #[arg(long)]
    udp_bind: Option<std::net::SocketAddr>,
    /// Override del bind WebSocket.
    #[arg(long)]
    ws_bind: Option<std::net::SocketAddr>,
    /// Override del bind del HTTP de observabilidad.
    #[arg(long)]
    observability_bind: Option<std::net::SocketAddr>,
    /// Nivel de log.
    #[arg(long, env = "GS_LOG", default_value = "info")]
    log: String,

    /// Paquetes por segundo permitidos por IP (0 = ilimitado).
    #[arg(long, env = "GS_RATE_LIMIT", default_value_t = 0)]
    rate_limit: u64,

    /// Habilitar plano de control gRPC (feature `grpc`). Ej. 0.0.0.0:50051.
    #[arg(long)]
    grpc_bind: Option<std::net::SocketAddr>,

    /// Certificado PEM (feature `tls`) → WSS en el puerto WS y TLS en gRPC.
    #[arg(long)]
    tls_cert: Option<PathBuf>,

    /// Clave privada PEM (feature `tls`).
    #[arg(long)]
    tls_key: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let filter = EnvFilter::try_new(&args.log).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let mut cfg = match args.config {
        Some(p) => RelayConfig::from_file(&p)?,
        None => RelayConfig::default(),
    };
    if let Some(a) = args.udp_bind {
        cfg.udp_bind = a;
    }
    if let Some(a) = args.ws_bind {
        cfg.ws_bind = a;
    }
    if let Some(a) = args.observability_bind {
        cfg.observability_bind = a;
    }
    if args.rate_limit > 0 {
        cfg.rate_limit_per_sec = args.rate_limit;
    }
    if let Some(bind) = args.grpc_bind {
        cfg.grpc_bind = Some(bind);
    }
    if args.tls_cert.is_some() || args.tls_key.is_some() {
        cfg.tls_cert = args.tls_cert;
        cfg.tls_key = args.tls_key;
    }

    tracing::info!(?cfg, "starting gs-relay");

    let metrics = RelayMetrics::new();
    let router = Arc::new(Router::new(
        cfg.max_sessions,
        cfg.max_peers_per_session,
        metrics,
    ));
    let rate_limit =
        (cfg.rate_limit_per_sec > 0).then(|| Arc::new(RateLimiter::new(cfg.rate_limit_per_sec)));

    // TLS (feature `tls`): si se dan cert+key, WS pasa a WSS y gRPC a TLS.
    let tls_acceptor: Option<Arc<gravital_talk_relay::tls::TlsAcceptor>> =
        match (&cfg.tls_cert, &cfg.tls_key) {
            (Some(cert), Some(key)) if !cfg!(feature = "tls") => {
                anyhow::bail!("TLS requiere compilar el relay con --features tls");
            }
            (Some(cert), Some(key)) => {
                let acceptor = gravital_talk_relay::tls::load_acceptor(cert, key)?;
                tracing::info!(?cert, "TLS habilitado: WSS + gRPC over TLS");
                Some(acceptor)
            }
            _ => None,
        };

    let udp_socket = Arc::new(UdpSocket::bind(cfg.udp_bind).await?);
    let ws_listener = TcpListener::bind(cfg.ws_bind).await?;
    let obs_listener = TcpListener::bind(cfg.observability_bind).await?;

    // GC thread: evict sessions idle por más del TTL configurado.
    let gc_router = router.clone();
    let ttl = cfg.session_ttl_secs;
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(30));
        loop {
            tick.tick().await;
            let removed = gc_router.evict_idle(ttl);
            if removed > 0 {
                tracing::info!(removed, "evicted idle sessions");
            }
        }
    });

    let udp_task = tokio::spawn(udp::run(
        udp_socket.clone(),
        router.clone(),
        rate_limit.clone(),
    ));
    let ws_task = tokio::spawn(ws::run(
        ws_listener,
        udp_socket.clone(),
        router.clone(),
        rate_limit,
        tls_acceptor.clone(),
    ));
    let obs_task = tokio::spawn(observability::run(obs_listener, router.clone()));

    // Plano de control gRPC (feature `grpc`).
    #[cfg(feature = "grpc")]
    if let Some(bind) = cfg.grpc_bind {
        let router_grpc = router.clone();
        tokio::spawn(async move {
            #[cfg(feature = "tls")]
            if let (Some(cert), Some(key)) = (&cfg.tls_cert, &cfg.tls_key) {
                if let Err(e) =
                    gravital_talk_relay::grpc::serve_tls(bind, router_grpc.clone(), cert, key).await
                {
                    tracing::error!(?e, "gRPC control plane (TLS) error");
                }
                return;
            }
            if let Err(e) = gravital_talk_relay::grpc::serve(bind, router_grpc).await {
                tracing::error!(?e, "gRPC control plane error");
            }
        });
        tracing::info!(?bind, "gRPC control plane listening");
    }

    // Esperar Ctrl-C o que algún task termine con error.
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("shutdown requested");
        }
        r = udp_task => {
            tracing::error!(?r, "UDP task exited unexpectedly");
        }
        r = ws_task => {
            tracing::error!(?r, "WS task exited unexpectedly");
        }
        r = obs_task => {
            tracing::error!(?r, "observability task exited unexpectedly");
        }
    }

    Ok(())
}
