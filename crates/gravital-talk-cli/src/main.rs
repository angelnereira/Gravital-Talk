//! `gs` — CLI de Gravital Talk.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::io::{AsyncReadExt, AsyncWriteExt};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use gravital_talk::{
    CodecId, CodecSession, Config, Session, SessionRole, Transport, UdpConfig, UdpTransport,
};
use gravital_talk_io::{AudioCapture, AudioPlayback, StreamConfig};
use hound::{SampleFormat, WavSpec, WavWriter};
use tracing_subscriber::EnvFilter;

/// Gravital Talk — protocolo moderno de audio en tiempo real.
#[derive(Debug, Parser)]
#[command(name = "gs", version, about, long_about = None)]
struct Cli {
    /// Nivel de log (`error`, `warn`, `info`, `debug`, `trace`).
    #[arg(long, env = "GS_LOG", default_value = "info")]
    log: String,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum CodecArg {
    Pcm,
    #[cfg(feature = "opus")]
    Opus,
}

impl CodecArg {
    fn to_codec_id(self) -> CodecId {
        match self {
            CodecArg::Pcm => CodecId::Pcm,
            #[cfg(feature = "opus")]
            CodecArg::Opus => CodecId::Opus,
        }
    }
}

impl FromStr for CodecArg {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "pcm" => Ok(CodecArg::Pcm),
            #[cfg(feature = "opus")]
            "opus" => Ok(CodecArg::Opus),
            other => Err(format!("unknown codec '{other}'")),
        }
    }
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Envía audio a un peer (`--input` puede ser `sine`, un WAV, o `--device` para micrófono).
    Send {
        /// Host destino.
        #[arg(long)]
        host: String,
        /// Puerto destino.
        #[arg(long, default_value_t = 9000)]
        port: u16,
        /// `sine` (sintetiza tono) o ruta a WAV PCM 16 bits. Ignorado si `--device` está activo.
        #[arg(long, default_value = "sine")]
        input: String,
        /// Nombre del input device (p. ej. `default`). Activa captura desde micrófono.
        #[arg(long)]
        device: Option<String>,
        /// Codec de audio a usar.
        #[arg(long, default_value = "pcm")]
        codec: CodecArg,
        /// Duración en segundos (ignorado con `--device`).
        #[arg(long, default_value_t = 10)]
        duration: u64,
        /// Sample rate.
        #[arg(long, default_value_t = 48_000)]
        sample_rate: u32,
        /// Canales (1 o 2).
        #[arg(long, default_value_t = 1)]
        channels: u8,
    },
    /// Recibe audio y lo escribe a un WAV (+ playback si `--device` activo).
    Receive {
        /// Dirección de bind.
        #[arg(long, default_value = "0.0.0.0")]
        bind: String,
        /// Puerto.
        #[arg(long, default_value_t = 9000)]
        port: u16,
        /// Peer esperado.
        #[arg(long)]
        peer: String,
        /// Puerto del peer.
        #[arg(long)]
        peer_port: u16,
        /// Ruta de salida WAV.
        #[arg(long)]
        output: PathBuf,
        /// Nombre del output device (p. ej. `default`). Activa playback de altavoz en paralelo.
        #[arg(long)]
        device: Option<String>,
        /// Codec de audio a usar (debe coincidir con el sender).
        #[arg(long, default_value = "pcm")]
        codec: CodecArg,
        /// Duración máxima en segundos.
        #[arg(long, default_value_t = 30)]
        duration: u64,
        /// Sample rate para el WAV.
        #[arg(long, default_value_t = 48_000)]
        sample_rate: u32,
        /// Canales.
        #[arg(long, default_value_t = 1)]
        channels: u8,
    },
    /// Lista los audio devices de input y output disponibles.
    Devices,
    /// Benchmark loopback: mide latencia encode→socket→decode en localhost.
    Bench {
        /// `loopback` es el único modo soportado actualmente.
        #[arg(long, default_value = "loopback")]
        mode: String,
        /// Duración en segundos.
        #[arg(long, default_value_t = 5)]
        duration: u64,
    },
    /// Ejecuta un handshake contra un peer e imprime las métricas.
    Info {
        #[arg(long)]
        host: String,
        #[arg(long, default_value_t = 9000)]
        port: u16,
    },
    /// Verifica el entorno: versión, red, permisos.
    Doctor,
    /// Relay productivo: UDP + WebSocket + floor control + rooms + métricas Prometheus.
    ///
    /// Ejemplo:
    ///   gs relay --udp-port 9000 --ws-port 9090 --obs-port 9100
    ///   gs relay --config /etc/gs-relay.toml
    Relay {
        /// Ruta a un TOML de configuración (parámetros CLI tienen precedencia).
        #[arg(long)]
        config: Option<PathBuf>,
        /// Bind address para tráfico UDP.
        #[arg(long, default_value = "0.0.0.0")]
        bind: String,
        /// Puerto UDP de escucha (default: 9000).
        #[arg(long, default_value_t = 9000)]
        udp_port: u16,
        /// Puerto WebSocket de escucha (default: 9090).
        #[arg(long, default_value_t = 9090)]
        ws_port: u16,
        /// Puerto HTTP de observabilidad / rooms API (default: 9100).
        #[arg(long, default_value_t = 9100)]
        obs_port: u16,
        /// TTL de sesiones inactivas en segundos (default: 300).
        #[arg(long, default_value_t = 300)]
        session_ttl: u64,
        /// Máximo número de sesiones simultáneas (default: 10000).
        #[arg(long, default_value_t = 10_000)]
        max_sessions: usize,
        /// Máximo de peers por sesión (default: 50).
        #[arg(long, default_value_t = 50)]
        max_peers: usize,
    },
    /// Operaciones de sala (room codes para descubrimiento sin intercambiar IPs).
    Room {
        #[command(subcommand)]
        action: RoomAction,
    },
    /// Descubre peers Gravital Talk en la red local via UDP broadcast.
    Discover {
        /// Segundos a escuchar (default: 3).
        #[arg(long, default_value_t = 3)]
        timeout: u64,
    },
    /// Push-to-Talk interactivo en tiempo real.
    ///
    /// Conecta con un peer directo o a través de un relay usando un room code.
    /// Usa SPACE para hablar, Q para salir.
    ///
    /// Ejemplos:
    ///   gs ptt --relay 1.2.3.4:9100 --room GRVT-2847
    ///   gs ptt --peer 192.168.1.5 --peer-port 9000
    ///   gs ptt --peer 192.168.1.5 --peer-port 9000 --listen
    Ptt {
        /// Dirección del relay (HOST). Requiere también --room.
        #[arg(long)]
        relay: Option<String>,
        /// Puerto UDP del relay (default: 9000).
        #[arg(long, default_value_t = 9000)]
        relay_port: u16,
        /// Puerto HTTP de observabilidad del relay para resolver rooms (default: 9100).
        #[arg(long, default_value_t = 9100)]
        relay_obs_port: u16,
        /// Código de sala en formato XXXX-NNNN (requerido con --relay).
        #[arg(long)]
        room: Option<String>,
        /// Peer directo (HOST). Mutualmente exclusivo con --relay.
        #[arg(long)]
        peer: Option<String>,
        /// Puerto del peer directo.
        #[arg(long, default_value_t = 9000)]
        peer_port: u16,
        /// Puerto local de escucha (0 = efímero).
        #[arg(long, default_value_t = 0)]
        port: u16,
        /// Actúa como servidor (espera que el peer conecte primero). Solo P2P.
        #[arg(long)]
        listen: bool,
        /// Dispositivo de entrada de audio (micrófono).
        #[arg(long, default_value = "default")]
        device: String,
        /// Dispositivo de salida de audio (altavoz). Por defecto igual que --device.
        #[arg(long)]
        out_device: Option<String>,
        /// Codec: pcm u opus.
        #[arg(long, default_value = "opus")]
        codec: CodecArg,
    },
}

