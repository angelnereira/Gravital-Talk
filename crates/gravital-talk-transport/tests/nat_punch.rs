//! Perforación de NAT sobre una red simulada.
//!
//! La red de estos tests reproduce lo que hace un router doméstico: cada
//! extremo sale por una dirección pública, y sólo le llega lo que viene de un
//! destino al que él ha enviado antes. Con eso se puede comprobar, sin
//! depender de ninguna red real, las tres cosas que importan:
//!
//! - que sin servidor de encuentro dos extremos tras NAT **no** conectan (el
//!   control: si esto pasara, el simulador no estaría filtrando nada);
//! - que con él sí, y que después circula audio cifrado;
//! - qué pasa cuando un lado tiene NAT simétrico.

#![cfg(feature = "noise")]

use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use gravital_talk_transport::rendezvous::Registry;
use gravital_talk_transport::{Config, Session, Transport, TransportError};
use tokio::sync::mpsc;
use tokio::time::timeout;

type Datagram = (Vec<u8>, SocketAddr);

/// Cómo asigna el NAT la dirección pública de un extremo.
#[derive(Clone, Copy, PartialEq)]
enum Mapping {
    /// Sin NAT: la dirección pública es la propia (un servidor).
    Open,
    /// El mismo puerto público para cualquier destino (lo habitual en casa).
    Cone,
    /// Un puerto público distinto por destino (CGNAT estricto).
    Symmetric,
}

/// Qué deja entrar el NAT.
#[derive(Clone, Copy, PartialEq)]
enum Filter {
    /// Sólo de la IP **y puerto** a los que se envió antes.
    PortRestricted,
    /// De cualquier puerto de una IP a la que se envió antes.
    AddressRestricted,
}

struct Node {
    inbox: mpsc::UnboundedSender<Datagram>,
    mapping: Mapping,
    filter: Filter,
    /// Dirección pública base (con `Symmetric`, sólo la IP es estable).
    public: SocketAddr,
    /// Destinos a los que este extremo ha enviado: lo que su NAT deja volver.
    sent_to: HashSet<SocketAddr>,
    /// Puertos públicos asignados por destino (sólo `Symmetric`).
    ports: HashMap<SocketAddr, u16>,
}

/// "Internet": entrega datagramas aplicando el NAT de cada extremo.
#[derive(Default)]
struct Net {
    nodes: Mutex<Vec<Node>>,
    /// Todo lo entregado, para poder reinyectarlo (ataques de repetición).
    tap: Mutex<Vec<(Vec<u8>, SocketAddr)>>,
    /// Mientras esté activo, la red se traga todo lo que se envía.
    blackout: std::sync::atomic::AtomicBool,
}

impl Net {
    fn join(self: &Arc<Self>, public: &str, mapping: Mapping, filter: Filter) -> Arc<NatTransport> {
        let (tx, rx) = mpsc::unbounded_channel();
        let mut nodes = self.nodes.lock().unwrap();
        nodes.push(Node {
            inbox: tx,
            mapping,
            filter,
            public: public.parse().unwrap(),
            sent_to: HashSet::new(),
            ports: HashMap::new(),
        });
        Arc::new(NatTransport {
            net: self.clone(),
            id: nodes.len() - 1,
            inbox: tokio::sync::Mutex::new(rx),
        })
    }

    /// Cambia de red a un extremo: dirección pública nueva y un NAT que aún
    /// no ha visto salir nada.
    fn rehome(&self, id: usize, public: &str) {
        let mut nodes = self.nodes.lock().unwrap();
        nodes[id].public = public.parse().unwrap();
        nodes[id].sent_to.clear();
        nodes[id].ports.clear();
    }

