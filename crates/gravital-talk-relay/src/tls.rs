//! Carga de certificados TLS (feature `tls`).
//!
//! Sin la feature, `TlsAcceptor` es un tipo vacío (`Infallible`) para que la
//! firma de `ws::run`/`grpc::serve` compile en ambos perfiles.

use std::path::Path;

#[cfg(feature = "tls")]
pub use tokio_rustls::TlsAcceptor;

/// Tipo del acceptor TLS (placeholder si la feature está apagada).
#[cfg(not(feature = "tls"))]
pub type TlsAcceptor = core::convert::Infallible;

/// Carga pares PEM (cert + key) y construye un `TlsAcceptor` compartido.
#[cfg(feature = "tls")]
pub fn load_acceptor(cert: &Path, key: &Path) -> anyhow::Result<std::sync::Arc<TlsAcceptor>> {
    use rustls_pemfile::{certs, pkcs8_private_keys, rsa_private_keys};

    // Un solo proveedor criptográfico (ring) para el proceso.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let cert_pem = std::fs::read(cert)?;
    let key_pem = std::fs::read(key)?;

    let mut cert_reader = cert_pem.as_slice();
    let parsed = certs(&mut cert_reader).collect::<std::io::Result<Vec<_>>>()?;
    let cert_chain: Vec<_> = parsed
        .into_iter()
        .map(rustls::pki_types::CertificateDer::into_owned)
        .collect();
    if cert_chain.is_empty() {
        anyhow::bail!("no certificates found in {cert:?}");
    }

    // Acepta PKCS#8 y PKCS#1 (RSA).
    let mut key_reader = key_pem.as_slice();
    let pkcs8 = pkcs8_private_keys(&mut key_reader).collect::<std::io::Result<Vec<_>>>()?;
    let private_key = if let Some(k) = pkcs8.into_iter().next() {
        rustls::pki_types::PrivateKeyDer::Pkcs8(k)
    } else {
        let mut key_reader2 = key_pem.as_slice();
        let rsa = rsa_private_keys(&mut key_reader2).collect::<std::io::Result<Vec<_>>>()?;
        match rsa.into_iter().next() {
            Some(k) => rustls::pki_types::PrivateKeyDer::Pkcs1(k),
            None => anyhow::bail!("no private key found in {key:?}"),
        }
    };

    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(cert_chain, private_key)?;
    Ok(std::sync::Arc::new(TlsAcceptor::from(std::sync::Arc::new(
        config,
    ))))
}

#[cfg(not(feature = "tls"))]
pub fn load_acceptor(_cert: &Path, _key: &Path) -> anyhow::Result<std::sync::Arc<TlsAcceptor>> {
    anyhow::bail!("relay compiled without `tls` feature")
}

/// Stream final tras aplicar TLS opcional (boxeado para reducir tamaño).
#[cfg(feature = "tls")]
pub enum MaybeTls {
    Plain(tokio::net::TcpStream),
    Tls(Box<tokio_rustls::server::TlsStream<tokio::net::TcpStream>>),
}

/// Acepta TLS si hay acceptor; si no, devuelve el stream plano.
#[cfg(feature = "tls")]
pub async fn try_accept(
    tls: Option<&std::sync::Arc<TlsAcceptor>>,
    stream: tokio::net::TcpStream,
) -> anyhow::Result<MaybeTls> {
    match tls {
        Some(acceptor) => Ok(MaybeTls::Tls(Box::new(acceptor.accept(stream).await?))),
        None => Ok(MaybeTls::Plain(stream)),
    }
}

/// Stream final tras aplicar TLS opcional (sin feature solo hay plano).
#[cfg(not(feature = "tls"))]
pub enum MaybeTls {
    Plain(tokio::net::TcpStream),
}

/// Sin la feature `tls`, siempre stream plano.
#[cfg(not(feature = "tls"))]
pub async fn try_accept(
    _tls: Option<&std::sync::Arc<TlsAcceptor>>,
    stream: tokio::net::TcpStream,
) -> anyhow::Result<MaybeTls> {
    Ok(MaybeTls::Plain(stream))
}