#[derive(Debug, Subcommand)]
enum RoomAction {
    /// Registra una sala en un relay y obtiene el código de 9 caracteres.
    Create {
        /// Host del relay.
        #[arg(long, default_value = "127.0.0.1")]
        relay: String,
        /// Puerto HTTP de observabilidad del relay (default: 9100).
        #[arg(long, default_value_t = 9100)]
        obs_port: u16,
        /// session_id numérico para la sala (debe ser el mismo que usarán los peers).
        #[arg(long)]
        session_id: u32,
    },
    /// Resuelve un código de sala en un relay y muestra el session_id.
    Join {
        /// Código de sala en formato XXXX-NNNN.
        code: String,
        /// Host del relay.
        #[arg(long, default_value = "127.0.0.1")]
        relay: String,
        /// Puerto HTTP de observabilidad del relay.
        #[arg(long, default_value_t = 9100)]
        obs_port: u16,
    },
    /// Lista todas las salas activas en un relay.
    List {
        #[arg(long, default_value = "127.0.0.1")]
        relay: String,
        #[arg(long, default_value_t = 9100)]
        obs_port: u16,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let filter = EnvFilter::try_new(&cli.log).unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;
    rt.block_on(async move { dispatch(cli.cmd).await })
}

async fn dispatch(cmd: Command) -> Result<()> {
    match cmd {
        Command::Send {
            host,
            port,
            input,
            device,
            codec,
            duration,
            sample_rate,
            channels,
        } => {
            cmd_send(
                host,
                port,
                input,
                device.as_deref(),
                codec,
                duration,
                sample_rate,
                channels,
            )
            .await
        }
        Command::Receive {
            bind,
            port,
            peer,
            peer_port,
            output,
            device,
            codec,
            duration,
            sample_rate,
            channels,
        } => {
            cmd_receive(
                bind,
                port,
                peer,
                peer_port,
                output,
                device.as_deref(),
                codec,
                duration,
                sample_rate,
                channels,
            )
            .await
        }
        Command::Devices => cmd_devices(),
        Command::Bench { mode, duration } => cmd_bench(mode, duration).await,
        Command::Info { host, port } => cmd_info(host, port).await,
        Command::Doctor => cmd_doctor(),
        Command::Relay {
            config,
            bind,
            udp_port,
            ws_port,
            obs_port,
            session_ttl,
            max_sessions,
            max_peers,
        } => cmd_relay(config, bind, udp_port, ws_port, obs_port, session_ttl, max_sessions, max_peers).await,
        Command::Room { action } => cmd_room(action).await,
        Command::Discover { timeout } => cmd_discover(timeout).await,
        Command::Ptt {
            relay,
            relay_port,
            relay_obs_port,
            room,
            peer,
            peer_port,
            port,
            listen,
            device,
            out_device,
            codec,
        } => {
            cmd_ptt(
                relay,
                relay_port,
                relay_obs_port,
                room,
                peer,
                peer_port,
                port,
                listen,
                device,
                out_device.unwrap_or_else(|| "default".to_string()),
                codec,
            )
            .await
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn cmd_send(
    host: String,
    port: u16,
    input: String,
    device: Option<&str>,
    codec_arg: CodecArg,
    duration_s: u64,
    sample_rate: u32,
    channels: u8,
) -> Result<()> {
    let peer: SocketAddr = format!("{host}:{port}")
        .parse()
        .context("invalid peer addr")?;
    let transport = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: "0.0.0.0:0".parse()?,
            ..Default::default()
        })
        .await?,
    );
    let config = Config {
        sample_rate,
        channels,
        frame_duration_ms: 10,
        ..Config::default()
    };
    let codec_id = codec_arg.to_codec_id();
    let cs = CodecSession::new(transport, config.clone(), codec_id)?;
    cs.handshake(SessionRole::Client, peer).await?;
    tracing::info!(session_id = cs.session().session_id(), codec = ?codec_id, "handshake OK");

    let samples_per_frame =
        (sample_rate as usize * config.frame_duration_ms as usize) / 1000 * channels as usize;

    if let Some(dev) = device {
        // Mic capture mode: stream until Ctrl-C.
        let stream_cfg = StreamConfig {
            sample_rate,
            channels,
            frame_duration_ms: config.frame_duration_ms,
        };
        let (_cap, rx) = AudioCapture::start(stream_cfg, Some(dev))?;
        tracing::info!(device = dev, "capturing from mic — press Ctrl-C to stop");
        while let Ok(samples) = rx.recv() {
            cs.send_samples(&samples).await?;
        }
    } else {
        // Synthetic or WAV source.
        let frames_per_sec = 1000 / config.frame_duration_ms.max(1) as u64;
        let total_frames = duration_s * frames_per_sec;

        let iter: Box<dyn Iterator<Item = Vec<i16>>> = if input == "sine" {
            Box::new(sine_frames_i16(samples_per_frame, channels, sample_rate))
        } else {
            Box::new(wav_frames_i16(
                PathBuf::from(input),
                samples_per_frame,
                channels,
            )?)
        };

        let start = Instant::now();
        let mut frame_deadline = start;
        let frame_period = Duration::from_millis(config.frame_duration_ms as u64);
        let mut sent = 0u64;

        for samples in iter.take(total_frames as usize) {
            cs.send_samples(&samples).await?;
            sent += 1;
            frame_deadline += frame_period;
            let now = Instant::now();
            if frame_deadline > now {
                tokio::time::sleep(frame_deadline - now).await;
            }
        }

        let elapsed = start.elapsed();
        tracing::info!(
            frames = sent,
            elapsed_s = elapsed.as_secs_f32(),
            "send complete"
        );
    }

    cs.close().await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn cmd_receive(
    bind: String,
    port: u16,
    peer: String,
    peer_port: u16,
    output: PathBuf,
    device: Option<&str>,
    codec_arg: CodecArg,
    duration_s: u64,
    sample_rate: u32,
    channels: u8,
) -> Result<()> {
    let bind_addr: SocketAddr = format!("{bind}:{port}").parse()?;
    let peer_addr: SocketAddr = format!("{peer}:{peer_port}").parse()?;

    let transport = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr,
            ..Default::default()
        })
        .await?,
    );
    let config = Config {
        sample_rate,
        channels,
        frame_duration_ms: 10,
        ..Config::default()
    };
    let codec_id = codec_arg.to_codec_id();
    let cs = CodecSession::new(transport, config.clone(), codec_id)?;
    cs.handshake(SessionRole::Server, peer_addr).await?;
    tracing::info!(session_id = cs.session().session_id(), codec = ?codec_id, "handshake OK");

    let spec = WavSpec {
        channels: channels as u16,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(&output, spec)?;

    let playback = if let Some(dev) = device {
        let stream_cfg = StreamConfig {
            sample_rate,
            channels,
            frame_duration_ms: config.frame_duration_ms,
        };
        let pb = AudioPlayback::start(stream_cfg, Some(dev))?;
        tracing::info!(device = dev, "playback to speaker active");
        Some(pb)
    } else {
        None
    };

    let deadline = tokio::time::Instant::now() + Duration::from_secs(duration_s);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, cs.recv_samples()).await {
            Ok(Ok(samples)) => {
                for &s in &samples {
                    writer.write_sample(s)?;
                }
                if let Some(ref pb) = playback {
                    let _ = pb.push(samples);
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(?e, "recv error");
                break;
            }
            Err(_) => break,
        }
    }

    writer.finalize()?;
    tracing::info!(path = %output.display(), "wav written");
    cs.close().await?;
    Ok(())
}