    fn deliver(&self, from: usize, bytes: &[u8], dest: SocketAddr) {
        if self.blackout.load(std::sync::atomic::Ordering::Acquire) {
            return;
        }
        let mut nodes = self.nodes.lock().unwrap();

        // Dirección con la que sale este datagrama.
        let sender = &mut nodes[from];
        sender.sent_to.insert(dest);
        let source = match sender.mapping {
            Mapping::Open | Mapping::Cone => sender.public,
            Mapping::Symmetric => {
                let next = sender.public.port() + 1 + sender.ports.len() as u16;
                let port = *sender.ports.entry(dest).or_insert(next);
                SocketAddr::new(sender.public.ip(), port)
            }
        };

        // A quién pertenece la dirección de destino.
        let target = nodes.iter().position(|n| match n.mapping {
            Mapping::Open | Mapping::Cone => n.public == dest,
            Mapping::Symmetric => {
                n.public.ip() == dest.ip() && n.ports.values().any(|p| *p == dest.port())
            }
        });
        let Some(target) = target else { return };
        let node = &nodes[target];

        let allowed = match (node.mapping, node.filter) {
            (Mapping::Open, _) => true,
            // Un NAT simétrico sólo acepta en cada puerto al destino para el
            // que lo abrió.
            (Mapping::Symmetric, _) => node.ports.get(&source) == Some(&dest.port()),
            (Mapping::Cone, Filter::PortRestricted) => node.sent_to.contains(&source),
            (Mapping::Cone, Filter::AddressRestricted) => {
                node.sent_to.iter().any(|a| a.ip() == source.ip())
            }
        };
        if allowed {
            let _ = node.inbox.send((bytes.to_vec(), source));
            self.tap.lock().unwrap().push((bytes.to_vec(), dest));
        }
        drop(nodes);
    }
}

struct NatTransport {
    net: Arc<Net>,
    id: usize,
    inbox: tokio::sync::Mutex<mpsc::UnboundedReceiver<Datagram>>,
}

impl NatTransport {
    fn public(&self) -> SocketAddr {
        self.net.nodes.lock().unwrap()[self.id].public
    }

    fn rehome(&self, public: &str) {
        self.net.rehome(self.id, public);
    }
}

#[async_trait]
impl Transport for NatTransport {
    async fn send(&self, _bytes: &[u8]) -> Result<usize, TransportError> {
        Err(TransportError::InvalidState("sin peer por defecto"))
    }

    async fn send_to(&self, bytes: &[u8], dest: SocketAddr) -> Result<usize, TransportError> {
        self.net.deliver(self.id, bytes, dest);
        Ok(bytes.len())
    }

    async fn recv(&self, buf: &mut [u8]) -> Result<(usize, SocketAddr), TransportError> {
        let (data, from) = self
            .inbox
            .lock()
            .await
            .recv()
            .await
            .ok_or(TransportError::Closed)?;
        buf[..data.len()].copy_from_slice(&data);
        Ok((data.len(), from))
    }

    fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        Ok(self.public())
    }

    async fn close(&self) -> Result<(), TransportError> {
        Ok(())
    }
}

const SERVER: &str = "198.51.100.1:9000";
const TOKEN: &str = "secreto-de-la-sala";

/// Arranca un servidor de encuentro en la red simulada.
fn spawn_rendezvous(net: &Arc<Net>) -> SocketAddr {
    let transport = net.join(SERVER, Mapping::Open, Filter::PortRestricted);
    let registry = Registry::new(64);
    tokio::spawn(async move {
        let mut buf = [0u8; 1500];
        while let Ok((n, from)) = transport.recv(&mut buf).await {
            if let Some(reply) = registry.handle(&buf[..n], from) {
                let _ = transport.send_to(&reply, from).await;
            }
        }
    });
    SERVER.parse().unwrap()
}

fn session(transport: Arc<NatTransport>, rendezvous: Option<SocketAddr>) -> Arc<Session> {
    let s = Session::new(
        transport,
        Config {
            frame_duration_ms: 10,
            ..Config::default()
        },
    );
    s.set_room_token(Some(TOKEN.into()));
    s.set_rendezvous(rendezvous);
    Arc::new(s)
}

