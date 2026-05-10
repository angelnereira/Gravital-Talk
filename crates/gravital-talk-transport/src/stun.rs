//! Cliente STUN minimalista — RFC 5389 Binding Request/Response.
//!
//! Sin dependencias externas: implementa sólo lo necesario para descubrir
//! la IP pública del socket a través de `stun.l.google.com:19302`.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use tokio::net::UdpSocket;
use tokio::time::timeout;

// ── Constantes del protocolo ───────────────────────────────────────────────────

const MAGIC_COOKIE: u32 = 0x2112_A442;
const BINDING_REQUEST: u16 = 0x0001;
const BINDING_RESPONSE: u16 = 0x0101;
const ATTR_XOR_MAPPED_ADDRESS: u16 = 0x0020;
const ATTR_MAPPED_ADDRESS: u16 = 0x0001;
const FAMILY_IPV4: u8 = 0x01;

const STUN_SERVERS: &[&str] = &[
    "stun.l.google.com:19302",
    "stun1.l.google.com:19302",
    "stun2.l.google.com:19302",
];

/// Error del cliente STUN.
#[derive(Debug, thiserror::Error)]
pub enum StunError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("all STUN servers timed out or failed")]
    Timeout,

    #[error("malformed STUN response")]
    Malformed,

    #[error("STUN resolve error: {0}")]
    Resolve(String),
}

/// Descubre la dirección IP pública del socket local UDP usando STUN.
///
/// Crea un socket UDP efímero (si `local_port == 0`, usa puerto efímero)
/// y envía un Binding Request a los servidores STUN de Google.
///
/// Retorna la `SocketAddr` pública tal como la ve el servidor STUN.
pub async fn discover_public_addr(local_port: u16) -> Result<SocketAddr, StunError> {
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), local_port);
    let sock = UdpSocket::bind(bind_addr).await?;

    for server in STUN_SERVERS {
        // Resolver el nombre DNS del servidor.
        let server_addr = match tokio::net::lookup_host(*server).await {
            Ok(mut addrs) => match addrs.next() {
                Some(a) => a,
                None => continue,
            },
            Err(e) => {
                tracing::debug!("STUN resolve {server}: {e}");
                continue;
            }
        };

        // Construir Binding Request (20 bytes fijos, sin atributos).
        let tx_id = random_tx_id();
        let request = build_binding_request(&tx_id);

        match timeout(
            Duration::from_secs(5),
            do_stun_exchange(&sock, server_addr, &request, &tx_id),
        )
        .await
        {
            Ok(Ok(addr)) => return Ok(addr),
            Ok(Err(e)) => {
                tracing::debug!("STUN {server} error: {e}");
                continue;
            }
            Err(_) => {
                tracing::debug!("STUN {server} timed out");
                continue;
            }
        }
    }

    Err(StunError::Timeout)
}

// ── Internals ──────────────────────────────────────────────────────────────────

async fn do_stun_exchange(
    sock: &UdpSocket,
    server: SocketAddr,
    request: &[u8],
    tx_id: &[u8; 12],
) -> Result<SocketAddr, StunError> {
    sock.send_to(request, server).await?;

    let mut buf = [0u8; 512];
    loop {
        let (n, _from) = sock.recv_from(&mut buf).await?;
        if let Some(addr) = parse_binding_response(&buf[..n], tx_id) {
            return Ok(addr);
        }
        // Paquete inesperado (otro tráfico en el mismo socket) — ignorar.
    }
}

/// Construye un STUN Binding Request de 20 bytes (header sólo, sin atributos).
///
/// ```text
/// 0                   1                   2                   3
/// 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |0 0|  STUN Message Type   |         Message Length              |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                         Magic Cookie                           |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                                                                |
/// |                     Transaction ID (96 bits)                   |
/// |                                                                |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// ```
fn build_binding_request(tx_id: &[u8; 12]) -> [u8; 20] {
    let mut pkt = [0u8; 20];
    pkt[0..2].copy_from_slice(&BINDING_REQUEST.to_be_bytes());
    // Message Length = 0 (sin atributos)
    pkt[2..4].copy_from_slice(&0u16.to_be_bytes());
    pkt[4..8].copy_from_slice(&MAGIC_COOKIE.to_be_bytes());
    pkt[8..20].copy_from_slice(tx_id);
    pkt
}