fn cmd_devices() -> Result<()> {
    println!("─── Input devices ───");
    match gravital_talk_io::list_input_devices() {
        Ok(devs) if devs.is_empty() => println!("  (none)"),
        Ok(devs) => {
            for d in devs {
                let tag = if d.is_default { " [default]" } else { "" };
                println!("  {}{}", d.name, tag);
            }
        }
        Err(e) => println!("  error: {e}"),
    }

    println!("─── Output devices ───");
    match gravital_talk_io::list_output_devices() {
        Ok(devs) if devs.is_empty() => println!("  (none)"),
        Ok(devs) => {
            for d in devs {
                let tag = if d.is_default { " [default]" } else { "" };
                println!("  {}{}", d.name, tag);
            }
        }
        Err(e) => println!("  error: {e}"),
    }
    Ok(())
}

async fn cmd_bench(mode: String, duration_s: u64) -> Result<()> {
    if mode != "loopback" {
        bail!("only 'loopback' bench mode is supported");
    }
    use gravital_talk::{PacketBuilder, PacketHeader, PacketView};
    let header = PacketHeader::new(0x10, 0xDEAD_BEEF, 0, 0);
    let payload = vec![0u8; 960];
    let mut out = vec![0u8; 1200];

    let deadline = Instant::now() + Duration::from_secs(duration_s);
    let mut iters = 0u64;
    let mut max_ns = 0u128;
    let mut sum_ns = 0u128;
    while Instant::now() < deadline {
        let t0 = Instant::now();
        let n = PacketBuilder::new(header, &payload)
            .encode(&mut out)
            .unwrap();
        let _v = PacketView::decode(&out[..n]).unwrap();
        let elapsed = t0.elapsed().as_nanos();
        sum_ns += elapsed;
        if elapsed > max_ns {
            max_ns = elapsed;
        }
        iters += 1;
    }
    let avg = sum_ns / iters.max(1) as u128;
    println!("encode+decode loopback: {iters} iters, avg {avg} ns, max {max_ns} ns, payload=960B");
    Ok(())
}

