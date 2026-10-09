//! Cerrar y volver a emparejar sobre la misma sesión.
//!
//! Requisito de producto: "cualquiera de los dos cierra la conexión y vuelve a
//! solicitar el emparejamiento". Antes de `Session::reopen`, `close()` fijaba
//! `closed = true` para siempre, así que ese objeto no podía negociar otra vez:
//! la única salida era destruirlo y crear uno nuevo.
//!
//! Estos tests usan un transporte en memoria con direccionamiento por puerto,
//! para que dos sesiones en el mismo proceso se comuniquen como en la red.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use gravital_talk_transport::{Config, Session, SessionRole, Transport, TransportError};
use tokio::sync::Mutex;

/// Datagrama en el bus: payload y puerto lógico del emisor.
type Datagram = (Vec<u8>, u16);

/// Bus con varios suscriptores por puerto.
///
/// Hace falta más de un lector por puerto porque una sesión servidora en modo
/// sala tiene a la vez el lazo que acepta peers y el que reproduce audio.
#[derive(Debug, Clone, Default)]
struct Bus {
    subscribers: Arc<std::sync::Mutex<Vec<Vec<tokio::sync::mpsc::UnboundedSender<Datagram>>>>>,
}

impl Bus {
    fn new(n: usize) -> Self {
        Self {
            subscribers: Arc::new(std::sync::Mutex::new(vec![Vec::new(); n])),
        }
    }

    fn transport(&self, local: u16) -> Arc<dyn Transport> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Datagram>();
        self.subscribers.lock().expect("bus lock")[local as usize].push(tx);
        Arc::new(BusTransport {
            inbox: Arc::new(Mutex::new(rx)),
            bus: self.clone(),
            local,
        })
    }
}

#[derive(Debug)]
struct BusTransport {
    inbox: Arc<Mutex<tokio::sync::mpsc::UnboundedReceiver<Datagram>>>,
    bus: Bus,
    local: u16,
}

#[async_trait]
impl Transport for BusTransport {
    async fn send(&self, bytes: &[u8]) -> Result<usize, TransportError> {
        self.send_to(bytes, SocketAddr::from(([127, 0, 0, 1], 0)))
            .await
    }

    async fn send_to(&self, data: &[u8], peer: SocketAddr) -> Result<usize, TransportError> {
        let subscribers = {
            let guard = self.bus.subscribers.lock().expect("bus lock");
            match guard.get(peer.port() as usize) {
                Some(list) if !list.is_empty() => list.clone(),
                _ => return Err(TransportError::PeerClosed("no subscriber")),
            }
        };
        for tx in subscribers {
            let _ = tx.send((data.to_vec(), self.local));
        }
        Ok(data.len())
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), TransportError> {
        let mut guard = self.inbox.lock().await;
        match guard.recv().await {
            Some((data, from_port)) => {
                let n = data.len().min(buf.len());
                buf[..n].copy_from_slice(&data[..n]);
                Ok((n, SocketAddr::from(([127, 0, 0, 1], from_port))))
            }
            None => Err(TransportError::PeerClosed("bus closed")),
        }
    }

    fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        Ok(SocketAddr::from(([127, 0, 0, 1], self.local)))
    }

    async fn close(&self) -> Result<(), TransportError> {
        Ok(())
    }
}

fn config() -> Config {
    Config {
        frame_duration_ms: 10,
        mtu: 1200,
        ..Default::default()
    }
}

fn host_addr() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], 0))
}

/// Abre una sala: el anfitrión escucha y un invitado se une.
///
/// Devuelve ambas sesiones ya conectadas, listas para transmitir.
async fn pair_open(bus: &Bus) -> (Arc<Session>, Arc<Session>) {
    let host = Arc::new(Session::new(bus.transport(0), config()));
    let join = Arc::new(Session::new(bus.transport(1), config()));

    let host_fut = tokio::spawn({
        let host = Arc::clone(&host);
        async move { host.handshake_open().await }
    });
    tokio::time::sleep(Duration::from_millis(120)).await;
    join.handshake(SessionRole::Client, host_addr())
        .await
        .expect("primer handshake del invitado");
    host_fut
        .await
        .expect("host task")
        .expect("handshake del anfitrión");

    (host, join)
}

