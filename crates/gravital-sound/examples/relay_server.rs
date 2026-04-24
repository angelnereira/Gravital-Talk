//! Relay UDP básico que reenvía paquetes entre pares con el mismo
//! `session_id`. Uso:
//!
//!   cargo run --release --example relay_server -- 0.0.0.0:9100

use std::collections::HashMap;
use std::env;
use std::net::SocketAddr;

use gravital_sound::{PacketView, Transport, UdpConfig, UdpTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bind: SocketAddr = env::args()
        .nth(1)
        .unwrap_or_else(|| "0.0.0.0:9100".to_string())
        .parse()?;
    let t = UdpTransport::bind(UdpConfig {
        bind_addr: bind,
        ..Default::default()
    })
    .await?;
    println!("relay listening on {:?}", t.local_addr()?);

    let mut routes: HashMap<u32, SocketAddr> = HashMap::new();
    let mut buf = vec![0u8; 1500];
    loop {
        let (n, from) = t.recv(&mut buf).await?;
        let slice = &buf[..n];
        match PacketView::decode(slice) {
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
                eprintln!("drop bad packet from {from}: {e}");
            }
        }
    }
}