/// Anfitrión esperando + invitado conectando. Devuelve el resultado del
/// invitado y, si conectó, las dos sesiones ya activas.
async fn pair(
    host_t: Arc<NatTransport>,
    guest_t: Arc<NatTransport>,
    rendezvous: Option<SocketAddr>,
    candidates: Vec<SocketAddr>,
) -> (Result<(), TransportError>, Arc<Session>, Arc<Session>) {
    let host = session(host_t, rendezvous);
    let guest = session(guest_t, rendezvous);
    let accepting = {
        let host = host.clone();
        tokio::spawn(async move { host.handshake_open().await })
    };
    let joined = timeout(Duration::from_secs(20), guest.handshake_any(&candidates))
        .await
        .expect("el invitado no debe colgarse");
    if joined.is_ok() {
        timeout(Duration::from_secs(5), accepting)
            .await
            .expect("el anfitrión debe terminar")
            .unwrap()
            .expect("handshake del anfitrión");
    } else {
        host.close().await.ok();
    }
    (joined, host, guest)
}

/// Un frame de ida y otro de vuelta por el camino perforado.
async fn assert_audio_flows(host: &Session, guest: &Session) {
    let frame = vec![0x5Au8; 960];
    guest.send_audio(&frame).await.expect("invitado envía");
    let got = timeout(Duration::from_secs(3), host.recv_audio())
        .await
        .expect("el anfitrión debe recibir")
        .expect("recv");
    assert_eq!(&got.payload[..], &frame[..]);

    host.send_audio(&frame).await.expect("anfitrión envía");
    let got = timeout(Duration::from_secs(3), guest.recv_audio())
        .await
        .expect("el invitado debe recibir")
        .expect("recv");
    assert_eq!(&got.payload[..], &frame[..]);
}

#[tokio::test(start_paused = true)]
async fn sin_encuentro_dos_extremos_tras_nat_no_conectan() {
    // El control. El invitado conoce la dirección pública del anfitrión (la
    // traía la invitación), pero el NAT del anfitrión no ha enviado nada
    // hacia él y descarta todo lo que llega.
    let net = Arc::new(Net::default());
    let host = net.join("203.0.113.1:40000", Mapping::Cone, Filter::PortRestricted);
    let guest = net.join("203.0.113.2:50000", Mapping::Cone, Filter::PortRestricted);
    let host_public = host.public();

    let (joined, _, _) = pair(host, guest, None, vec![host_public]).await;
    assert!(joined.is_err(), "no debería conectar: {joined:?}");
}

#[tokio::test(start_paused = true)]
async fn con_encuentro_se_perfora_y_circula_audio() {
    let net = Arc::new(Net::default());
    let server = spawn_rendezvous(&net);
    let host = net.join("203.0.113.1:40000", Mapping::Cone, Filter::PortRestricted);
    let guest = net.join("203.0.113.2:50000", Mapping::Cone, Filter::PortRestricted);
    let host_public = host.public();

    let (joined, host, guest) = pair(host, guest, Some(server), vec![host_public]).await;
    joined.expect("debe conectar perforando");
    assert_audio_flows(&host, &guest).await;
}

#[tokio::test(start_paused = true)]
async fn el_invitado_aprende_la_direccion_del_servidor_de_encuentro() {
    // La invitación sólo trae la dirección de red local del anfitrión, que
    // desde fuera no lleva a ningún sitio. La pública llega por el servidor.
    let net = Arc::new(Net::default());
    let server = spawn_rendezvous(&net);
    let host = net.join("203.0.113.1:40000", Mapping::Cone, Filter::PortRestricted);
    let guest = net.join("203.0.113.2:50000", Mapping::Cone, Filter::PortRestricted);
    let lan_only = "192.168.0.13:40000".parse().unwrap();

    let (joined, host, guest) = pair(host, guest, Some(server), vec![lan_only]).await;
    joined.expect("debe conectar por la dirección aprendida");
    assert_audio_flows(&host, &guest).await;
}

