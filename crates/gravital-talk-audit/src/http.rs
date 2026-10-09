//! Cliente HTTP mínimo para resolver códigos de sala contra el plano de
//! control del relay.
//!
//! Duplica a propósito las mismas rutinas que usa `gs ptt` (`crates/
//! gravital-talk-cli/src/main.rs`) en vez de compartirlas: moverlas obligaría
//! a tocar el CLI, y el harness debe poder evolucionar sin coupling con la
//! herramienta de usuario.

use std::net::SocketAddr;

use anyhow::{bail, Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Resuelve `host` a la primera dirección de `port`.
pub async fn lookup_host_any(host: &str, port: u16) -> Result<SocketAddr> {
    tokio::net::lookup_host((host, port))
        .await
        .with_context(|| format!("cannot resolve host {host}"))?
        .next()
        .with_context(|| format!("host {host} resolved to no addresses"))
}

/// GET HTTP/1.1 mínimo y devuelve el cuerpo de la respuesta.
pub async fn http_get(host: &str, port: u16, path: &str) -> Result<String> {
    let addr = lookup_host_any(host, port).await?;
    let mut stream = tokio::net::TcpStream::connect(addr).await?;
    let req = format!("GET {path} HTTP/1.1\r\nHost: {host}:{port}\r\nConnection: close\r\n\r\n");
    stream.write_all(req.as_bytes()).await?;
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await?;
    extract_http_body(&buf)
}

/// Separa el cuerpo de una respuesta HTTP cruda (todo tras la línea en blanco).
fn extract_http_body(raw: &[u8]) -> Result<String> {
    let sep = b"\r\n\r\n";
    match raw.windows(4).position(|w| w == sep) {
        Some(pos) => Ok(String::from_utf8_lossy(&raw[pos + 4..]).trim().to_string()),
        None => bail!("malformed HTTP response (no header separator)"),
    }
}

/// Extrae un campo numérico `"key":123` de un JSON plano, sin dependencias.
pub fn json_u32_field(json: &str, key: &str) -> Option<u32> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = json[start..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse().ok()
}

/// Extrae un campo de texto `"key":"valor"` de un JSON plano.
pub fn json_str_field(json: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\"");
    let start = json.find(&needle)? + needle.len();
    let rest = json[start..].trim_start();
    let rest = rest.strip_prefix(':')?.trim_start();
    let rest = rest.strip_prefix('"')?;
    let end = rest.find('"')?;
    Some(rest[..end].to_string())
}

/// Resuelve un código de sala contra el relay y devuelve su `session_id`.
pub async fn resolve_room(
    relay: &str,
    obs_port: u16,
    room_code: &str,
    token: Option<&str>,
) -> Result<u32> {
    let path = match token {
        Some(t) if !t.is_empty() => format!("/api/rooms/{room_code}?token={t}"),
        _ => format!("/api/rooms/{room_code}"),
    };
    let resp = http_get(relay, obs_port, &path)
        .await
        .with_context(|| format!("cannot resolve room {room_code} at {relay}:{obs_port}"))?;
    json_u32_field(&resp, "session_id")
        .with_context(|| format!("room response without session_id: {resp}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_scalar_fields() {
        let json = r#"{"code":"GRVT-A3F2","session_id":4242,"peers":3}"#;
        assert_eq!(json_u32_field(json, "session_id"), Some(4242));
        assert_eq!(json_u32_field(json, "peers"), Some(3));
        assert_eq!(json_u32_field(json, "missing"), None);
        assert_eq!(json_str_field(json, "code").as_deref(), Some("GRVT-A3F2"));
        assert_eq!(json_str_field(json, "session_id"), None);
    }

    #[test]
    fn parses_fields_with_spaces() {
        let json = r#"{ "session_id" : 7 }"#;
        assert_eq!(json_u32_field(json, "session_id"), Some(7));
    }

    #[test]
    fn extracts_body_after_header_separator() {
        let raw = b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"session_id\":9}";
        assert_eq!(
            extract_http_body(raw).unwrap(),
            r#"{"session_id":9}"#.to_string()
        );
        assert!(extract_http_body(b"no separator here").is_err());
    }
}
