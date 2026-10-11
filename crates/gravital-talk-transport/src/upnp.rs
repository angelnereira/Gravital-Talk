//! Apertura de puerto en el router por UPnP IGD.
//!
//! ## Para qué
//!
//! El anfitrión de una sala escucha en un puerto, pero detrás de un router
//! nadie de fuera llega hasta él. Perforar el NAT necesita que alguien
//! presente a los dos extremos. Esto otro no necesita a nadie: el anfitrión le
//! pide a **su propio router** que reenvíe un puerto público hacia él, y desde
//! ese momento es alcanzable desde cualquier red — incluido un invitado en
//! datos móviles, que sólo tiene que poder salir.
//!
//! Es lo que hacen las consolas y los clientes de videollamada. Casi todos los
//! routers domésticos lo traen activado.
//!
//! ## Qué no cubre
//!
//! - Un anfitrión en datos móviles: no hay router propio al que pedírselo.
//! - Routers con UPnP desactivado (algunos de empresa, algunos de operador).
//! - Doble NAT: si el router está a su vez detrás del NAT del operador, abre
//!   el puerto en una dirección que tampoco es pública. Se detecta —la IP
//!   externa que declara el router es privada— y se trata como fallo, porque
//!   publicar esa dirección en una invitación sería prometer algo falso.
//!
//! ## Implementación
//!
//! El protocolo son tres pasos, y aquí van a mano para no arrastrar un cliente
//! HTTP y un parser XML completos por un intercambio de tres mensajes:
//!
//! 1. **SSDP**: un datagrama multicast pregunta "¿hay algún router?" y el
//!    router contesta con la URL de su descripción.
//! 2. **Descripción**: un GET a esa URL devuelve un XML del que sólo interesa
//!    la URL de control del servicio `WANIPConnection` (o `WANPPPConnection`).
//! 3. **SOAP**: un POST a esa URL con `AddPortMapping`.
//!
//! El XML no se parsea: se buscan las etiquetas que hacen falta. Los routers
//! generan estos documentos con plantillas fijas, y un parser estricto falla
//! con más de uno de ellos.

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};
use tokio::time::timeout;

/// Dirección multicast de SSDP.
pub const SSDP_ADDR: SocketAddr =
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(239, 255, 255, 250)), 1900);

/// Cuánto se espera a que un router conteste al descubrimiento.
const DISCOVERY_TIMEOUT: Duration = Duration::from_millis(1500);

/// Tope de cada petición HTTP al router.
const HTTP_TIMEOUT: Duration = Duration::from_secs(3);

/// Tamaño máximo de una respuesta del router que se acepta leer.
const MAX_RESPONSE: usize = 256 * 1024;

/// Servicios que saben abrir puertos, por orden de preferencia.
const SERVICES: &[&str] = &[
    "urn:schemas-upnp-org:service:WANIPConnection:2",
    "urn:schemas-upnp-org:service:WANIPConnection:1",
    "urn:schemas-upnp-org:service:WANPPPConnection:1",
];

/// Puertos externos alternativos que se prueban si el pedido está cogido.
const PORT_ATTEMPTS: u16 = 4;

/// Por qué no se pudo abrir el puerto.
#[derive(Debug, thiserror::Error)]
pub enum UpnpError {
    /// Ningún router contestó: no hay UPnP en esta red, o está desactivado.
    #[error("ningún router contestó por UPnP")]
    NoGateway,

    /// El router contestó pero no ofrece un servicio para abrir puertos.
    #[error("el router no ofrece apertura de puertos")]
    NoService,

    /// El router rechazó la petición.
    #[error("el router rechazó la petición (código {code}: {description})")]
    Rejected { code: u32, description: String },

    /// La IP externa del router no es pública (doble NAT, CGNAT).
    #[error("el router está a su vez detrás de otro NAT (IP externa {0})")]
    NotPublic(IpAddr),