#[tokio::test(start_paused = true)]
async fn un_lado_simetrico_conecta_si_el_otro_no_filtra_por_puerto() {
    // El anfitrión sale por un puerto distinto hacia cada destino: el que ve
    // el servidor no sirve. Pero su sonda llega al invitado (que acepta
    // cualquier puerto de una IP conocida) y de ella saca la dirección buena.
    let net = Arc::new(Net::default());
    let server = spawn_rendezvous(&net);
    let host = net.join(
        "203.0.113.1:40000",
        Mapping::Symmetric,
        Filter::PortRestricted,
    );
    let guest = net.join(
        "203.0.113.2:50000",
        Mapping::Cone,
        Filter::AddressRestricted,
    );

    let (joined, host, guest) = pair(host, guest, Some(server), vec![]).await;
    joined.expect("debe conectar aprendiendo la dirección de la sonda");
    assert_audio_flows(&host, &guest).await;
}

/// Servidor de encuentro que además reenvía, como `gs-relay`: lo que llega
/// con un `session_id` se manda a los demás extremos vistos con ese mismo id.
fn spawn_relay(net: &Arc<Net>) -> SocketAddr {
    use gravital_talk_core::packet::PacketView;
    let transport = net.join(SERVER, Mapping::Open, Filter::PortRestricted);
    let registry = Registry::new(64);
    tokio::spawn(async move {
        let mut routes: HashMap<u32, Vec<SocketAddr>> = HashMap::new();
        let mut buf = [0u8; 1500];
        while let Ok((n, from)) = transport.recv(&mut buf).await {
            if gravital_talk_transport::rendezvous::is_rendezvous(&buf[..n]) {
                if let Some(reply) = registry.handle(&buf[..n], from) {
                    let _ = transport.send_to(&reply, from).await;
                }
                continue;
            }
            let Ok(view) = PacketView::decode(&buf[..n]) else {
                continue;
            };
            let peers = routes.entry(view.header().session_id).or_default();
            if !peers.contains(&from) {
                peers.push(from);
            }
            for peer in peers.iter().filter(|p| **p != from) {
                let _ = transport.send_to(&buf[..n], *peer).await;
            }
        }
    });
    SERVER.parse().unwrap()
}

#[tokio::test(start_paused = true)]
async fn sin_servidor_un_movil_y_una_red_fija_conectan_intercambiando_direcciones() {
    // El caso medido con redes reales: un extremo tras NAT simétrico (datos
    // móviles) y el otro tras un NAT de cono que filtra por puerto (router de
    // casa). No hay servidor: la invitación lleva la dirección del anfitrión y
    // la respuesta lleva la del invitado. Ninguna de las dos es exacta para
    // el lado simétrico, pero su puerto real cae cerca, y el barrido de
    // vecinos lo encuentra.
    let net = Arc::new(Net::default());
    let host_t = net.join(
        "203.0.113.1:40000",
        Mapping::Symmetric,
        Filter::PortRestricted,
    );
    let guest_t = net.join("203.0.113.2:50000", Mapping::Cone, Filter::PortRestricted);
    // Lo que STUN diría de cada uno.
    let host_seen = host_t.public();
    let guest_seen = guest_t.public();

    let host = session(host_t, None);
    let guest = session(guest_t, None);
    // El anfitrión escanea la respuesta del invitado.
    host.add_path_hints(&[guest_seen]);
    let accepting = {
        let host = host.clone();
        tokio::spawn(async move { host.handshake_open().await })
    };
    timeout(
        Duration::from_secs(30),
        guest.handshake_any_within(&[host_seen], Duration::from_secs(20)),
    )
    .await
    .expect("el invitado no debe colgarse")
    .expect("deben conectar directo");
    timeout(Duration::from_secs(5), accepting)
        .await
        .expect("el anfitrión debe terminar")
        .unwrap()
        .expect("handshake del anfitrión");
    assert_audio_flows(&host, &guest).await;
}