async fn cmd_info(host: String, port: u16) -> Result<()> {
    let peer: SocketAddr = format!("{host}:{port}").parse()?;
    let transport = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: "0.0.0.0:0".parse()?,
            ..Default::default()
        })
        .await?,
    );
    let session = Session::new(transport, Config::default());
    let started = Instant::now();
    session.handshake(SessionRole::Client, peer).await?;
    let rtt = started.elapsed();

    let fill = session.jitter_buffer().fill_percent();
    let snap = session.metrics().snapshot(fill);
    println!("─── session info ───");
    println!(" peer           : {peer}");
    println!(" session_id     : 0x{:08X}", session.session_id());
    println!(" handshake_rtt  : {:?}", rtt);
    println!(" protocol       : v{}", gravital_talk::PROTOCOL_VERSION);
    println!(" state          : {:?}", session.state().await);
    println!(" estimated MOS  : {:.2}", snap.estimated_mos);
    println!(" loss%          : {:.2}", snap.loss_percent);
    println!(" jitter ms      : {:.2}", snap.jitter_ms);
    session.close().await?;
    Ok(())
}

fn cmd_doctor() -> Result<()> {
    println!("Gravital Talk doctor");
    println!(" version        : {}", env!("CARGO_PKG_VERSION"));
    println!(" protocol       : v{}", gravital_talk::PROTOCOL_VERSION);
    println!(" target_os      : {}", std::env::consts::OS);
    println!(" target_arch    : {}", std::env::consts::ARCH);

    match std::net::UdpSocket::bind("0.0.0.0:0") {
        Ok(s) => {
            let addr = s.local_addr().map(|a| a.to_string()).unwrap_or_default();
            println!(" udp bind       : OK (ephemeral {addr})");
        }
        Err(e) => println!(" udp bind       : FAILED: {e}"),
    }
    Ok(())
}