    /// La respuesta del router no se entiende.
    #[error("respuesta del router no válida: {0}")]
    Malformed(&'static str),

    #[error("I/O: {0}")]
    Io(#[from] std::io::Error),

    #[error("el router no contestó a tiempo")]
    Timeout,
}

/// Un router con apertura de puertos, ya localizado.
#[derive(Debug, Clone)]
pub struct Gateway {
    /// Dirección del servidor HTTP del router.
    pub addr: SocketAddr,
    /// Ruta de control del servicio.
    pub control_path: String,
    /// Tipo exacto del servicio (va en la cabecera `SOAPAction`).
    pub service: String,
}

/// Un puerto abierto en el router.
#[derive(Debug, Clone)]
pub struct PortMapping {
    /// Dirección pública por la que ahora se llega a este equipo.
    pub external: SocketAddr,
    /// Puerto local al que el router reenvía.
    pub internal_port: u16,
    gateway: Gateway,
}

impl PortMapping {
    /// Cierra el puerto en el router.
    ///
    /// Si falla no pasa nada grave —el router lo cierra solo cuando vence la
    /// concesión—, pero dejarlo abierto más de lo necesario es mala educación.
    pub async fn remove(&self) -> Result<(), UpnpError> {
        let body = format!(
            "<NewRemoteHost></NewRemoteHost><NewExternalPort>{}</NewExternalPort>\
             <NewProtocol>UDP</NewProtocol>",
            self.external.port()
        );
        soap(&self.gateway, "DeletePortMapping", &body)
            .await
            .map(drop)
    }
}

/// Abre en el router un puerto UDP hacia `internal_port` de este equipo.
///
/// `lease` es cuánto dura la apertura si nadie la cierra antes; es la red de
/// seguridad por si la app muere sin poder avisar al router.
pub async fn map_udp_port(internal_port: u16, lease: Duration) -> Result<PortMapping, UpnpError> {
    let gateway = discover(SSDP_ADDR).await?;
    map_udp_port_on(gateway, internal_port, lease).await
}

/// Como [`map_udp_port`], sobre un router ya localizado.
pub async fn map_udp_port_on(
    gateway: Gateway,
    internal_port: u16,
    lease: Duration,
) -> Result<PortMapping, UpnpError> {
    // La IP con la que este equipo habla con el router es la que el router
    // tiene que usar como destino del reenvío.
    let internal_ip = local_ip_towards(gateway.addr).await?;

    let external_ip = external_ip(&gateway).await?;
    if !is_public(external_ip) {
        return Err(UpnpError::NotPublic(external_ip));
    }

    let mut last = UpnpError::Malformed("sin intentos");
    for offset in 0..PORT_ATTEMPTS {
        let external_port = internal_port.wrapping_add(offset).max(1024);
        match add_mapping(&gateway, external_port, internal_ip, internal_port, lease).await {
            Ok(()) => {
                return Ok(PortMapping {
                    external: SocketAddr::new(external_ip, external_port),
                    internal_port,
                    gateway,
                })
            }
            // 718: ese puerto externo ya lo tiene otro equipo. Se prueba el
            // siguiente.
            Err(UpnpError::Rejected {
                code: 718,
                description,
            }) => {
                last = UpnpError::Rejected {
                    code: 718,
                    description,
                };
            }
            Err(e) => return Err(e),
        }
    }
    Err(last)
}

async fn add_mapping(
    gateway: &Gateway,
    external_port: u16,
    internal_ip: IpAddr,
    internal_port: u16,
    lease: Duration,
) -> Result<(), UpnpError> {
    let body = |lease_secs: u64| {
        format!(
            "<NewRemoteHost></NewRemoteHost><NewExternalPort>{external_port}</NewExternalPort>\
             <NewProtocol>UDP</NewProtocol><NewInternalPort>{internal_port}</NewInternalPort>\
             <NewInternalClient>{internal_ip}</NewInternalClient><NewEnabled>1</NewEnabled>\
             <NewPortMappingDescription>Gravital Talk</NewPortMappingDescription>\
             <NewLeaseDuration>{lease_secs}</NewLeaseDuration>"
        )
    };
    match soap(gateway, "AddPortMapping", &body(lease.as_secs())).await {
        Ok(_) => Ok(()),
        // 725: el router sólo admite aperturas permanentes. Se acepta: se
        // cierra explícitamente al terminar la sala.
        Err(UpnpError::Rejected { code: 725, .. }) => {
            soap(gateway, "AddPortMapping", &body(0)).await.map(drop)
        }
        Err(e) => Err(e),
    }
}

/// IP pública que el router dice tener.
pub async fn external_ip(gateway: &Gateway) -> Result<IpAddr, UpnpError> {
    let response = soap(gateway, "GetExternalIPAddress", "").await?;
    tag(&response, "NewExternalIPAddress")
        .and_then(|ip| ip.trim().parse().ok())
        .ok_or(UpnpError::Malformed("sin IP externa"))
}

/// Localiza un router con apertura de puertos preguntando por SSDP a `target`.
///
/// `target` es [`SSDP_ADDR`] salvo en los tests, que apuntan a un router de
/// mentira en loopback.
pub async fn discover(target: SocketAddr) -> Result<Gateway, UpnpError> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
    let device = "urn:schemas-upnp-org:device:InternetGatewayDevice:1";
    for st in std::iter::once(&device).chain(SERVICES) {
        let search = format!(
            "M-SEARCH * HTTP/1.1\r\nHOST: 239.255.255.250:1900\r\nMAN: \"ssdp:discover\"\r\n\
             MX: 1\r\nST: {st}\r\n\r\n"
        );
        socket.send_to(search.as_bytes(), target).await?;
    }

    let mut buf = [0u8; 2048];
    let deadline = tokio::time::Instant::now() + DISCOVERY_TIMEOUT;
    let mut last = UpnpError::NoGateway;
    loop {
        let Ok(received) = tokio::time::timeout_at(deadline, socket.recv_from(&mut buf)).await
        else {
            return Err(last);
        };
        let (n, from) = received?;
        let text = String::from_utf8_lossy(&buf[..n]);
        let Some(location) = header(&text, "location") else {
            continue;
        };
        // Se usa el primer router que de verdad ofrezca el servicio: en una
        // red puede contestar más de un dispositivo UPnP.
        match gateway_from_description(location, from.ip()).await {
            Ok(gateway) => return Ok(gateway),
            Err(e) => last = e,
        }
    }
}

/// Descarga la descripción del router y saca de ella la URL de control.
async fn gateway_from_description(location: &str, from: IpAddr) -> Result<Gateway, UpnpError> {
    let (addr, path) = split_url(location).ok_or(UpnpError::Malformed("URL de descripción"))?;
    // La descripción tiene que servirla quien contestó: un dispositivo de la
    // red no debe poder mandarnos a hablar con un tercero.
    if addr.ip() != from {
        return Err(UpnpError::Malformed("la descripción apunta a otro equipo"));
    }
    let request = format!(
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\nAccept: text/xml\r\n\r\n"
    );
    let (status, body) = http(addr, request.as_bytes()).await?;
    if status != 200 {
        return Err(UpnpError::Malformed("descripción no disponible"));
    }
    for service in SERVICES {
        if let Some(control) = control_url(&body, service) {
            // La URL de control puede venir absoluta o relativa al router.
            let control_path = match split_url(control) {
                Some((_, path)) => path,
                None if control.starts_with('/') => control.to_owned(),
                None => format!("/{control}"),
            };
            return Ok(Gateway {
                addr,
                control_path,
                service: (*service).to_owned(),
            });
        }
    }
    Err(UpnpError::NoService)
}

/// Busca la `controlURL` del bloque `<service>` de tipo `service`.
fn control_url<'a>(description: &'a str, service: &str) -> Option<&'a str> {
    let at = description.find(service)?;
    let block_end = description[at..].find("</service>").map(|e| at + e)?;
    tag(&description[at..block_end], "controlURL").map(str::trim)
}