/// Parsea un Binding Response y extrae la dirección mapeada.
///
/// Soporta `XOR-MAPPED-ADDRESS` (primario) y `MAPPED-ADDRESS` (legacy).
fn parse_binding_response(buf: &[u8], expected_tx_id: &[u8; 12]) -> Option<SocketAddr> {
    if buf.len() < 20 {
        return None;
    }

    let msg_type = u16::from_be_bytes([buf[0], buf[1]]);
    if msg_type != BINDING_RESPONSE {
        return None;
    }

    let msg_len = u16::from_be_bytes([buf[2], buf[3]]) as usize;
    if buf.len() < 20 + msg_len {
        return None;
    }

    let magic = u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]);
    if magic != MAGIC_COOKIE {
        return None;
    }

    if &buf[8..20] != expected_tx_id {
        return None;
    }

    // Iterar sobre atributos.
    let mut pos = 20usize;
    let end = 20 + msg_len;
    let mut fallback: Option<SocketAddr> = None;

    while pos + 4 <= end {
        let attr_type = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let attr_len = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]) as usize;
        pos += 4;

        if pos + attr_len > end {
            break;
        }

        let attr_val = &buf[pos..pos + attr_len];

        match attr_type {
            ATTR_XOR_MAPPED_ADDRESS => {
                if let Some(addr) = parse_xor_mapped_address(attr_val) {
                    return Some(addr); // prioritario
                }
            }
            ATTR_MAPPED_ADDRESS => {
                if let Some(addr) = parse_mapped_address(attr_val) {
                    fallback = Some(addr);
                }
            }
            _ => {}
        }

        // Atributos STUN se alinean a 4 bytes.
        let padded = (attr_len + 3) & !3;
        pos += padded;
    }

    fallback
}

/// Parsea `XOR-MAPPED-ADDRESS` (IPv4).
///
/// ```text
/// 0                   1                   2                   3
/// 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |0 0 0 0 0 0 0 0|    Family     |         X-Port                |
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// |                X-Address (Variable)
/// +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
/// ```
fn parse_xor_mapped_address(val: &[u8]) -> Option<SocketAddr> {
    if val.len() < 8 {
        return None;
    }
    let family = val[1];
    if family != FAMILY_IPV4 {
        return None; // IPv6 no soportado (no necesario para el caso de uso)
    }
    let x_port = u16::from_be_bytes([val[2], val[3]]);
    let port = x_port ^ (MAGIC_COOKIE >> 16) as u16;

    let x_addr = u32::from_be_bytes([val[4], val[5], val[6], val[7]]);
    let addr = x_addr ^ MAGIC_COOKIE;
    let ip = Ipv4Addr::from(addr);
    Some(SocketAddr::new(IpAddr::V4(ip), port))
}

/// Parsea `MAPPED-ADDRESS` legacy (IPv4).
fn parse_mapped_address(val: &[u8]) -> Option<SocketAddr> {
    if val.len() < 8 {
        return None;
    }
    let family = val[1];
    if family != FAMILY_IPV4 {
        return None;
    }
    let port = u16::from_be_bytes([val[2], val[3]]);
    let ip = Ipv4Addr::new(val[4], val[5], val[6], val[7]);
    Some(SocketAddr::new(IpAddr::V4(ip), port))
}

fn random_tx_id() -> [u8; 12] {
    let mut id = [0u8; 12];
    // Usa el mismo generador que el resto del codebase (rand_core::OsRng via x25519-dalek).
    use rand_core::{OsRng, RngCore};
    OsRng.fill_bytes(&mut id);
    id
}

// ── Tests unitarios (sin red) ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_xor_response(tx_id: &[u8; 12], ip: Ipv4Addr, port: u16) -> Vec<u8> {
        // Atributo XOR-MAPPED-ADDRESS para IPv4 (8 bytes de valor)
        let x_port = port ^ (MAGIC_COOKIE >> 16) as u16;
        let x_ip = u32::from(ip) ^ MAGIC_COOKIE;

        let mut attr = Vec::new();
        attr.extend_from_slice(&ATTR_XOR_MAPPED_ADDRESS.to_be_bytes());
        attr.extend_from_slice(&8u16.to_be_bytes()); // length
        attr.push(0x00);                             // reserved
        attr.push(FAMILY_IPV4);
        attr.extend_from_slice(&x_port.to_be_bytes());
        attr.extend_from_slice(&x_ip.to_be_bytes());

        let mut pkt = Vec::new();
        pkt.extend_from_slice(&BINDING_RESPONSE.to_be_bytes());
        pkt.extend_from_slice(&(attr.len() as u16).to_be_bytes());
        pkt.extend_from_slice(&MAGIC_COOKIE.to_be_bytes());
        pkt.extend_from_slice(tx_id);
        pkt.extend_from_slice(&attr);
        pkt
    }

    #[test]
    fn parse_xor_response_roundtrip() {
        let tx_id = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let expected_ip = Ipv4Addr::new(203, 0, 113, 45);
        let expected_port = 54321u16;

        let pkt = make_xor_response(&tx_id, expected_ip, expected_port);
        let addr = parse_binding_response(&pkt, &tx_id).expect("should parse");

        assert_eq!(addr.port(), expected_port);
        assert_eq!(addr.ip(), IpAddr::V4(expected_ip));
    }

    #[test]
    fn wrong_tx_id_rejected() {
        let tx_id = [0u8; 12];
        let wrong_id = [1u8; 12];
        let pkt = make_xor_response(&tx_id, Ipv4Addr::new(1, 2, 3, 4), 1234);
        assert!(parse_binding_response(&pkt, &wrong_id).is_none());
    }

    #[test]
    fn short_packet_rejected() {
        let tx_id = [0u8; 12];
        assert!(parse_binding_response(&[0u8; 5], &tx_id).is_none());
    }
}