async fn cmd_relay(
    config_path: Option<PathBuf>,
    bind: String,
    udp_port: u16,
    ws_port: u16,
    obs_port: u16,
    session_ttl: u64,
    max_sessions: usize,
    max_peers: usize,
) -> Result<()> {
    use gravital_talk_relay::{
        config::RelayConfig, metrics::RelayMetrics, observability, router::Router, udp, ws,
    };
    use tokio::net::{TcpListener, UdpSocket};

    // Build config: file first, then CLI overrides.
    let mut cfg = match config_path {
        Some(ref p) => RelayConfig::from_file(p)
            .with_context(|| format!("failed to load relay config from {}", p.display()))?,
        None => RelayConfig::default(),
    };

    // CLI flags override file values.
    cfg.udp_bind = format!("{bind}:{udp_port}").parse()?;
    cfg.ws_bind = format!("{bind}:{ws_port}").parse()?;
    cfg.observability_bind = format!("{bind}:{obs_port}").parse()?;
    cfg.session_ttl_secs = session_ttl;
    cfg.max_sessions = max_sessions;
    cfg.max_peers_per_session = max_peers;

    tracing::info!(
        udp    = %cfg.udp_bind,
        ws     = %cfg.ws_bind,
        obs    = %cfg.observability_bind,
        ttl    = cfg.session_ttl_secs,
        max_sessions = cfg.max_sessions,
        "starting gs relay"
    );

    let metrics = RelayMetrics::new();
    let router = Arc::new(Router::new(cfg.max_sessions, cfg.max_peers_per_session, metrics));

    let udp_socket = Arc::new(UdpSocket::bind(cfg.udp_bind).await
        .with_context(|| format!("cannot bind UDP {}", cfg.udp_bind))?);
    let ws_listener = TcpListener::bind(cfg.ws_bind).await
        .with_context(|| format!("cannot bind WS {}", cfg.ws_bind))?;
    let obs_listener = TcpListener::bind(cfg.observability_bind).await
        .with_context(|| format!("cannot bind observability {}", cfg.observability_bind))?;

    println!(
        "Gravital Talk relay running.\n  UDP  → {}\n  WS   → {}\n  HTTP → {} (/metrics /healthz /api/rooms)\nPress Ctrl-C to stop.",
        cfg.udp_bind, cfg.ws_bind, cfg.observability_bind
    );

    // GC: evict sessions idle more than TTL.
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

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            println!("\nShutting down relay.");
        }
        r = udp_task => {
            tracing::error!(?r, "UDP relay task exited unexpectedly");
        }
        r = ws_task => {
            tracing::error!(?r, "WS relay task exited unexpectedly");
        }
        r = obs_task => {
            tracing::error!(?r, "observability task exited unexpectedly");
        }
    }

    Ok(())
}