/// El requisito central: una sesión cerrada puede volver a emparejarse.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn closed_session_can_pair_again() {
    // Tres puertos: anfitrión y dos invitados sucesivos.
    let bus = Bus::new(3);
    let (host, join) = pair_open(&bus).await;

    // Primera sesión: transmitir algo.
    host.ptt_press().await.ok();
    for _ in 0..3 {
        host.send_audio(&[7u8; 400]).await.ok();
    }
    host.ptt_release().await.ok();

    // Cerrar por ambos lados.
    host.close().await.expect("close del anfitrión");
    join.close().await.ok();

    // REQUISITO: el mismo objeto anfitrión debe poder aceptar otro emparejamiento.
    host.reopen()
        .await
        .expect("reopen debe funcionar tras close()");

    let join2 = Arc::new(Session::new(bus.transport(2), config()));
    let host_fut = tokio::spawn({
        let host = Arc::clone(&host);
        async move { host.handshake_open().await }
    });
    tokio::time::sleep(Duration::from_millis(150)).await;

    join2
        .handshake(SessionRole::Client, host_addr())
        .await
        .expect("segundo emparejamiento debe completar el handshake");
    host_fut
        .await
        .expect("host task")
        .expect("segundo handshake del anfitrión");

    assert_eq!(host.peer_count().await, 1);
    assert_eq!(join2.peer_count().await, 1);

    host.close().await.ok();
    join2.close().await.ok();
}

/// `reopen` sobre una sesión abierta es un error, no una orden.
///
/// Rearrancar en caliente perdería audio sin aviso: quien quiera eso debe
/// cerrar primero, que es una decisión explícita.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reopen_on_open_session_is_rejected() {
    let bus = Bus::new(2);
    let (host, _join) = pair_open(&bus).await;

    let err = host.reopen().await;
    assert!(
        err.is_err(),
        "reopen sobre sesión activa debe fallar, no reiniciar en caliente"
    );

    // La sesión sigue utilizable: el error no la ha dañado.
    host.ptt_press().await.ok();
    host.send_audio(&[9u8; 400]).await.ok();
    host.ptt_release().await.ok();
    host.close().await.ok();
}

/// El estado de la sesión anterior no se hereda.
///
/// Es el fallo que justifica cada `reset()` de `reopen`: la ventana anti-replay
/// anclada a la secuencia vieja descartaría paquetes legítimos, y el bitmap de
/// pérdidas daría un 100 % ficticio.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn reopen_clears_previous_session_state() {
    let bus = Bus::new(2);
    let (host, join) = pair_open(&bus).await;

    // Ensuciar todo el estado que se pueda.
    host.ptt_press().await.ok();
    for _ in 0..5 {
        host.send_audio(&[3u8; 400]).await.ok();
    }
    host.ptt_release().await.ok();

    let antes = host.metrics().snapshot(0.0);
    assert!(
        antes.packets_sent > 0,
        "el test necesita métricas sucias para ser significativo"
    );

    host.close().await.ok();
    join.close().await.ok();
    host.reopen().await.expect("reopen");

    let despues = host.metrics().snapshot(0.0);
    assert_eq!(
        despues.packets_sent, 0,
        "los contadores de la sesión anterior no deben heredarse"
    );
    assert_eq!(
        despues.packets_received, 0,
        "los contadores de la sesión anterior no deben heredarse"
    );
    assert_eq!(
        host.session_id(),
        0,
        "un nuevo emparejamiento empieza sin id"
    );
    assert!(
        !host.is_ptt_active(),
        "un cierre a mitad de transmisión no debe dejar el PTT activo"
    );
}