#[tokio::test(start_paused = true)]
async fn dos_lados_simetricos_no_conectan_si_el_servidor_solo_presenta() {
    // El límite de la perforación: ningún extremo puede predecir el puerto
    // del otro. Con un servidor que sólo presenta direcciones, no hay camino.
    let net = Arc::new(Net::default());
    let server = spawn_rendezvous(&net);
    let host = net.join(
        "203.0.113.1:40000",
        Mapping::Symmetric,
        Filter::PortRestricted,
    );
    let guest = net.join(
        "203.0.113.2:50000",
        Mapping::Symmetric,
        Filter::PortRestricted,
    );

    let (joined, _, _) = pair(host, guest, Some(server), vec![]).await;
    assert!(joined.is_err(), "no debería conectar: {joined:?}");
}

#[tokio::test(start_paused = true)]
async fn dos_lados_simetricos_hablan_por_el_relay() {
    // Dos móviles en datos. Como nada directo contesta, el audio pasa por el
    // relay sin que nadie lo pida — y sigue cifrado de extremo a extremo: el
    // relay reenvía bytes que no sabe leer.
    let net = Arc::new(Net::default());
    let relay = spawn_relay(&net);
    let host = net.join(
        "203.0.113.1:40000",
        Mapping::Symmetric,
        Filter::PortRestricted,
    );
    let guest = net.join(
        "203.0.113.2:50000",
        Mapping::Symmetric,
        Filter::PortRestricted,
    );

    let (joined, host, guest) = pair(host, guest, Some(relay), vec![]).await;
    joined.expect("debe conectar por el relay");
    assert!(guest.is_relayed().await && host.is_relayed().await);
    assert_audio_flows(&host, &guest).await;
}

#[tokio::test(start_paused = true)]
async fn habiendo_camino_directo_no_se_usa_el_relay() {
    // El relay es el último recurso: con dos NAT de cono se perfora, y el
    // audio no pasa por él aunque esté disponible.
    let net = Arc::new(Net::default());
    let relay = spawn_relay(&net);
    let host = net.join("203.0.113.1:40000", Mapping::Cone, Filter::PortRestricted);
    let guest = net.join("203.0.113.2:50000", Mapping::Cone, Filter::PortRestricted);
    let host_public = host.public();

    let (joined, host, guest) = pair(host, guest, Some(relay), vec![]).await;
    joined.expect("debe conectar directo");
    assert!(!guest.is_relayed().await && !host.is_relayed().await);
    assert_eq!(guest.peers().await, vec![host_public]);
    assert_audio_flows(&host, &guest).await;
}

#[tokio::test(start_paused = true)]
async fn entre_un_camino_local_y_uno_publico_se_elige_el_local() {
    // Sin NAT de por medio los dos caminos contestan; gana el de red local
    // aunque el público vaya primero en la lista.
    let net = Arc::new(Net::default());
    let host = net.join("192.168.0.13:40000", Mapping::Open, Filter::PortRestricted);
    let guest = net.join("192.168.0.20:50000", Mapping::Open, Filter::PortRestricted);
    let lan = host.public();
    let dead_public: SocketAddr = "203.0.113.9:1".parse().unwrap();

    let host_s = session(host, None);
    let guest_s = session(guest, None);
    let accepting = {
        let host_s = host_s.clone();
        tokio::spawn(async move { host_s.handshake_open().await })
    };
    let chosen = guest_s
        .select_path(&[dead_public, lan], Duration::from_secs(3))
        .await
        .expect("el camino local contesta");
    assert_eq!(chosen, lan);
    host_s.close().await.ok();
    let _ = accepting.await;
}

