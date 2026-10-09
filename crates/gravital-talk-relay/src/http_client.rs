//! Cliente HTTP/1.1 mínimo para llamadas salientes del relay.
//!
//! El relay era hasta ahora **sólo servidor**: tenía `hyper` en modo servidor y
//! ningún cliente. La integración con Band-All necesita hacer POST salientes, y
//! añadir `reqwest` por dos llamadas no compensa (arrastra TLS, redirects y
//! medio ecosistema).
//!
//! Alcance deliberadamente corto: POST con JSON y cabeceras fijas, sobre TCP
//! plano. Sin redirects, sin chunked, sin keep-alive. Si algún día se necesita
//! más, es el momento de reconsiderar `reqwest`.

use std::time::Duration;

use anyhow::{bail, Context, Result};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// Timeout por defecto para las llamadas salientes.
///
/// Corto a propósito: el handshake del protocolo tiene su propio plazo y no
/// conviene sumarle una espera de red larga.
pub const DEFAULT_OUTBOUND_TIMEOUT: Duration = Duration::from_millis(1500);

/// POST de `body` JSON a `url`, devolviendo el cuerpo de la respuesta.
///
/// `url` debe ser `http://host:port/path`. No se soporta HTTPS aquí: el relay
/// habla con Band-All dentro de la misma red (compose/k8s), y el TLS entre
/// servicios es trabajo del mesh o de un sidecar.
pub async fn post_json(
    url: &str,
    body: &[u8],
    timeout: Duration,
    headers: &[(&str, &str)],
) -> Result<Vec<u8>> {
    let (host, port, path) = parse_http_url(url)?;

    let addr = tokio::net::lookup_host((host.as_str(), port))
        .await
        .with_context(|| format!("cannot resolve {host}:{port}"))?
        .next()
        .with_context(|| format!("host {host} resolved to no addresses"))?;

    let mut stream = tokio::time::timeout(timeout, TcpStream::connect(addr))
        .await
        .with_context(|| format!("timeout connecting to {host}:{port}"))??;

    // Petición a mano: sin `Content-Length` el servidor no sabe dónde acaba.
    let mut req = format!(
        "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Length: {}\r\nConnection: close\r\n",
        body.len()
    );
    for (name, value) in headers {
        req.push_str(&format!("{name}: {value}\r\n"));
    }
    req.push_str("\r\n");

    tokio::time::timeout(timeout, stream.write_all(req.as_bytes()))
        .await
        .with_context(|| format!("timeout sending request to {host}"))??;
    tokio::time::timeout(timeout, stream.write_all(body))
        .await
        .with_context(|| format!("timeout sending body to {host}"))??;
    // Sin flush explícito: `write_all` ya vacía el buffer del usuario.

    let mut raw = Vec::new();
    tokio::time::timeout(timeout, stream.read_to_end(&mut raw))
        .await
        .with_context(|| format!("timeout reading response from {host}"))??;

    // Un cuerpo vacío es una respuesta válida (204), no un error de framing.
    let sep = b"\r\n\r\n";
    let (header, payload) = match raw.windows(4).position(|w| w == sep) {
        Some(pos) => (
            String::from_utf8_lossy(&raw[..pos]).to_string(),
            raw[pos + 4..].to_vec(),
        ),
        None => bail!("malformed HTTP response from {host} (no header separator)"),
    };

    let status_line = header.lines().next().unwrap_or_default();
    let status = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse::<u16>().ok())
        .with_context(|| format!("cannot parse status line from {host}: {status_line}"))?;

    // 2xx es éxito. Cualquier otra cosa se trata como error: el cuerpo trae el
    // detalle (Band-All devuelve RFC 7807) y conviene no tragárselo.
    if !(200..300).contains(&status) {
        let detail = String::from_utf8_lossy(&payload);
        bail!("band-all respondio {status}: {}", detail.trim());
    }

    // `Content-Length` se respeta: sin él, `read_to_end` con `Connection:
    // close` ya lee justo hasta que el servidor cierra.
    Ok(payload)
}

/// Descompone `http://host:port/path`.
fn parse_http_url(url: &str) -> Result<(String, u16, String)> {
    let rest = url
        .strip_prefix("http://")
        .with_context(|| format!("url no soportada (se esperaba http://): {url}"))?;

    let (authority, path) = match rest.split_once('/') {
        Some((a, p)) => (a, format!("/{p}")),
        None => (rest, "/".to_string()),
    };

    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => {
            let port: u16 = p
                .parse()
                .with_context(|| format!("puerto no valido en {url}"))?;
            (h.to_string(), port)
        }
        None => (authority.to_string(), 80),
    };

    if host.is_empty() {
        bail!("host vacio en {url}");
    }
    Ok((host, port, path))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_urls_with_and_without_port_and_path() {
        assert_eq!(
            parse_http_url("http://bandall:8080/v1/mfa/verify").unwrap(),
            ("bandall".to_string(), 8080, "/v1/mfa/verify".to_string())
        );
        assert_eq!(
            parse_http_url("http://127.0.0.1:8080/").unwrap(),
            ("127.0.0.1".to_string(), 8080, "/".to_string())
        );
        assert_eq!(
            parse_http_url("http://bandall/v1/x").unwrap(),
            ("bandall".to_string(), 80, "/v1/x".to_string())
        );
    }

    #[test]
    fn rejects_non_http_schemes() {
        // Sin TLS en este cliente: si alguien pone https, es mejor un error
        // claro que un intento de hablar TLS en plano.
        assert!(parse_http_url("https://bandall/v1/x").is_err());
        assert!(parse_http_url("bandall:8080/v1/x").is_err());
    }

    #[test]
    fn rejects_empty_host_and_bad_port() {
        assert!(parse_http_url("http:///v1/x").is_err());
        assert!(parse_http_url("http://bandall:99999/x").is_err());
        assert!(parse_http_url("http://bandall:abc/x").is_err());
    }
}
