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

/// Tope para resolver un nombre STUN por DNS.
const DNS_TIMEOUT: Duration = Duration::from_millis(1200);

/// Tope para una consulta STUN en un socket efímero.
const EXCHANGE_TIMEOUT: Duration = Duration::from_millis(1500);

/// Tope por intento STUN cuando se usa el socket de la sesión.
const SESSION_EXCHANGE_TIMEOUT: Duration = Duration::from_millis(1000);

/// Tope total para `discover_public_addr_on`.
///
/// Nunca debe acercarse a los 6 s que espera la app antes de empezar el
/// handshake: si este futuro siguiera leyendo del socket de la sesión cuando
/// arranca `accept_any` o `connect_any`, competiría por los paquetes entrantes.
const SESSION_TOTAL_TIMEOUT: Duration = Duration::from_millis(2800);

/// Destinos representativos de subredes privadas y de punto de acceso móvil
/// (Android `192.168.43.0/24` y `192.168.49.0/24`, NetworkManager `10.42.0.0/24`,
/// iOS `172.20.10.0/28`, LAN domésticas habituales y link-local).
const FALLBACK_ROUTE_TARGETS: &[Ipv4Addr] = &[
    Ipv4Addr::new(8, 8, 8, 8),
    Ipv4Addr::new(192, 168, 43, 1),
    Ipv4Addr::new(192, 168, 49, 1),
    Ipv4Addr::new(192, 168, 1, 1),
    Ipv4Addr::new(192, 168, 0, 1),
    Ipv4Addr::new(10, 42, 0, 1),
    Ipv4Addr::new(172, 20, 10, 1),
    Ipv4Addr::new(10, 0, 0, 1),
    Ipv4Addr::new(172, 16, 0, 1),
    Ipv4Addr::new(169, 254, 1, 1),
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

    #[error("transport error: {0}")]
    Transport(String),
}

/// Descubre la dirección **local** (LAN) de este dispositivo.
///
/// No envía ningún paquete a la red: crea un socket UDP efímero, pide al
/// kernel que seleccione la ruta hacia un destino y lee `local_addr()`.
///
/// Primero prueba `8.8.8.8:53` (ruta por defecto). Si el dispositivo está en
/// una red aislada sin salida a Internet ni puerta de enlace por defecto —por
/// ejemplo, un teléfono haciendo de punto Wi-Fi sin datos móviles o un router
/// de emergencia sin WAN—, `connect(8.8.8.8)` falla con `ENETUNREACH`. En ese
/// caso sonda las subredes privadas habituales en la tabla de rutas del kernel
/// hasta dar con la interfaz activa.
pub async fn discover_local_addr() -> Result<SocketAddr, StunError> {
    let sock = UdpSocket::bind(SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0)).await?;
    if let Some(addr) = probe_local_addr_targets(&sock, FALLBACK_ROUTE_TARGETS).await {
        return Ok(addr);
    }
    // Android 11+ elige al azar una subred /24 dentro de 192.168.0.0/16 al
    // abrir un punto de acceso Wi-Fi.256 consultas a la tabla de rutas del
    // kernel cuestan menos de 0,5 ms y no emiten tráfico.
    for third in 2..=254u8 {
        let candidate = Ipv4Addr::new(192, 168, third, 1);
        if let Some(addr) = probe_single_target(&sock, candidate).await {
            return Ok(addr);
        }
    }
    for second in 17..=31u8 {
        let candidate = Ipv4Addr::new(172, second, 0, 1);
        if let Some(addr) = probe_single_target(&sock, candidate).await {
            return Ok(addr);
        }
    }
    Ok(sock.local_addr()?)
}

async fn probe_local_addr_targets(sock: &UdpSocket, targets: &[Ipv4Addr]) -> Option<SocketAddr> {
    for &target in targets {
        if let Some(addr) = probe_single_target(sock, target).await {
            return Some(addr);
        }
    }
    None
}

async fn probe_single_target(sock: &UdpSocket, target: Ipv4Addr) -> Option<SocketAddr> {
    if sock
        .connect(SocketAddr::new(IpAddr::V4(target), 53))
        .await
        .is_ok()
    {
        if let Ok(addr) = sock.local_addr() {
            if !addr.ip().is_unspecified() && !addr.ip().is_loopback() {
                return Some(addr);
            }
        }
    }
    None
}

