//! Envía una onda sinusoidal de 440 Hz a un receptor Gravital Sound.
//!
//! Uso:
//!   cargo run --release --example sender -- <peer_addr> [local_bind]
//! Por ejemplo:
//!   cargo run --release --example sender -- 127.0.0.1:9000 127.0.0.1:9100

use std::env;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gravital_sound::{Config, Session, SessionRole, UdpConfig, UdpTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let peer: SocketAddr = env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:9000".to_string())
        .parse()?;
    let bind: SocketAddr = env::args()
        .nth(2)
        .unwrap_or_else(|| "127.0.0.1:9100".to_string())
        .parse()?;

    let transport = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: bind,
            ..Default::default()
        })
        .await?,
    );

    // 10 ms frames para que 480 samples PCM16 mono = 960 B quepan en la MTU
    // default (1200) después del header de 24 B y trailer de 4 B.
    let config = Config {
        frame_duration_ms: 10,
        ..Config::default()
    };
    let session = Session::new(transport, config.clone());
    session.handshake(SessionRole::Client, peer).await?;
    println!("handshake ok, session_id=0x{:08X}", session.session_id());

    let sr = config.sample_rate as f32;
    let samples_per_frame = (config.sample_rate * config.frame_duration_ms as u32 / 1000) as usize;
    let step = 2.0 * std::f32::consts::PI * 440.0 / sr;
    let mut phase = 0.0f32;

    let start = Instant::now();
    let frame_period = Duration::from_millis(config.frame_duration_ms as u64);
    let mut next = start;
    for i in 0..500u32 {
        let mut buf = Vec::with_capacity(samples_per_frame * 2);
        for _ in 0..samples_per_frame {
            let s = (phase.sin() * 16_000.0) as i16;
            buf.extend_from_slice(&s.to_le_bytes());
            phase += step;
            if phase > std::f32::consts::TAU {
                phase -= std::f32::consts::TAU;
            }
        }
        session.send_audio(&buf).await?;
        next += frame_period;
        let now = Instant::now();
        if next > now {
            tokio::time::sleep(next - now).await;
        }
        if i % 50 == 0 {
            println!("sent {i} frames");
        }
    }

    session.close().await?;
    println!("done");
    Ok(())
}