async fn cmd_room(action: RoomAction) -> Result<()> {
    match action {
        RoomAction::Create { relay, obs_port, session_id } => {
            let body = format!(r#"{{"session_id":{session_id}}}"#);
            let resp = http_post(&relay, obs_port, "/api/rooms", &body).await?;
            println!("{resp}");
        }
        RoomAction::Join { code, relay, obs_port } => {
            let path = format!("/api/rooms/{code}");
            let resp = http_get(&relay, obs_port, &path).await?;
            println!("{resp}");
        }
        RoomAction::List { relay, obs_port } => {
            let resp = http_get(&relay, obs_port, "/api/rooms").await?;
            println!("{resp}");
        }
    }
    Ok(())
}

async fn cmd_discover(timeout_s: u64) -> Result<()> {
    use gravital_talk_transport::discovery;
    println!("Scanning LAN for Gravital Talk peers ({timeout_s}s)...");
    let timeout = std::time::Duration::from_secs(timeout_s);
    match discovery::discover_lan(timeout) {
        Ok(peers) if peers.is_empty() => println!("No peers found."),
        Ok(peers) => {
            println!("Found {} peer(s):", peers.len());
            for p in peers {
                println!("  {} — session_id={} — \"{}\"", p.addr, p.session_id, p.name);
            }
        }
        Err(e) => println!("Discovery error: {e}"),
    }
    Ok(())
}

/// Minimal HTTP GET using tokio TcpStream.
async fn http_get(host: &str, port: u16, path: &str) -> Result<String> {
    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    let mut stream = tokio::net::TcpStream::connect(addr).await?;
    let req = format!(
        "GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n"
    );
    stream.write_all(req.as_bytes()).await?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    extract_http_body(&buf)
}

/// Minimal HTTP POST using tokio TcpStream.
async fn http_post(host: &str, port: u16, path: &str, body: &str) -> Result<String> {
    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    let mut stream = tokio::net::TcpStream::connect(addr).await?;
    let req = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream.write_all(req.as_bytes()).await?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    extract_http_body(&buf)
}

/// Extracts the body from a raw HTTP/1.1 response (after the blank line).
fn extract_http_body(raw: &[u8]) -> Result<String> {
    let sep = b"\r\n\r\n";
    if let Some(pos) = raw.windows(4).position(|w| w == sep) {
        let body = &raw[pos + 4..];
        Ok(String::from_utf8_lossy(body).trim().to_string())
    } else {
        anyhow::bail!("malformed HTTP response (no header separator)");
    }
}

// ─── gs ptt ──────────────────────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)]
async fn cmd_ptt(
    relay: Option<String>,
    relay_port: u16,
    relay_obs_port: u16,
    room: Option<String>,
    peer_host: Option<String>,
    peer_port: u16,
    local_port: u16,
    listen: bool,
    in_device: String,
    out_device: String,
    codec_arg: CodecArg,
) -> Result<()> {
    // ── Determinar peer y rol ───────────────────────────────────────────────
    let (peer_addr, role, via_relay): (SocketAddr, SessionRole, bool) = match (&relay, &room, &peer_host) {
        (Some(relay_host), Some(room_code), None) => {
            // Modo relay: resolver room → session_id, luego conectar al relay UDP
            let path = format!("/api/rooms/{room_code}");
            let resp = http_get(relay_host, relay_obs_port, &path).await
                .context("failed to resolve room code — is the relay running?")?;
            tracing::info!(room = room_code, response = %resp, "room resolved");
            let relay_udp: SocketAddr = format!("{relay_host}:{relay_port}").parse()?;
            (relay_udp, SessionRole::Client, true)
        }
        (None, None, Some(host)) => {
            let peer: SocketAddr = format!("{host}:{peer_port}").parse()?;
            let role = if listen { SessionRole::Server } else { SessionRole::Client };
            (peer, role, false)
        }
        _ => bail!("use either --relay + --room  OR  --peer [--listen]"),
    };

    let bind_addr: SocketAddr = format!("0.0.0.0:{local_port}").parse()?;
    let codec_id = codec_arg.to_codec_id();
    let config = Config {
        sample_rate: 48_000,
        channels: 1,
        frame_duration_ms: 20,
        ..Config::default()
    };

    // ── Setup audio I/O (sobrevive a reconexiones) ──────────────────────────
    let stream_cfg = StreamConfig {
        sample_rate: config.sample_rate,
        channels: config.channels,
        frame_duration_ms: config.frame_duration_ms,
    };
    let playback = AudioPlayback::start(stream_cfg, Some(&out_device))
        .context("failed to open output device")?;
    let tone_tx = playback.sender();

    // ── Flags compartidos (sobreviven reconexiones) ─────────────────────────
    let ptt_on = Arc::new(AtomicBool::new(false));
    let quit = Arc::new(AtomicBool::new(false));  // salida definitiva

    // ── Activar UI de terminal (una vez) ────────────────────────────────────
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let mode_str = if via_relay {
        format!("relay {} room {}", relay.as_deref().unwrap_or("?"), room.as_deref().unwrap_or("?"))
    } else {
        format!("direct → {peer_addr}")
    };

    // ── Bucle de reconexión ─────────────────────────────────────────────────
    let mut reconnect_delay = Duration::from_secs(2);
    let result: Result<()> = loop {
        // Canal de señal de desconexión: el task recv avisa cuando el peer cierra.
        let (disc_tx, disc_rx) = tokio::sync::mpsc::channel::<()>(1);

        // Crear sesión fresca para cada intento de conexión.
        let transport = match UdpTransport::bind(UdpConfig { bind_addr, ..Default::default() }).await {
            Ok(t) => Arc::new(t),
            Err(e) => { break Err(anyhow::anyhow!("cannot bind UDP: {e}")); }
        };
        let cs = match CodecSession::new(transport, config.clone(), codec_id) {
            Ok(s) => Arc::new(s),
            Err(e) => { break Err(e.into()); }
        };

        // Handshake (mostrar estado en pantalla antes de entrar al UI loop).
        {
            use std::io::Write;
            print!("\x1B[2J\x1B[H");
            println!("Conectando ({mode_str})...");
            stdout.flush().ok();
        }

        match cs.handshake(role, peer_addr).await {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!(?e, "handshake failed, will retry");
                // Mostrar error brevemente en pantalla.
                print!("\x1B[2J\x1B[H");
                println!("Handshake fallido: {e}\nReconectando en {}s...", reconnect_delay.as_secs());
                std::io::stdout().flush().ok();
                tokio::time::sleep(reconnect_delay).await;
                reconnect_delay = (reconnect_delay * 2).min(Duration::from_secs(30));
                continue;
            }
        }
        reconnect_delay = Duration::from_secs(2); // reset backoff on success

        // ── Task de recepción + playback ─────────────────────────────────────
        let cs_rx = cs.clone();
        let pb_rx = playback.sender();
        let quit_rx = quit.clone();
        let recv_handle = tokio::spawn(async move {
            while !quit_rx.load(Ordering::Acquire) {
                match cs_rx.recv_samples().await {
                    Ok(samples) => { let _ = pb_rx.send(samples); }
                    Err(e) => {
                        tracing::debug!(?e, "recv_samples error — signaling disconnect");
                        let _ = disc_tx.send(()).await;
                        break;
                    }
                }
            }
        });

        // ── Task de captura + envío ──────────────────────────────────────────
        let cs_tx = cs.clone();
        let ptt_tx = ptt_on.clone();
        let quit_tx = quit.clone();
        let in_device_cap = in_device.clone();
        let send_handle = tokio::spawn(async move {
            let mut capture: Option<(AudioCapture, std::sync::mpsc::Receiver<Vec<i16>>)> = None;
            loop {
                if quit_tx.load(Ordering::Acquire) { break; }
                if ptt_tx.load(Ordering::Acquire) {
                    if capture.is_none() {
                        match AudioCapture::start(stream_cfg, Some(in_device_cap.as_str())) {
                            Ok((cap, rx)) => { capture = Some((cap, rx)); }
                            Err(e) => {
                                tracing::warn!(?e, "audio capture start failed");
                                tokio::time::sleep(Duration::from_millis(100)).await;
                                continue;
                            }
                        }
                    }
                    if let Some((_, ref rx)) = capture {
                        match rx.try_recv() {
                            Ok(samples) => {
                                if let Err(e) = cs_tx.send_samples(&samples).await {
                                    tracing::debug!(?e, "send_samples error");
                                }
                            }
                            Err(std::sync::mpsc::TryRecvError::Empty) => {
                                tokio::time::sleep(Duration::from_millis(5)).await;
                            }
                            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                                capture = None;
                            }
                        }
                    }
                } else {
                    capture = None;
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            }
        });

        // ── UI interactiva ───────────────────────────────────────────────────
        let ui_result = ptt_ui_loop(
            &cs,
            &ptt_on,
            &quit,
            tone_tx.clone(),
            config.sample_rate,
            disc_rx,
        ).await;

        // Limpiar tasks y sesión.
        recv_handle.abort();
        send_handle.abort();
        ptt_on.store(false, Ordering::Release);
        let _ = cs.session().ptt_release().await;
        let _ = cs.close().await;

        match ui_result {
            Ok(PttUiResult::Quit) => break Ok(()),
            Ok(PttUiResult::Reconnect) => {
                // Loop de reconexión continúa.
                continue;
            }
            Err(e) => break Err(e),
        }
    };

    // Restaurar terminal siempre.
    let _ = disable_raw_mode();
    let _ = execute!(std::io::stdout(), LeaveAlternateScreen);

    result
}

