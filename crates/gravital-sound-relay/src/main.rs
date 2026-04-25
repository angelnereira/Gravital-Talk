//! Binario `gs-relay` — relay productivo de Gravital Sound.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use gravital_sound_relay::{
    config::RelayConfig, metrics::RelayMetrics, observability, router::Router, udp, ws,
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

    tracing::info!(?cfg, "starting gs-relay");

    let metrics = RelayMetrics::new();
    let router = Arc::new(Router::new(cfg.max_sessions, metrics));

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

    let udp_task = tokio::spawn(udp::run(udp_socket.clone(), router.clone()));
    let ws_task = tokio::spawn(ws::run(ws_listener, udp_socket.clone(), router.clone()));
    let obs_task = tokio::spawn(observability::run(obs_listener, router.clone()));

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
