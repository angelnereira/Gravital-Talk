//! Transporte UDP con tuning agresivo de socket.
//!
//! El `UdpTransport` configura el socket antes de envolverlo en
//! `tokio::net::UdpSocket`:
//!
//! - `SO_REUSEADDR` y `SO_REUSEPORT` (Unix) para permitir múltiples
//!   receptores en la misma tupla.
//! - `SO_SNDBUF` / `SO_RCVBUF` a 4 MB para absorber ráfagas.
//! - `IP_TOS` a DSCP EF (0xB8) en `LatencyClass::RealTime` para que la red
//!   priorice tráfico de audio.
//!
//! El hot path (`send_to` / `recv_from`) no asigna memoria.

use std::net::SocketAddr;

use async_trait::async_trait;
use socket2::{Domain, Protocol, Socket, Type};
use tokio::net::UdpSocket;

use crate::error::TransportError;
use crate::traits::{LatencyClass, Transport};

/// Tamaño de buffer de socket por default (4 MB).
pub const DEFAULT_SOCKET_BUFFER: usize = 4 * 1024 * 1024;

/// DSCP Expedited Forwarding para tráfico real-time (46 << 2 = 184 = 0xB8).
pub const DSCP_EF: u32 = 0xB8;

/// Configuración del socket UDP.
#[derive(Debug, Clone)]
pub struct UdpConfig {
    /// Dirección a la que bindear (`0.0.0.0:0` para ephemeral).
    pub bind_addr: SocketAddr,
    /// Peer por default para `send` sin destino explícito.
    pub peer: Option<SocketAddr>,
    /// Tamaño de `SO_SNDBUF` / `SO_RCVBUF` en bytes.
    pub socket_buffer_bytes: usize,
    /// Clase de latencia deseada.
    pub latency_class: LatencyClass,
    /// Si habilitar `SO_REUSEPORT` (Unix).
    pub reuse_port: bool,
    /// TTL IP (None = default del sistema).
    pub ttl: Option<u32>,
}

impl Default for UdpConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:0".parse().expect("valid default bind addr"),
            peer: None,
            socket_buffer_bytes: DEFAULT_SOCKET_BUFFER,
            latency_class: LatencyClass::RealTime,
            reuse_port: true,
            ttl: None,
        }
    }
}

/// Transporte UDP sobre `tokio::net::UdpSocket`.
#[derive(Debug)]
pub struct UdpTransport {
    socket: UdpSocket,
    default_peer: Option<SocketAddr>,
}

impl UdpTransport {
    /// Crea un nuevo `UdpTransport` bindeando y tuneando el socket.
    /// Es async porque `UdpSocket::connect` a un peer requiere el runtime.
    pub async fn bind(config: UdpConfig) -> Result<Self, TransportError> {
        let domain = match config.bind_addr {
            SocketAddr::V4(_) => Domain::IPV4,
            SocketAddr::V6(_) => Domain::IPV6,
        };
        let sock = Socket::new(domain, Type::DGRAM, Some(Protocol::UDP))?;

        sock.set_nonblocking(true)?;
        sock.set_reuse_address(true)?;

        #[cfg(unix)]
        if config.reuse_port {
            sock.set_reuse_port(true)?;
        }

        sock.set_send_buffer_size(config.socket_buffer_bytes)?;
        sock.set_recv_buffer_size(config.socket_buffer_bytes)?;

        if let Some(ttl) = config.ttl {
            sock.set_ttl(ttl)?;
        }

        // DSCP EF para tráfico real-time (prioridad alta en routers conformes).
        if config.latency_class == LatencyClass::RealTime {
            #[cfg(unix)]
            {
                // Algunos kernels o namespaces rechazan el set; no es fatal.
                let _ = sock.set_tos(DSCP_EF);
            }
        }

        sock.bind(&config.bind_addr.into())?;

        let std_sock: std::net::UdpSocket = sock.into();
        let socket = UdpSocket::from_std(std_sock)?;

        if let Some(peer) = config.peer {
            socket.connect(peer).await?;
        }

        Ok(Self {
            socket,
            default_peer: config.peer,
        })
    }

    /// Referencia al socket subyacente (útil para integración con
    /// `tokio::select!`).
    #[must_use]
    pub fn socket(&self) -> &UdpSocket {
        &self.socket
    }
}

#[async_trait]
impl Transport for UdpTransport {
    async fn send(&self, bytes: &[u8]) -> Result<usize, TransportError> {
        match self.default_peer {
            Some(peer) => Ok(self.socket.send_to(bytes, peer).await?),
            None => Ok(self.socket.send(bytes).await?),
        }
    }

    async fn send_to(&self, bytes: &[u8], dest: SocketAddr) -> Result<usize, TransportError> {
        Ok(self.socket.send_to(bytes, dest).await?)
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), TransportError> {
        Ok(self.socket.recv_from(buf).await?)
    }

    fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        Ok(self.socket.local_addr()?)
    }

    async fn close(&self) -> Result<(), TransportError> {
        // UDP no tiene close explícito; el drop libera el fd. Este método
        // existe para simetría con transportes orientados a conexión.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn bind_and_local_addr() {
        let t = UdpTransport::bind(UdpConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        })
        .await
        .unwrap();
        let addr = t.local_addr().unwrap();
        assert!(addr.port() > 0);
        assert_eq!(addr.ip().to_string(), "127.0.0.1");
    }

    #[tokio::test]
    async fn roundtrip_localhost() {
        let server = UdpTransport::bind(UdpConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        })
        .await
        .unwrap();
        let server_addr = server.local_addr().unwrap();

        let client = UdpTransport::bind(UdpConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            peer: Some(server_addr),
            ..Default::default()
        })
        .await
        .unwrap();

        let payload = b"gravital";
        client.send(payload).await.unwrap();

        let mut buf = [0u8; 64];
        let (n, from) = server.recv(&mut buf).await.unwrap();
        assert_eq!(&buf[..n], payload);
        assert_eq!(from, client.local_addr().unwrap());
    }
}