/// Ejecuta una acción SOAP y devuelve el cuerpo de la respuesta.
async fn soap(gateway: &Gateway, action: &str, arguments: &str) -> Result<String, UpnpError> {
    let service = &gateway.service;
    let envelope = format!(
        "<?xml version=\"1.0\"?>\r\n<s:Envelope xmlns:s=\"http://schemas.xmlsoap.org/soap/envelope/\" \
         s:encodingStyle=\"http://schemas.xmlsoap.org/soap/encoding/\"><s:Body>\
         <u:{action} xmlns:u=\"{service}\">{arguments}</u:{action}></s:Body></s:Envelope>\r\n"
    );
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\
         Content-Type: text/xml; charset=\"utf-8\"\r\nSOAPAction: \"{service}#{action}\"\r\n\
         Content-Length: {}\r\n\r\n{envelope}",
        gateway.control_path,
        gateway.addr,
        envelope.len()
    );
    let (status, body) = http(gateway.addr, request.as_bytes()).await?;
    if status == 200 {
        return Ok(body);
    }
    // Un error SOAP llega como 500 con el motivo dentro.
    let code = tag(&body, "errorCode").and_then(|c| c.trim().parse().ok());
    Err(
        code.map_or(UpnpError::Malformed("error sin código"), |code| {
            UpnpError::Rejected {
                code,
                description: tag(&body, "errorDescription")
                    .unwrap_or_default()
                    .trim()
                    .to_owned(),
            }
        }),
    )
}

/// Una petición HTTP/1.1 con `Connection: close`: se envía y se lee hasta el
/// final. Devuelve el código de estado y el cuerpo.
async fn http(addr: SocketAddr, request: &[u8]) -> Result<(u16, String), UpnpError> {
    let exchange = async {
        let mut stream = TcpStream::connect(addr).await?;
        stream.write_all(request).await?;
        let mut raw = Vec::new();
        stream
            .take(MAX_RESPONSE as u64)
            .read_to_end(&mut raw)
            .await?;
        Ok::<_, UpnpError>(raw)
    };
    let raw = timeout(HTTP_TIMEOUT, exchange)
        .await
        .map_err(|_| UpnpError::Timeout)??;
    let text = String::from_utf8_lossy(&raw);
    let status = text
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or(UpnpError::Malformed("sin línea de estado"))?;
    let body = text
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_owned())
        .unwrap_or_default();
    Ok((status, body))
}