// ── Cambio de red con la sesión en marcha ─────────────────────────────────

/// Lazo de recepción de un extremo: es lo que mantiene vivo el camino
/// (latidos, sondas) y lo que entrega el audio. Devuelve por dónde llegan los
/// frames.
fn spawn_receiver(session: &Arc<Session>) -> mpsc::UnboundedReceiver<Vec<u8>> {
    let (tx, rx) = mpsc::unbounded_channel();
    let session = session.clone();
    tokio::spawn(async move {
        loop {
            match timeout(Duration::from_millis(100), session.recv_audio()).await {
                Ok(Ok(frame)) => {
                    if tx.send(frame.payload.to_vec()).is_err() {
                        break;
                    }
                }
                Ok(Err(_)) => break,
                Err(_) => {}
            }
        }
    });
    rx
}

/// Envía frames hasta que uno llega al otro lado, o se agota el plazo.
async fn audio_gets_through(
    from: &Session,
    inbox: &mut mpsc::UnboundedReceiver<Vec<u8>>,
    marker: u8,
    within: Duration,
) -> bool {
    let frame = vec![marker; 960];
    let attempt = async {
        loop {
            let _ = from.send_audio(&frame).await;
            match timeout(Duration::from_millis(200), inbox.recv()).await {
                Ok(Some(got)) if got == frame => return,
                _ => {}
            }
        }
    };
    timeout(within, attempt).await.is_ok()
}

#[tokio::test(start_paused = true)]
async fn el_invitado_cambia_de_red_y_la_sesion_sigue() {
    // Anfitrión alcanzable (sin NAT); el invitado pasa de una red a otra,
    // como un móvil que sale de la wifi a datos. Nadie reconecta: el primer
    // latido desde la dirección nueva basta para que el anfitrión la aprenda.
    let net = Arc::new(Net::default());
    let host_t = net.join("203.0.113.1:40000", Mapping::Open, Filter::PortRestricted);
    let guest_t = net.join("203.0.113.2:50000", Mapping::Cone, Filter::PortRestricted);
    let host_addr = host_t.public();

    let (joined, host, guest) = pair(host_t, guest_t.clone(), None, vec![host_addr]).await;
    joined.expect("conexión inicial");
    let mut host_rx = spawn_receiver(&host);
    let mut guest_rx = spawn_receiver(&guest);
    assert!(audio_gets_through(&host, &mut guest_rx, 1, Duration::from_secs(2)).await);

    guest_t.rehome("198.51.100.77:61000");
    let moved: SocketAddr = "198.51.100.77:61000".parse().unwrap();

    // Sin que el invitado hable: sólo con sus latidos.
    tokio::time::sleep(Duration::from_secs(3)).await;
    assert_eq!(
        host.peers().await,
        vec![moved],
        "el anfitrión debe seguirlo"
    );
    assert!(audio_gets_through(&host, &mut guest_rx, 2, Duration::from_secs(2)).await);
    assert!(audio_gets_through(&guest, &mut host_rx, 3, Duration::from_secs(2)).await);
}

#[tokio::test(start_paused = true)]
async fn los_dos_tras_nat_recuperan_el_camino_por_el_servidor_de_encuentro() {
    // Aquí el latido desde la dirección nueva NO entra: el NAT del anfitrión
    // sólo espera a la antigua. El invitado se vuelve a registrar, el
    // anfitrión se entera por el servidor y sonda la nueva: eso la abre.
    let net = Arc::new(Net::default());
    let server = spawn_rendezvous(&net);
    let host_t = net.join("203.0.113.1:40000", Mapping::Cone, Filter::PortRestricted);
    let guest_t = net.join("203.0.113.2:50000", Mapping::Cone, Filter::PortRestricted);

    let (joined, host, guest) = pair(host_t, guest_t.clone(), Some(server), vec![]).await;
    joined.expect("conexión inicial");
    let mut host_rx = spawn_receiver(&host);
    let mut guest_rx = spawn_receiver(&guest);
    assert!(audio_gets_through(&guest, &mut host_rx, 1, Duration::from_secs(2)).await);

    guest_t.rehome("198.51.100.77:61000");

    assert!(
        audio_gets_through(&guest, &mut host_rx, 2, Duration::from_secs(15)).await,
        "el audio del invitado debe volver a llegar"
    );
    assert!(audio_gets_through(&host, &mut guest_rx, 3, Duration::from_secs(5)).await);
}