#[derive(Debug)]
enum PttUiResult {
    Quit,
    Reconnect,
}

async fn ptt_ui_loop(
    cs: &Arc<CodecSession>,
    ptt_on: &Arc<AtomicBool>,
    quit: &Arc<AtomicBool>,
    tone_tx: std::sync::mpsc::Sender<Vec<i16>>,
    sample_rate: u32,
    mut disc_rx: tokio::sync::mpsc::Receiver<()>,
) -> Result<PttUiResult> {
    use std::io::Write;
    let mut stdout = std::io::stdout();

    let mut last_render = Instant::now();
    let render_interval = Duration::from_millis(200);

    loop {
        // ── Señal de desconexión del peer ───────────────────────────────────
        if disc_rx.try_recv().is_ok() {
            // Mostrar mensaje de reconexión brevemente y volver al bucle.
            print!("\x1B[2J\x1B[H");
            println!("╔══════════════════════════════════════════════════╗");
            println!("║          GRAVITAL TALK — PTT                     ║");
            println!("╠══════════════════════════════════════════════════╣");
            println!("║  ⚠  PEER DESCONECTADO — Reconectando...          ║");
            println!("╚══════════════════════════════════════════════════╝");
            stdout.flush().ok();
            tokio::time::sleep(Duration::from_secs(2)).await;
            return Ok(PttUiResult::Reconnect);
        }

        // ── Render UI ───────────────────────────────────────────────────────
        if last_render.elapsed() >= render_interval {
            last_render = Instant::now();

            let ptt = ptt_on.load(Ordering::Acquire);
            let peer_ptt = cs.session().is_peer_ptt_active();
            let snap = cs.session().metrics().snapshot(cs.session().jitter_buffer().fill_percent());
            let sid = cs.session().session_id();

            // Limpiar y redibujar.
            print!("\x1B[2J\x1B[H"); // clear screen, cursor home
            println!("╔══════════════════════════════════════════════════╗");
            println!("║          GRAVITAL TALK — PTT                     ║");
            println!("╠══════════════════════════════════════════════════╣");
            println!("║  Session: 0x{sid:08X}                          ║");
            println!(
                "║  Quality: MOS {:.1}  Loss {:.1}%  Jitter {:.0}ms    ║",
                snap.estimated_mos, snap.loss_percent, snap.jitter_ms
            );
            println!("╠══════════════════════════════════════════════════╣");

            if ptt {
                println!("║  ● TRANSMITIENDO  — suelta [ESPACIO] para parar  ║");
            } else {
                println!("║  ○ En espera      — [ESPACIO] para hablar         ║");
            }

            if peer_ptt {
                println!("║  ◉ PEER TRANSMITIENDO                            ║");
            } else {
                println!("║  ○ Peer escuchando                               ║");
            }

            println!("╠══════════════════════════════════════════════════╣");
            println!("║  [ESPACIO] toggle PTT  •  [Q] salir               ║");
            println!("╚══════════════════════════════════════════════════╝");
            stdout.flush()?;
        }

        // ── Eventos de teclado ──────────────────────────────────────────────
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Char('Q') => {
                            quit.store(true, Ordering::Release);
                            return Ok(PttUiResult::Quit);
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            quit.store(true, Ordering::Release);
                            return Ok(PttUiResult::Quit);
                        }
                        KeyCode::Char(' ') => {
                            let was_on = ptt_on.load(Ordering::Acquire);
                            ptt_on.store(!was_on, Ordering::Release);
                            if !was_on {
                                let _ = cs.session().ptt_press().await;
                                play_tone(gravital_talk_transport::generate_pcm_tone(880.0, 100, sample_rate), &tone_tx);
                            } else {
                                let _ = cs.session().ptt_release().await;
                                play_tone(gravital_talk_transport::generate_pcm_tone(440.0, 80, sample_rate), &tone_tx);
                            }
                        }
                        // Tecla 'T' como alternativa
                        KeyCode::Char('t') | KeyCode::Char('T') => {
                            let was_on = ptt_on.load(Ordering::Acquire);
                            ptt_on.store(!was_on, Ordering::Release);
                            if !was_on {
                                let _ = cs.session().ptt_press().await;
                                play_tone(gravital_talk_transport::generate_pcm_tone(880.0, 100, sample_rate), &tone_tx);
                            } else {
                                let _ = cs.session().ptt_release().await;
                                play_tone(gravital_talk_transport::generate_pcm_tone(440.0, 80, sample_rate), &tone_tx);
                            }
                        }
                        _ => {}
                    }
                }
                Event::Resize(_, _) => {} // Refrescar en el próximo tick.
                _ => {}
            }
        }
    }

    Ok(PttUiResult::Quit)
}

