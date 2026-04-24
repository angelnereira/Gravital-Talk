//! Full-duplex VoIP peer sobre dos puertos UDP.
//!
//! Simula una llamada bidireccional de `duration` segundos.
//!
//! Uso:
//!   cargo run --release --example voip_peer [pcm|opus] [seconds]

use std::sync::Arc;
use std::time::Duration;

use gravital_sound::{CodecId, CodecSession, Config, SessionRole, UdpConfig, UdpTransport};

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
    let addr_a = "127.0.0.1:19800".parse()?;
    let addr_b = "127.0.0.1:19801".parse()?;

    let ta = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: addr_a,
            ..Default::default()
        })
        .await?,
    );
    let tb = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: addr_b,
            ..Default::default()
        })
        .await?,
    );

    let config = Config {
        frame_duration_ms: 10,
        ..Config::default()
    };
    let frame_samples = (config.sample_rate as usize * config.frame_duration_ms as usize) / 1000;

    let peer_a = Arc::new(CodecSession::new(ta, config.clone(), codec_id)?);
    let peer_b = Arc::new(CodecSession::new(tb, config.clone(), codec_id)?);

    let pb = peer_b.clone();
    let hs_b = tokio::spawn(async move { pb.handshake(SessionRole::Server, addr_a).await });
    peer_a.handshake(SessionRole::Client, addr_b).await?;
    hs_b.await??;
    tracing::info!(codec = ?codec_id, "both peers connected");

    let silence = Arc::new(vec![0i16; frame_samples]);
    let deadline = tokio::time::Instant::now() + Duration::from_secs(duration_s);

    // Spawn send tasks for A→B and B→A.
    let pa = peer_a.clone();
    let silence_a = silence.clone();
    let send_a = tokio::spawn(async move {
        let mut count = 0u64;
        while tokio::time::Instant::now() < deadline {
            pa.send_samples(&silence_a).await?;
            count += 1;
            tokio::time::sleep(Duration::from_millis(config.frame_duration_ms as u64)).await;
        }
        anyhow::Ok(count)
    });

    let pb2 = peer_b.clone();
    let silence_b = silence.clone();
    let send_b = tokio::spawn(async move {
        let mut count = 0u64;
        while tokio::time::Instant::now() < deadline {
            pb2.send_samples(&silence_b).await?;
            count += 1;
            tokio::time::sleep(Duration::from_millis(config.frame_duration_ms as u64)).await;
        }
        anyhow::Ok(count)
    });

    // Drain received frames on both sides.
    let ra = peer_a.clone();
    let drain_a = tokio::spawn(async move {
        let mut count = 0u64;
        while tokio::time::Instant::now() < deadline {
            if (tokio::time::timeout(Duration::from_millis(50), ra.recv_samples()).await).is_ok() {
                count += 1;
            }
        }
        count
    });

    let rb = peer_b.clone();
    let drain_b = tokio::spawn(async move {
        let mut count = 0u64;
        while tokio::time::Instant::now() < deadline {
            if (tokio::time::timeout(Duration::from_millis(50), rb.recv_samples()).await).is_ok() {
                count += 1;
            }
        }
        count
    });

    let (sent_a, sent_b) = (send_a.await??, send_b.await??);
    let (recv_a, recv_b) = (drain_a.await?, drain_b.await?);

    peer_a.close().await?;
    peer_b.close().await?;

    let loss_ab = if sent_a > 0 {
        100.0 * (sent_a.saturating_sub(recv_b)) as f32 / sent_a as f32
    } else {
        0.0
    };
    let loss_ba = if sent_b > 0 {
        100.0 * (sent_b.saturating_sub(recv_a)) as f32 / sent_b as f32
    } else {
        0.0
    };

    println!("\n─── VoIP peer full-duplex ({codec_id:?}) ───");
    println!(" peer A → B : sent {sent_a}, received {recv_b}, loss {loss_ab:.1}%");
    println!(" peer B → A : sent {sent_b}, received {recv_a}, loss {loss_ba:.1}%");
    Ok(())
}