#[tokio::test(start_paused = true)]
async fn el_anfitrion_cambia_de_direccion_y_el_invitado_lo_encuentra() {
    // La invitación traía dos direcciones del anfitrión. La que se usaba deja
    // de responder; el invitado sonda las demás y pasa a la que contesta.
    let net = Arc::new(Net::default());
    let host_t = net.join("192.168.0.13:40000", Mapping::Open, Filter::PortRestricted);
    let guest_t = net.join("192.168.0.20:50000", Mapping::Open, Filter::PortRestricted);
    let first = host_t.public();
    let second: SocketAddr = "203.0.113.50:40000".parse().unwrap();

    let (joined, host, guest) = pair(host_t.clone(), guest_t, None, vec![first, second]).await;
    joined.expect("conexión inicial");
    let mut host_rx = spawn_receiver(&host);
    let mut guest_rx = spawn_receiver(&guest);
    assert!(audio_gets_through(&guest, &mut host_rx, 1, Duration::from_secs(2)).await);

    host_t.rehome("203.0.113.50:40000");

    assert!(
        audio_gets_through(&guest, &mut host_rx, 2, Duration::from_secs(15)).await,
        "el invitado debe encontrar la otra dirección"
    );
    assert_eq!(guest.peers().await, vec![second]);
    assert!(audio_gets_through(&host, &mut guest_rx, 3, Duration::from_secs(5)).await);
}

#[tokio::test(start_paused = true)]
async fn un_paquete_repetido_desde_otra_direccion_no_secuestra_la_sesion() {
    // Un atacante en el camino captura tráfico legítimo y lo reenvía desde su
    // propia dirección. El paquete es auténtico —lleva un AEAD válido— pero
    // su secuencia ya se vio: no puede servir para mover la sesión.
    let net = Arc::new(Net::default());
    let host_t = net.join("203.0.113.1:40000", Mapping::Open, Filter::PortRestricted);
    let guest_t = net.join("203.0.113.2:50000", Mapping::Open, Filter::PortRestricted);
    let attacker = net.join("198.51.100.66:6666", Mapping::Open, Filter::PortRestricted);
    let host_addr = host_t.public();
    let guest_addr = guest_t.public();

    let (joined, host, guest) = pair(host_t, guest_t, None, vec![host_addr]).await;
    joined.expect("conexión inicial");
    let mut host_rx = spawn_receiver(&host);
    let _guest_rx = spawn_receiver(&guest);
    assert!(audio_gets_through(&guest, &mut host_rx, 1, Duration::from_secs(2)).await);
    tokio::time::sleep(Duration::from_secs(2)).await;

    // Todo lo que el invitado mandó al anfitrión, repetido por el atacante.
    let captured: Vec<Vec<u8>> = net
        .tap
        .lock()
        .unwrap()
        .iter()
        .filter(|(_, dest)| *dest == host_addr)
        .map(|(bytes, _)| bytes.clone())
        .collect();
    assert!(captured.len() > 3, "debe haber tráfico que repetir");
    for bytes in &captured {
        attacker.send_to(bytes, host_addr).await.unwrap();
    }
    tokio::time::sleep(Duration::from_secs(1)).await;

    assert_eq!(
        host.peers().await,
        vec![guest_addr],
        "la sesión no debe moverse"
    );
    assert!(audio_gets_through(&guest, &mut host_rx, 2, Duration::from_secs(2)).await);
}

