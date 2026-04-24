//! `gs` — CLI de Gravital Sound.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use gravital_sound::{Config, Session, SessionRole, Transport, UdpConfig, UdpTransport};
use hound::{SampleFormat, WavSpec, WavWriter};
use tracing_subscriber::EnvFilter;

/// Gravital Sound — protocolo moderno de audio en tiempo real.
#[derive(Debug, Parser)]
#[command(name = "gs", version, about, long_about = None)]
struct Cli {
    /// Nivel de log (`error`, `warn`, `info`, `debug`, `trace`).
    #[arg(long, env = "GS_LOG", default_value = "info")]
    log: String,

    #[command(subcommand)]
    cmd: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Envía audio PCM a un peer (`--input` puede ser `sine` o un archivo WAV).
    Send {
        /// Host destino.
        #[arg(long)]
        host: String,
        /// Puerto destino.
        #[arg(long, default_value_t = 9000)]
        port: u16,
        /// `sine` (sintetiza tono) o ruta a WAV PCM 16 bits.
        #[arg(long, default_value = "sine")]
        input: String,
        /// Duración en segundos para el modo `sine`.
        #[arg(long, default_value_t = 10)]
        duration: u64,
        /// Sample rate.
        #[arg(long, default_value_t = 48_000)]
        sample_rate: u32,
        /// Canales (1 o 2).
        #[arg(long, default_value_t = 1)]
        channels: u8,
    },
    /// Recibe audio y lo escribe a un WAV.
    Receive {
        /// Dirección de bind.
        #[arg(long, default_value = "0.0.0.0")]
        bind: String,
        /// Puerto.
        #[arg(long, default_value_t = 9000)]
        port: u16,
        /// Peer esperado (requerido para handshake server).
        #[arg(long)]
        peer: String,
        /// Puerto del peer.
        #[arg(long)]
        peer_port: u16,
        /// Ruta de salida WAV.
        #[arg(long)]
        output: PathBuf,
        /// Duración máxima en segundos antes de detener la captura.
        #[arg(long, default_value_t = 30)]
        duration: u64,
        /// Sample rate para el WAV.
        #[arg(long, default_value_t = 48_000)]
        sample_rate: u32,
        /// Canales.
        #[arg(long, default_value_t = 1)]
        channels: u8,
    },
    /// Benchmark loopback: mide latencia encode→socket→decode en localhost.
    Bench {
        /// `loopback` es el único modo soportado en MVP.
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
    /// Relay básico que hace echo de paquetes entre pares con el mismo `session_id`.
    Relay {
        #[arg(long, default_value = "0.0.0.0")]
        bind: String,
        #[arg(long, default_value_t = 9100)]
        port: u16,
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
            duration,
            sample_rate,
            channels,
        } => cmd_send(host, port, input, duration, sample_rate, channels).await,
        Command::Receive {
            bind,
            port,
            peer,
            peer_port,
            output,
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
                duration,
                sample_rate,
                channels,
            )
            .await
        }
        Command::Bench { mode, duration } => cmd_bench(mode, duration).await,
        Command::Info { host, port } => cmd_info(host, port).await,
        Command::Doctor => cmd_doctor(),
        Command::Relay { bind, port } => cmd_relay(bind, port).await,
    }
}

