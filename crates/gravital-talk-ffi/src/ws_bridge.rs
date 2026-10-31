//! Puente UDP local <-> WebSocket para hablar con un relay publicado sólo por
//! HTTP(S).
//!
//! Hay plataformas (Railway) que publican un puerto HTTP y no dejan entrar
//! UDP. El relay acepta el mismo protocolo por WebSocket en `/ws`, pero la
//! sesión sólo sabe de UDP. En vez de tocar la sesión, el puente abre un
//! socket UDP en `127.0.0.1` y la sesión se conecta a él como si fuera el
//! relay: cada datagrama sale como un mensaje binario del WebSocket, y cada
//! mensaje que llega vuelve como datagrama a quien habló por última vez.
//!
//! El cifrado de extremo a extremo no cambia: los paquetes viajan sellados
//! igual que por UDP; el TLS del WebSocket es una capa más.
//!
//! Si el WebSocket se cae (cambio de red en el móvil, reinicio del relay), el
//! puente reconecta solo con espera creciente, sin cambiar de puerto local.

use std::net::SocketAddr;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::watch;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// Plazo para abrir el WebSocket (TCP + TLS + upgrade).
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// Ping periódico: los proxies HTTP cierran las conexiones calladas.
const PING_EVERY: Duration = Duration::from_secs(15);
/// Espera entre reconexiones: empieza aquí y se duplica hasta el máximo.
const BACKOFF_MIN: Duration = Duration::from_millis(500);
const BACKOFF_MAX: Duration = Duration::from_secs(8);
/// Mayor datagrama que se reenvía (el protocolo no pasa de ~1200 B).
const MAX_DATAGRAM: usize = 2048;

struct Bridge {
    port: u16,
    stop: watch::Sender<bool>,
    thread: Option<std::thread::JoinHandle<()>>,
}

static BRIDGES: Mutex<Vec<Bridge>> = Mutex::new(Vec::new());

fn bridges() -> std::sync::MutexGuard<'static, Vec<Bridge>> {
    match BRIDGES.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    }
}

/// Configuración TLS: raíces de `webpki-roots` y criptografía `ring`. No
/// depende de los certificados del sistema, que en Android no se ven desde
/// código nativo.
fn tls_config() -> Result<Arc<rustls::ClientConfig>, String> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    if let Some(c) = CONFIG.get() {
        return Ok(c.clone());
    }
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = rustls::ClientConfig::builder_with_provider(Arc::new(
        rustls::crypto::ring::default_provider(),
    ))
    .with_safe_default_protocol_versions()
    .map_err(|e| format!("tls: {e}"))?
    .with_root_certificates(roots)
    .with_no_client_auth();
    Ok(CONFIG.get_or_init(|| Arc::new(config)).clone())
}

async fn connect(url: &str) -> Result<WsStream, String> {
    let connector = if url.starts_with("wss://") {
        Some(Connector::Rustls(tls_config()?))
    } else {
        None
    };
    let fut = tokio_tungstenite::connect_async_tls_with_config(url, None, true, connector);
    match tokio::time::timeout(CONNECT_TIMEOUT, fut).await {
        Ok(Ok((ws, _))) => Ok(ws),
        Ok(Err(e)) => Err(format!("websocket: {e}")),
        Err(_) => Err("websocket: tiempo de conexión agotado".into()),
    }
}

/// Mueve paquetes entre el socket local y el WebSocket hasta que se pida
/// parar. Reconecta cuando el WebSocket se cae.
async fn pump(url: String, udp: UdpSocket, first: WsStream, mut stop: watch::Receiver<bool>) {
    let mut next = Some(first);
    // Quien habla por el socket local: la sesión. Es el destino de lo que
    // llega por el WebSocket.
    let mut local_peer: Option<SocketAddr> = None;
    let mut backoff = BACKOFF_MIN;
    let mut buf = [0u8; MAX_DATAGRAM];

    loop {
        let ws = match next.take() {
            Some(ws) => ws,
            None => {
                tokio::select! {
                    _ = stop.changed() => return,
                    () = tokio::time::sleep(backoff) => {}
                }
                match connect(&url).await {
                    Ok(ws) => {
                        tracing::info!("ws bridge: reconectado");
                        backoff = BACKOFF_MIN;
                        ws
                    }
                    Err(e) => {
                        tracing::debug!(%e, "ws bridge: reconexión fallida");
                        backoff = (backoff * 2).min(BACKOFF_MAX);
                        continue;
                    }
                }
            }
        };

        let (mut sink, mut source) = ws.split();
        let mut ping = tokio::time::interval(PING_EVERY);
        ping.tick().await;

        loop {
            tokio::select! {
                _ = stop.changed() => {
                    let _ = sink.close().await;
                    return;
                }
                r = udp.recv_from(&mut buf) => {
                    let Ok((n, from)) = r else { continue };
                    local_peer = Some(from);
                    // tungstenite pide un `Vec` propio por mensaje.
                    if sink.send(Message::Binary(buf[..n].to_vec())).await.is_err() {
                        break;
                    }
                }
                m = source.next() => match m {
                    Some(Ok(Message::Binary(data))) => {
                        if let Some(peer) = local_peer {
                            let _ = udp.send_to(&data, peer).await;
                        }
                    }
                    Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                    Some(Ok(_)) => {}
                },
                _ = ping.tick() => {
                    if sink.send(Message::Ping(Vec::new())).await.is_err() {
                        break;
                    }
                }
            }
        }
        tracing::info!("ws bridge: conexión perdida, reintentando");
    }
}