/// Descubre la dirección IP pública del socket local UDP usando STUN.
///
/// Crea un socket UDP efímero (si `local_port == 0`, usa puerto efímero)
/// y envía un Binding Request a los servidores STUN de Google.
///
/// **`local_port` debe ser 0.** Pasarla el puerto de una sesión ya viva hace
/// que el bind falle con "address already in use", porque el socket de la
/// sesión lo ocupa. Es un error que ya ocurrió: el QR se quedaba con un host
/// de relleno y nadie podía conectar. Ver el test de regresión.
///
/// Retorna la `SocketAddr` pública tal como la ve el servidor STUN.
pub async fn discover_public_addr(local_port: u16) -> Result<SocketAddr, StunError> {
    let bind_addr = SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), local_port);
    let sock = UdpSocket::bind(bind_addr).await?;

    for server in STUN_SERVERS {
        // Resolver el nombre DNS del servidor con tope de tiempo: sin Internet,
        // el resolvedor del sistema puede tardar decenas de segundos.
        let Ok(lookup) = timeout(DNS_TIMEOUT, tokio::net::lookup_host(*server)).await else {
            tracing::debug!("STUN resolve {server}: timeout");
            break;
        };
        let server_addr = match lookup {
            Ok(mut addrs) => match addrs.find(SocketAddr::is_ipv4) {
                Some(a) => a,
                None => continue,
            },
            Err(e) => {
                tracing::debug!("STUN resolve {server}: {e}");
                break;
            }
        };

        // Construir Binding Request (20 bytes fijos, sin atributos).
        let tx_id = random_tx_id();
        let request = build_binding_request(&tx_id);

        match timeout(
            EXCHANGE_TIMEOUT,
            do_stun_exchange(&sock, server_addr, &request, &tx_id),
        )
        .await
        {
            Ok(Ok(addr)) => return Ok(addr),
            Ok(Err(e)) => {
                tracing::debug!("STUN {server} error: {e}");
                break;
            }
            Err(_) => {
                tracing::debug!("STUN {server} timed out");
                continue;
            }
        }
    }

    Err(StunError::Timeout)
}

/// Descubre la dirección pública **de un transporte ya abierto**.
///
/// Es la variante que sirve para publicar una dirección: un NAT asocia el
/// puerto público a un socket concreto, así que preguntar desde otro socket
/// (lo que hace [`discover_public_addr`]) da la IP correcta y un puerto que no
/// es el de la sala.
///
/// Sólo se puede llamar mientras nadie más lee del transporte, es decir antes
/// del handshake: las respuestas STUN llegan por el mismo socket que el resto
/// del tráfico. Lo que llegue entretanto y no sea la respuesta se descarta.
pub async fn discover_public_addr_on(
    transport: &dyn crate::traits::Transport,
) -> Result<SocketAddr, StunError> {
    timeout(
        SESSION_TOTAL_TIMEOUT,
        discover_public_addr_on_inner(transport),
    )
    .await
    .unwrap_or(Err(StunError::Timeout))
}

