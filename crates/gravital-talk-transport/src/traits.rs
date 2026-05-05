//! Trait base para implementaciones de transporte.
//!
//! El trait es minimal y async. El `Transport` lleva sólo bytes — la
//! semántica de paquetes (header, checksum, fragmentación) vive en capas
//! superiores.

use std::net::SocketAddr;

use crate::error::TransportError;

/// Clase de latencia deseada para el transporte. Afecta el tuning de
/// socket (buffers, DSCP, busy-poll) y la estrategia de flush.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LatencyClass {
    /// Máxima latencia baja, pensado para voz en tiempo real (≤ 20 ms E2E).
    #[default]
    RealTime,
    /// Intermedio: streaming con buffering mínimo.
    Interactive,
    /// Lote/bulk: prioriza throughput sobre latencia.
    Bulk,
}

/// Trait async objeto-safe para cualquier transporte que entregue
/// datagramas orientados a mensaje. UDP, QUIC y WebSocket encajan aquí.
#[async_trait::async_trait]
pub trait Transport: Send + Sync + 'static {
    /// Envía un datagrama al peer por default (el que se fijó en connect/bind).
    async fn send(&self, bytes: &[u8]) -> Result<usize, TransportError>;

    /// Envía a una dirección específica (modos server/relay).
    async fn send_to(&self, bytes: &[u8], dest: SocketAddr) -> Result<usize, TransportError>;

    /// Recibe un datagrama. Devuelve `(bytes_read, peer_addr)`.
    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), TransportError>;

    /// Dirección local del transporte (útil para logging).
    fn local_addr(&self) -> Result<SocketAddr, TransportError>;

    /// Cierra el transporte. Llamadas posteriores devuelven
    /// `TransportError::Closed`.
    async fn close(&self) -> Result<(), TransportError>;
}
