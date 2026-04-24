//! Recibe audio desde un emisor Gravital Sound y lo escribe a un WAV.
//!
//! Uso:
//!   cargo run --release --example receiver -- 0.0.0.0:9000 127.0.0.1:9100 out.wav

use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use gravital_sound::{Config, Session, SessionRole, UdpConfig, UdpTransport};
use hound::{SampleFormat, WavSpec, WavWriter};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bind: SocketAddr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:9000".to_string())
        .parse()?;
    let peer: SocketAddr = env::args()
        .nth(2)
        .unwrap_or_else(|| "127.0.0.1:9100".to_string())
        .parse()?;
    let out_path = PathBuf::from(env::args().nth(3).unwrap_or_else(|| "out.wav".to_string()));

    let transport = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: bind,
            ..Default::default()
        })
        .await?,
    );

    let config = Config::default();
    let session = Session::new(transport, config.clone());
    session.handshake(SessionRole::Server, peer).await?;
    println!("handshake ok, session_id=0x{:08X}", session.session_id());

    let spec = WavSpec {
        channels: config.channels as u16,
        sample_rate: config.sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };
    let mut writer = WavWriter::create(&out_path, spec)?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        match tokio::time::timeout(remaining, session.recv_audio()).await {
            Ok(Ok(frame)) => {
                for chunk in frame.payload.chunks_exact(2) {
                    writer.write_sample(i16::from_le_bytes([chunk[0], chunk[1]]))?;
                }
            }
            Ok(Err(e)) => {
                eprintln!("recv error: {e}");
                break;
            }
            Err(_) => break,
        }
    }

    writer.finalize()?;
    session.close().await?;
    println!("wrote {}", out_path.display());
    Ok(())
}
