//! `gs` — CLI de Gravital Sound.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use clap::{Parser, Subcommand};
use gravital_sound::{
    CodecId, CodecSession, Config, Session, SessionRole, Transport, UdpConfig, UdpTransport,
};
use gravital_sound_io::{AudioCapture, AudioPlayback, StreamConfig};
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
        Command::Relay { bind, port } => cmd_relay(bind, port).await,
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
    match gravital_sound_io::list_input_devices() {
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
    match gravital_sound_io::list_output_devices() {
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
