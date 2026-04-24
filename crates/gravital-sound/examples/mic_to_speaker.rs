//! Mic → encode → decode → speaker pipeline en un solo proceso.
//!
//! Mide latencia end-to-end con hdrhistogram.
//! En entornos sin hardware de audio activa un source sintético 440 Hz.
//!
//! Uso:
//!   cargo run --release --example mic_to_speaker [pcm|opus] [seconds]

use std::sync::mpsc::{self, Receiver};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gravital_sound::{CodecId, CodecSession, Config, SessionRole, UdpConfig, UdpTransport};
use gravital_sound_io::{AudioCapture, AudioPlayback, StreamConfig};
use hdrhistogram::Histogram;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt().with_env_filter("info").init();

    let mut args = std::env::args().skip(1);
    let codec_str = args.next().unwrap_or_else(|| "pcm".into());
    let duration_s: u64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(5);

    let codec_id = match codec_str.as_str() {
        #[cfg(feature = "opus")]
        "opus" => CodecId::Opus,
        _ => CodecId::Pcm,
    };

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()?;

    rt.block_on(run(codec_id, duration_s))
}

async fn run(codec_id: CodecId, duration_s: u64) -> anyhow::Result<()> {
    let server_addr = "127.0.0.1:19700".parse()?;
    let client_addr = "127.0.0.1:19701".parse()?;

    let srv_transport = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: server_addr,
            ..Default::default()
        })
        .await?,
    );
    let cli_transport = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: client_addr,
            ..Default::default()
        })
        .await?,
    );

    let config = Config {
        frame_duration_ms: 10,
        ..Config::default()
    };
    let srv_session = Arc::new(CodecSession::new(srv_transport, config.clone(), codec_id)?);
    let cli_session = Arc::new(CodecSession::new(cli_transport, config.clone(), codec_id)?);

    let srv = srv_session.clone();
    let hs_srv = tokio::spawn(async move { srv.handshake(SessionRole::Server, client_addr).await });
    cli_session
        .handshake(SessionRole::Client, server_addr)
        .await?;
    hs_srv.await??;
    tracing::info!(codec = ?codec_id, "loopback handshake OK");

    let stream_cfg = StreamConfig {
        sample_rate: config.sample_rate,
        channels: config.channels,
        frame_duration_ms: config.frame_duration_ms,
    };

    // Try real mic; fall back to synthetic if unavailable.
    let (_capture_hold, rx): (Option<AudioCapture>, Receiver<Vec<i16>>) =
        match AudioCapture::start(stream_cfg, Some("default")) {
            Ok((cap, rx)) => (Some(cap), rx),
            Err(e) => {
                tracing::warn!("no input device ({e}), using 440 Hz sine source");
                (None, start_sine_source(stream_cfg))
            }
        };

    let playback: Option<AudioPlayback> = match AudioPlayback::start(stream_cfg, Some("default")) {
        Ok(pb) => Some(pb),
        Err(e) => {
            tracing::warn!("no output device ({e}), skipping speaker playback");
            None
        }
    };

    let mut hist: Histogram<u64> = Histogram::new(3)?;
    let deadline = Instant::now() + Duration::from_secs(duration_s);

    tracing::info!("streaming for {}s…", duration_s);
    loop {
        if Instant::now() >= deadline {
            break;
        }
        let samples = match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(s) => s,
            Err(_) => break,
        };

        let t0 = Instant::now();
        cli_session.send_samples(&samples).await?;

        match tokio::time::timeout(Duration::from_millis(200), srv_session.recv_samples()).await {
            Ok(Ok(decoded)) => {
                let us = t0.elapsed().as_micros() as u64;
                let _ = hist.record(us);
                if let Some(ref pb) = playback {
                    let _ = pb.push(decoded);
                }
            }
            Ok(Err(e)) => tracing::warn!(?e, "recv error"),
            Err(_) => tracing::warn!("recv timeout"),
        }
    }

    cli_session.close().await?;
    srv_session.close().await?;

    if !hist.is_empty() {
        println!("\n─── Latency end-to-end ({codec_id:?} codec) ───");
        println!(" samples  : {}", hist.len());
        println!(" p50      : {} µs", hist.value_at_quantile(0.50));
        println!(" p90      : {} µs", hist.value_at_quantile(0.90));
        println!(" p99      : {} µs", hist.value_at_quantile(0.99));
        println!(" max      : {} µs", hist.max());
    }
    Ok(())
}

fn start_sine_source(cfg: StreamConfig) -> Receiver<Vec<i16>> {
    let (tx, rx) = mpsc::channel();
    let frame = cfg.samples_per_frame();
    std::thread::spawn(move || {
        let mut phase: f32 = 0.0;
        let step = 2.0 * std::f32::consts::PI * 440.0 / cfg.sample_rate as f32;
        loop {
            let mut buf = Vec::with_capacity(frame);
            let mono = frame / cfg.channels as usize;
            for _ in 0..mono {
                let s = (phase.sin() * 16_000.0) as i16;
                for _ in 0..cfg.channels {
                    buf.push(s);
                }
                phase += step;
                if phase > std::f32::consts::TAU {
                    phase -= std::f32::consts::TAU;
                }
            }
            if tx.send(buf).is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(cfg.frame_duration_ms as u64));
        }
    });
    rx
}