async fn discover_public_addr_on_inner(
    transport: &dyn crate::traits::Transport,
) -> Result<SocketAddr, StunError> {
    let transport_err = |e: crate::error::TransportError| StunError::Transport(e.to_string());
    let mut buf = [0u8; 512];

    for server in STUN_SERVERS {
        // Sólo IPv4: el socket de la sesión se abre en IPv4, y a un servidor
        // IPv6 no llegaría. Si el DNS expira o falla en el primero, no tiene
        // sentido repetir contra el mismo resolvedor para `stun1` y `stun2`.
        let Ok(Ok(mut addrs)) = timeout(DNS_TIMEOUT, tokio::net::lookup_host(*server)).await else {
            break;
        };
        let Some(server_addr) = addrs.find(SocketAddr::is_ipv4) else {
            continue;
        };

        let tx_id = random_tx_id();
        let request = build_binding_request(&tx_id);
        // Si no hay ruta hacia fuera (`ENETUNREACH`), abortamos en el acto sin
        // agotar más servidores.
        transport
            .send_to(&request, server_addr)
            .await
            .map_err(transport_err)?;

        let exchange = async {
            loop {
                let (n, _from) = transport.recv(&mut buf).await.map_err(transport_err)?;
                if let Some(addr) = parse_binding_response(&buf[..n], &tx_id) {
                    return Ok::<_, StunError>(addr);
                }
            }
        };
        match timeout(SESSION_EXCHANGE_TIMEOUT, exchange).await {
            Ok(Ok(addr)) => return Ok(addr),
            Ok(Err(e)) => return Err(e),
            Err(_) => tracing::debug!("STUN {server} sin respuesta"),
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
const fn parse_mapped_address(val: &[u8]) -> Option<SocketAddr> {
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
        attr.push(0x00); // reserved
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

    #[tokio::test]
    async fn descubrir_con_el_puerto_de_una_sesion_viva_falla_por_eaddrinuse() {
        // Regresion: la app pasaba `session.local_port()` a `discover_public_addr`
        // y eso crea un segundo socket en el MISMO puerto que ya ocupa la sesion.
        // El bind falla con "address already in use" y el QR se queda con el
        // host de relleno `red-local:0`, con el que nadie puede conectar.
        //
        // El socket de STUN tiene que ser efimero (puerto 0), no el de la sesion.
        use tokio::net::UdpSocket;

        let held = UdpSocket::bind("0.0.0.0:0").await.expect("bind previo");
        let port = held.local_addr().expect("addr").port();

        // Pasarle el puerto ocupado falla.
        let r = tokio::time::timeout(Duration::from_secs(10), discover_public_addr(port))
            .await
            .expect("no deberia colgarse");
        assert!(
            r.is_err(),
            "bindear un puerto ya ocupado deberia fallar, devolvio {:?}",
            r.ok()
        );

        // Y con puerto 0 (efimero) si funciona: es la forma correcta.
        let r = tokio::time::timeout(Duration::from_secs(15), discover_public_addr(0)).await;
        drop(held);
        match r {
            Ok(Ok(addr)) => assert!(addr.port() > 0),
            // Sin red no se puede probar el exito, solo que no reviente.
            _ => eprintln!("sin acceso a STUN: se omite la comprobacion de exito"),
        }
    }

    #[tokio::test]
    async fn sondar_rutas_locales_salta_loopback_y_encuentra_interfaz_activa() {
        let sock = UdpSocket::bind("0.0.0.0:0").await.expect("bind");
        // 127.0.0.1 enlaza a loopback (que no sirve para el QR de la sala) y se
        // descarta; los siguientes destinos encuentran la interfaz de red real.
        let targets = [
            Ipv4Addr::new(127, 0, 0, 1),
            Ipv4Addr::new(192, 168, 1, 1),
            Ipv4Addr::new(8, 8, 8, 8),
        ];
        if let Some(addr) = probe_local_addr_targets(&sock, &targets).await {
            assert!(!addr.ip().is_unspecified());
            assert!(!addr.ip().is_loopback());
        }
    }

    #[tokio::test]
    async fn stun_en_transporte_sin_salida_termina_antes_del_tope_de_la_app() {
        struct BlackholeTransport;

        #[async_trait::async_trait]
        impl crate::traits::Transport for BlackholeTransport {
            async fn send(&self, buf: &[u8]) -> Result<usize, crate::error::TransportError> {
                Ok(buf.len())
            }

            async fn send_to(
                &self,
                buf: &[u8],
                _addr: SocketAddr,
            ) -> Result<usize, crate::error::TransportError> {
                Ok(buf.len())
            }

            async fn recv(
                &self,
                _buf: &mut [u8],
            ) -> Result<(usize, SocketAddr), crate::error::TransportError> {
                std::future::pending().await
            }

            async fn close(&self) -> Result<(), crate::error::TransportError> {
                Ok(())
            }

            fn local_addr(&self) -> Result<SocketAddr, crate::error::TransportError> {
                Ok(SocketAddr::new(
                    IpAddr::V4(Ipv4Addr::new(192, 168, 1, 10)),
                    9000,
                ))
            }
        }

        let start = std::time::Instant::now();
        let res = discover_public_addr_on(&BlackholeTransport).await;
        let elapsed = start.elapsed();

        assert!(res.is_err());
        // Debe soltar el socket mucho antes del timeout de 6 s de la app.
        assert!(
            elapsed < Duration::from_millis(3500),
            "discover_public_addr_on tardo demasiado sin Internet: {elapsed:?}"
        );
    }
}