/// Sends PCM i16 samples to the playback channel (non-blocking; drops on full channel).
fn play_tone(samples: Vec<i16>, tx: &std::sync::mpsc::Sender<Vec<i16>>) {
    let _ = tx.send(samples);
}

fn sine_frames_i16(
    samples_per_frame: usize,
    channels: u8,
    sample_rate: u32,
) -> impl Iterator<Item = Vec<i16>> {
    let mut phase: f32 = 0.0;
    let step = 2.0 * std::f32::consts::PI * 440.0 / sample_rate as f32;
    std::iter::from_fn(move || {
        let mut buf = Vec::with_capacity(samples_per_frame);
        let mono_samples = samples_per_frame / channels as usize;
        for _ in 0..mono_samples {
            let sample = (phase.sin() * 16_000.0) as i16;
            for _c in 0..channels {
                buf.push(sample);
            }
            phase += step;
            if phase > std::f32::consts::TAU {
                phase -= std::f32::consts::TAU;
            }
        }
        Some(buf)
    })
}

fn wav_frames_i16(
    path: PathBuf,
    samples_per_frame: usize,
    channels: u8,
) -> Result<impl Iterator<Item = Vec<i16>>> {
    let reader = hound::WavReader::open(&path)?;
    let spec = reader.spec();
    if spec.channels != channels as u16 {
        bail!(
            "wav channels {} != session channels {}",
            spec.channels,
            channels
        );
    }
    let mut samples: Vec<i16> = reader
        .into_samples::<i16>()
        .collect::<std::result::Result<Vec<_>, _>>()?;
    let per_frame = samples_per_frame;
    Ok(std::iter::from_fn(move || {
        if samples.is_empty() {
            return None;
        }
        let take = per_frame.min(samples.len());
        Some(samples.drain(..take).collect())
    }))
}