#[tokio::test(start_paused = true)]
async fn un_anfitrion_en_sesion_sigue_contestando_sondas() {
    // Quien pregunta por el camino con la clave de la sala recibe respuesta
    // aunque la sala ya esté hablando: es lo que usa un extremo que ha
    // cambiado de red para saber por dónde volver.
    use gravital_talk_transport::rendezvous::{Message, RoomKey};

    let net = Arc::new(Net::default());
    let host_t = net.join("203.0.113.1:40000", Mapping::Open, Filter::PortRestricted);
    let guest_t = net.join("203.0.113.2:50000", Mapping::Open, Filter::PortRestricted);
    let other = net.join("198.51.100.5:7000", Mapping::Open, Filter::PortRestricted);
    let host_addr = host_t.public();

    let (joined, host, _guest) = pair(host_t, guest_t, None, vec![host_addr]).await;
    joined.expect("conexión inicial");
    let _host_rx = spawn_receiver(&host);

    let key = RoomKey::from_token(TOKEN.as_bytes());
    let nonce = [3u8; 8];
    other
        .send_to(&Message::Punch { nonce }.encode(key), host_addr)
        .await
        .unwrap();
    let mut buf = [0u8; 256];
    let (n, from) = timeout(Duration::from_secs(2), other.recv(&mut buf))
        .await
        .expect("debe contestar")
        .unwrap();
    assert_eq!(from, host_addr);
    assert_eq!(
        Message::decode(&buf[..n]),
        Some((key, Message::PunchAck { nonce }))
    );
}

#[tokio::test(start_paused = true)]
async fn un_frame_perdido_no_para_el_audio_aunque_se_lea_con_plazo_corto() {
    // Regresión. La app lee con un timeout de 20 ms; el plazo para dar un
    // frame por perdido (180 ms) era local a cada llamada y nunca vencía.
    let net = Arc::new(Net::default());
    let host_t = net.join("203.0.113.1:40000", Mapping::Open, Filter::PortRestricted);
    let guest_t = net.join("203.0.113.2:50000", Mapping::Open, Filter::PortRestricted);
    let host_addr = host_t.public();
    let (joined, host, guest) = pair(host_t, guest_t, None, vec![host_addr]).await;
    joined.expect("conexión inicial");

    // Lectura como la de la app: plazo mucho más corto que el de pérdida.
    let (tx, mut rx) = mpsc::unbounded_channel();
    {
        let host = host.clone();
        tokio::spawn(async move {
            loop {
                if let Ok(Ok(frame)) = timeout(Duration::from_millis(20), host.recv_audio()).await {
                    let _ = tx.send(frame.payload[0]);
                }
            }
        });
    }

    guest.send_audio(&[1u8; 960]).await.unwrap();
    assert_eq!(
        timeout(Duration::from_secs(1), rx.recv()).await.unwrap(),
        Some(1)
    );

    // Varios frames seguidos que se pierden por el camino: más de los que la
    // paridad FEC puede reconstruir, para que de verdad falten.
    net.blackout
        .store(true, std::sync::atomic::Ordering::Release);
    for lost in 2u8..=5 {
        guest.send_audio(&[lost; 960]).await.unwrap();
    }
    net.blackout
        .store(false, std::sync::atomic::Ordering::Release);

    // Los siguientes tienen que sonar igualmente, y en orden.
    guest.send_audio(&[6u8; 960]).await.unwrap();
    guest.send_audio(&[7u8; 960]).await.unwrap();
    let mut heard = Vec::new();
    while heard.last() != Some(&7) {
        let frame = timeout(Duration::from_secs(2), rx.recv())
            .await
            .expect("el audio no debe pararse")
            .unwrap();
        heard.push(frame);
    }
    assert!(heard.ends_with(&[6, 7]), "oído: {heard:?}");
}
