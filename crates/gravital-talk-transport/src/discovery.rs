//! Descubrimiento de peers en red local via UDP broadcast.
//!
//! ## Protocolo
//!
//! Formato del datagrama de anuncio: `GT1 <udp_port> <session_id> <name>`
//!
//! - `GT1` — identificador de protocolo (Gravital Talk v1).
//! - `udp_port` — puerto UDP donde el peer escucha datagramas Gravital.
//! - `session_id` — identificador de la sesión que se anuncia.
//! - `name` — nombre legible del peer (puede contener espacios).
//!
//! Ejemplo: `GT1 9000 3735928559 Alice`
//!
//! ## Uso
//!
//! ```no_run
//! use gravital_talk_transport::discovery;
//! use std::time::Duration;
//!
//! // Anunciar esta instancia a la red local.
//! discovery::announce_lan(9000, 0xDEAD_BEEF, "Alice").ok();
//!
//! // Escuchar anuncios durante 2 segundos.
//! let peers = discovery::discover_lan(Duration::from_secs(2)).unwrap();
//! for p in peers {
//!     println!("Peer {} en {} (session_id={})", p.name, p.addr, p.session_id);
//! }
//! ```

use std::net::{SocketAddr, UdpSocket};
use std::time::Duration;

/// Puerto UDP fijo usado para el descubrimiento LAN.
pub const DISCOVERY_PORT: u16 = 9009;

/// Información de un peer descubierto en la red local.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    /// Dirección IP + puerto UDP del peer.
    pub addr: SocketAddr,
    /// session_id anunciado por el peer.
    pub session_id: u32,
    /// Nombre legible del peer.
    pub name: String,
}

/// Envía un anuncio de descubrimiento a la red local (broadcast UDP).
///
/// El datagrama se envía a `255.255.255.255:DISCOVERY_PORT`.
/// En sistemas donde el broadcast directo está filtrado, se puede
/// invocar varias veces apuntando a la dirección de broadcast de la
/// subred concreta.
pub fn announce_lan(udp_port: u16, session_id: u32, name: &str) -> std::io::Result<()> {
    let sock = UdpSocket::bind("0.0.0.0:0")?;
    sock.set_broadcast(true)?;
    let msg = format!("GT1 {udp_port} {session_id} {name}");
    let target: SocketAddr = format!("255.255.255.255:{DISCOVERY_PORT}").parse().unwrap();
    sock.send_to(msg.as_bytes(), target)?;
    Ok(())
}

/// Escucha anuncios de descubrimiento LAN durante `timeout`.
///
/// Retorna todos los peers únicos descubiertos (deduplicados por dirección).
/// El socket se enlaza a `0.0.0.0:DISCOVERY_PORT`, por lo que en sistemas
/// donde ese puerto ya está en uso, la función retornará inmediatamente
/// con `Err`.
pub fn discover_lan(timeout: Duration) -> std::io::Result<Vec<PeerInfo>> {
    let bind: SocketAddr = format!("0.0.0.0:{DISCOVERY_PORT}").parse().unwrap();
    let sock = UdpSocket::bind(bind)?;
    sock.set_read_timeout(Some(timeout.min(Duration::from_secs(30))))?;

    let mut peers: Vec<PeerInfo> = Vec::new();
    let mut buf = [0u8; 512];
    let deadline = std::time::Instant::now() + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        sock.set_read_timeout(Some(remaining)).ok();

        match sock.recv_from(&mut buf) {
            Ok((n, from)) => {
                if let Some(info) = parse_announcement(&buf[..n], from) {
                    if !peers.iter().any(|p| p.addr == info.addr) {
                        peers.push(info);
                    }
                }
            }
            Err(_) => break,
        }
    }

    Ok(peers)
}

fn parse_announcement(data: &[u8], from: SocketAddr) -> Option<PeerInfo> {
    let s = std::str::from_utf8(data).ok()?;
    let s = s.trim();
    if !s.starts_with("GT1 ") {
        return None;
    }
    let rest = &s[4..];
    let mut parts = rest.splitn(3, ' ');
    let port: u16 = parts.next()?.parse().ok()?;
    let session_id: u32 = parts.next()?.parse().ok()?;
    let name = parts.next().unwrap_or("unknown").trim().to_string();
    let addr = SocketAddr::new(from.ip(), port);
    Some(PeerInfo { addr, session_id, name })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr};

    fn from(ip: &str, port: u16) -> SocketAddr {
        SocketAddr::new(ip.parse().unwrap(), port)
    }

    #[test]
    fn parse_valid_announcement() {
        let info = parse_announcement(b"GT1 9000 42 alice", from("192.168.1.10", 1234)).unwrap();
        assert_eq!(info.addr, from("192.168.1.10", 9000));
        assert_eq!(info.session_id, 42);
        assert_eq!(info.name, "alice");
    }

    #[test]
    fn parse_announcement_with_spaces_in_name() {
        let info =
            parse_announcement(b"GT1 9000 99 Bob Room A", from("10.0.0.1", 1234)).unwrap();
        assert_eq!(info.name, "Bob Room A");
        assert_eq!(info.session_id, 99);
    }

    #[test]
    fn parse_missing_protocol_tag() {
        assert!(parse_announcement(b"INVALID 9000 1 bob", from("127.0.0.1", 1)).is_none());
    }

    #[test]
    fn parse_bad_port() {
        assert!(parse_announcement(b"GT1 notaport 1 bob", from("127.0.0.1", 1)).is_none());
    }

    #[test]
    fn parse_bad_session_id() {
        assert!(parse_announcement(b"GT1 9000 notanumber bob", from("127.0.0.1", 1)).is_none());
    }

    #[test]
    fn parse_name_optional() {
        let info = parse_announcement(b"GT1 9000 77", from("127.0.0.1", 1)).unwrap();
        assert_eq!(info.name, "unknown");
        assert_eq!(info.session_id, 77);
    }

    #[test]
    fn discover_lan_no_senders_returns_empty() {
        // Bind al puerto de discovery sería necesario; en CI puede fallar
        // por permisos. Sólo verifica que el error no hace panic.
        let result = discover_lan(Duration::from_millis(50));
        // Puede ser Ok([]) o Err("address already in use"), ambos aceptables.
        match result {
            Ok(peers) => assert!(peers.is_empty()),
            Err(_) => {}
        }
    }
}