async fn cmd_send(
    host: String,
    port: u16,
    input: String,
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
        ..Config::default()
    };
    let session = Session::new(transport, config.clone());
    session.handshake(SessionRole::Client, peer).await?;
    tracing::info!(session_id = session.session_id(), "handshake OK");

    let samples_per_frame =
        (config.sample_rate as u64 * config.frame_duration_ms as u64 / 1000) as usize;
    let frames_per_sec = 1000 / config.frame_duration_ms.max(1) as u64;
    let total_frames = duration_s * frames_per_sec;

    let iter: Box<dyn Iterator<Item = Vec<u8>>> = if input == "sine" {
        Box::new(sine_frames(
            samples_per_frame,
            config.channels,
            config.sample_rate,
        ))
    } else {
        Box::new(wav_frames(
            PathBuf::from(input),
            samples_per_frame,
            config.channels,
        )?)
    };

    let start = Instant::now();
    let mut frame_deadline = start;
    let frame_period = Duration::from_millis(config.frame_duration_ms as u64);
    let mut sent = 0u64;

    for frame in iter.take(total_frames as usize) {
        session.send_audio(&frame).await?;
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
        kbps = (sent * samples_per_frame as u64 * 2 * 8) as f32 / elapsed.as_secs_f32() / 1000.0,
        "send complete"
    );
    session.close().await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn cmd_receive(
    bind: String,
    port: u16,
    peer: String,
    peer_port: u16,
    output: PathBuf,
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
        ..Config::default()
    };
    let session = Session::new(transport, config.clone());
    session.handshake(SessionRole::Server, peer_addr).await?;
    tracing::info!(session_id = session.session_id(), "handshake OK");

    let spec = WavSpec {
        channels: channels as u16,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(&output, spec)?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(duration_s);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, session.recv_audio()).await {
            Ok(Ok(frame)) => {
                for chunk in frame.payload.chunks_exact(2) {
                    let s = i16::from_le_bytes([chunk[0], chunk[1]]);
                    writer.write_sample(s)?;
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
    session.close().await?;
    Ok(())
}

async fn cmd_bench(mode: String, duration_s: u64) -> Result<()> {
    if mode != "loopback" {
        bail!("only 'loopback' bench mode is supported in MVP");
    }
    use gravital_sound::{PacketBuilder, PacketHeader, PacketView};
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
    println!(" protocol       : v{}", gravital_sound::PROTOCOL_VERSION);
    println!(" state          : {:?}", session.state().await);
    println!(" estimated MOS  : {:.2}", snap.estimated_mos);
    println!(" loss%          : {:.2}", snap.loss_percent);
    println!(" jitter ms      : {:.2}", snap.jitter_ms);
    session.close().await?;
    Ok(())
}

fn cmd_doctor() -> Result<()> {
    println!("Gravital Sound doctor");
    println!(" version        : {}", env!("CARGO_PKG_VERSION"));
    println!(" protocol       : v{}", gravital_sound::PROTOCOL_VERSION);
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

async fn cmd_relay(bind: String, port: u16) -> Result<()> {
    let addr: SocketAddr = format!("{bind}:{port}").parse()?;
    let t = UdpTransport::bind(UdpConfig {
        bind_addr: addr,
        reuse_port: true,
        ..Default::default()
    })
    .await?;
    tracing::info!(local = ?t.local_addr(), "relay listening");
    // Relay naive: mantiene la última dirección vista por session_id y reenvía.
    use std::collections::HashMap;
    let mut routes: HashMap<u32, SocketAddr> = HashMap::new();
    let mut buf = vec![0u8; 1500];
    loop {
        let (n, from) = t.recv(&mut buf).await?;
        let slice = &buf[..n];
        match gravital_sound::PacketView::decode(slice) {
            Ok(view) => {
                let sid = view.header().session_id;
                if let Some(other) = routes.get(&sid).copied() {
                    if other != from {
                        let _ = t.send_to(slice, other).await;
                        continue;
                    }
                }
                routes.insert(sid, from);
            }
            Err(e) => {
                tracing::debug!(?e, "dropping bad packet");
            }
        }
    }
}

fn sine_frames(
    samples_per_frame: usize,
    channels: u8,
    sample_rate: u32,
) -> impl Iterator<Item = Vec<u8>> {
    let mut phase: f32 = 0.0;
    let step = 2.0 * std::f32::consts::PI * 440.0 / sample_rate as f32;
    std::iter::from_fn(move || {
        let mut buf = Vec::with_capacity(samples_per_frame * channels as usize * 2);
        for _ in 0..samples_per_frame {
            let sample = (phase.sin() * 16_000.0) as i16;
            for _c in 0..channels {
                buf.extend_from_slice(&sample.to_le_bytes());
            }
            phase += step;
            if phase > std::f32::consts::TAU {
                phase -= std::f32::consts::TAU;
            }
        }
        Some(buf)
    })
}

fn wav_frames(
    path: PathBuf,
    samples_per_frame: usize,
    channels: u8,
) -> Result<impl Iterator<Item = Vec<u8>>> {
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
    let per_frame = samples_per_frame * channels as usize;
    Ok(std::iter::from_fn(move || {
        if samples.is_empty() {
            return None;
        }
        let take = per_frame.min(samples.len());
        let chunk: Vec<i16> = samples.drain(..take).collect();
        let mut buf = Vec::with_capacity(chunk.len() * 2);
        for s in chunk {
            buf.extend_from_slice(&s.to_le_bytes());
        }
        Some(buf)
    }))
}