/// Abre el puente hacia `url` (`ws://` o `wss://`) y devuelve el puerto UDP
/// local al que debe conectarse la sesión (en `127.0.0.1`).
///
/// La primera conexión se hace aquí, para que un relay inalcanzable dé error
/// en el acto en vez de un handshake que no termina.
pub fn start(url: &str) -> Result<u16, String> {
    if !(url.starts_with("ws://") || url.starts_with("wss://")) {
        return Err("la URL del relay debe empezar por ws:// o wss://".into());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("runtime: {e}"))?;
    let (udp, ws) = runtime.block_on(async {
        let udp = UdpSocket::bind((std::net::Ipv4Addr::LOCALHOST, 0))
            .await
            .map_err(|e| format!("udp: {e}"))?;
        let ws = connect(url).await?;
        Ok::<_, String>((udp, ws))
    })?;
    let port = udp.local_addr().map_err(|e| format!("udp: {e}"))?.port();

    let (stop_tx, stop_rx) = watch::channel(false);
    let url = url.to_owned();
    let thread = std::thread::Builder::new()
        .name("gs-ws-bridge".into())
        .spawn(move || runtime.block_on(pump(url, udp, ws, stop_rx)))
        .map_err(|e| format!("thread: {e}"))?;

    bridges().push(Bridge {
        port,
        stop: stop_tx,
        thread: Some(thread),
    });
    Ok(port)
}

/// Cierra el puente del puerto `port`. Devuelve `false` si no existía.
pub fn stop(port: u16) -> bool {
    let bridge = {
        let mut list = bridges();
        let Some(i) = list.iter().position(|b| b.port == port) else {
            return false;
        };
        list.swap_remove(i)
    };
    let mut bridge = bridge;
    let _ = bridge.stop.send(true);
    if let Some(t) = bridge.thread.take() {
        let _ = t.join();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Relay de mentira: devuelve por el WebSocket cada mensaje binario.
    async fn echo_server() -> SocketAddr {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((tcp, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
                    while let Some(Ok(m)) = ws.next().await {
                        if m.is_binary() && ws.send(m).await.is_err() {
                            break;
                        }
                    }
                });
            }
        });
        addr
    }

    #[test]
    fn un_datagrama_ida_y_vuelta_por_el_websocket() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let server = rt.block_on(echo_server());

        let port = start(&format!("ws://{server}/ws")).expect("puente");
        let reply = rt.block_on(async {
            let sock = UdpSocket::bind("127.0.0.1:0").await.unwrap();
            sock.send_to(b"hola relay", ("127.0.0.1", port))
                .await
                .unwrap();
            let mut buf = [0u8; 64];
            let (n, _) = tokio::time::timeout(Duration::from_secs(5), sock.recv_from(&mut buf))
                .await
                .expect("respuesta a tiempo")
                .unwrap();
            buf[..n].to_vec()
        });
        assert_eq!(reply, b"hola relay");
        assert!(stop(port));
        assert!(!stop(port), "un puente cerrado no se cierra dos veces");
    }

    #[test]
    fn rechaza_urls_que_no_son_websocket() {
        assert!(start("https://example.com").is_err());
        assert!(start("example.com:9000").is_err());
    }

    #[test]
    fn falla_en_el_acto_si_el_relay_no_responde() {
        // Puerto cerrado: la conexión se rechaza sin esperar al plazo.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        drop(listener);
        assert!(start(&format!("ws://{addr}/ws")).is_err());
    }
}