/// IP local con la que este equipo alcanza a `gateway`.
async fn local_ip_towards(gateway: SocketAddr) -> Result<IpAddr, UpnpError> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
    socket.connect(gateway).await?;
    Ok(socket.local_addr()?.ip())
}

/// Contenido de la primera etiqueta `<name>…</name>` (con o sin prefijo de
/// espacio de nombres en la apertura no: los routers no lo ponen aquí).
fn tag<'a>(xml: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&format!("</{name}>"))? + start;
    Some(&xml[start..end])
}

/// Valor de una cabecera HTTP, sin distinguir mayúsculas en el nombre.
fn header<'a>(response: &'a str, name: &str) -> Option<&'a str> {
    response.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
    })
}

/// Parte `http://ip:puerto/ruta` en dirección y ruta.
fn split_url(url: &str) -> Option<(SocketAddr, String)> {
    let rest = url.trim().strip_prefix("http://")?;
    let (authority, path) = rest
        .find('/')
        .map_or((rest, "/"), |i| (&rest[..i], &rest[i..]));
    let addr = match authority.parse::<SocketAddr>() {
        Ok(addr) => addr,
        // Sin puerto explícito: el 80.
        Err(_) => SocketAddr::new(authority.parse().ok()?, 80),
    };
    Some((addr, path.to_owned()))
}

/// `true` si la dirección es enrutable desde internet.
///
/// Excluye las privadas y el rango de CGNAT (100.64.0.0/10), que es
/// precisamente el caso que hay que detectar.
#[must_use]
pub const fn is_public(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            let cgnat = o[0] == 100 && (o[1] & 0xC0) == 64;
            !(v4.is_private()
                || v4.is_loopback()
                || v4.is_link_local()
                || v4.is_unspecified()
                || v4.is_broadcast()
                || cgnat)
        }
        IpAddr::V6(v6) => !(v6.is_loopback() || v6.is_unspecified() || v6.is_unique_local()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DESCRIPTION: &str = "<root><device><serviceList><service>\
        <serviceType>urn:schemas-upnp-org:service:Layer3Forwarding:1</serviceType>\
        <controlURL>/ctl/L3F</controlURL></service></serviceList><deviceList><device>\
        <serviceList><service>\
        <serviceType>urn:schemas-upnp-org:service:WANIPConnection:1</serviceType>\
        <SCPDURL>/WANIPCn.xml</SCPDURL><controlURL>/ctl/IPConn</controlURL>\
        </service></serviceList></device></deviceList></device></root>";

    #[test]
    fn se_elige_la_url_de_control_del_servicio_correcto() {
        // El primer `controlURL` del documento es de otro servicio: cogerlo
        // sin mirar el tipo mandaría el AddPortMapping a quien no lo entiende.
        assert_eq!(
            control_url(
                DESCRIPTION,
                "urn:schemas-upnp-org:service:WANIPConnection:1"
            ),
            Some("/ctl/IPConn")
        );
        assert_eq!(
            control_url(
                DESCRIPTION,
                "urn:schemas-upnp-org:service:WANPPPConnection:1"
            ),
            None
        );
    }

    #[test]
    fn las_urls_se_parten_con_y_sin_puerto() {
        assert_eq!(
            split_url("http://192.168.0.1:5000/rootDesc.xml"),
            Some(("192.168.0.1:5000".parse().unwrap(), "/rootDesc.xml".into()))
        );
        assert_eq!(
            split_url("http://192.168.0.1"),
            Some(("192.168.0.1:80".parse().unwrap(), "/".into()))
        );
        assert_eq!(split_url("https://192.168.0.1/x"), None);
        assert_eq!(split_url("http://router.local/x"), None);
    }

    #[test]
    fn las_cabeceras_no_distinguen_mayusculas() {
        let response = "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=120\r\nLocation: http://192.168.0.1:5000/rootDesc.xml\r\n\r\n";
        assert_eq!(
            header(response, "LOCATION"),
            Some("http://192.168.0.1:5000/rootDesc.xml")
        );
        assert_eq!(header(response, "st"), None);
    }

    #[test]
    fn una_ip_de_cgnat_no_es_publica() {
        let ip = |s: &str| s.parse::<IpAddr>().unwrap();
        assert!(is_public(ip("190.140.113.206")));
        assert!(!is_public(ip("192.168.0.1")));
        assert!(!is_public(ip("10.135.11.188")));
        assert!(!is_public(ip("100.64.0.1")));
        assert!(!is_public(ip("100.127.255.254")));
        // Justo fuera del rango de CGNAT.
        assert!(is_public(ip("100.128.0.1")));
    }
}
