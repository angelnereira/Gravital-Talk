//! Apertura de puerto contra un router UPnP de mentira.
//!
//! El router falso habla lo justo del protocolo real —SSDP, la descripción y
//! SOAP— sobre loopback, y lleva la cuenta de lo que se le pide. Sirve para
//! fijar el diálogo completo y los casos que con un router de verdad no se
//! pueden provocar a voluntad: puerto ocupado, concesiones sólo permanentes y
//! doble NAT.

// El router falso guarda su estado tras un único cerrojo que cada petición
// coge entera: no hay contención que afinar en un test.
#![allow(clippy::significant_drop_tightening)]

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gravital_talk_transport::upnp::{self, UpnpError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, UdpSocket};

const SERVICE: &str = "urn:schemas-upnp-org:service:WANIPConnection:1";

#[derive(Default)]
struct Router {
    external_ip: String,
    /// Puertos externos ya cogidos por "otros equipos".
    taken: Vec<u16>,
    /// Rechaza concesiones con caducidad (error 725).
    permanent_only: bool,
    /// Lo que se ha llegado a abrir: (externo, interno, cliente, concesión).
    mappings: Vec<(u16, u16, String, u64)>,
    deleted: Vec<u16>,
}

fn field<'a>(xml: &'a str, name: &str) -> &'a str {
    let open = format!("<{name}>");
    let start = xml.find(&open).map_or(0, |i| i + open.len());
    let end = xml[start..].find('<').map_or(start, |e| start + e);
    &xml[start..end]
}

fn soap_ok(action: &str, inner: &str) -> (u16, String) {
    (
        200,
        format!("<s:Envelope><s:Body><u:{action}Response xmlns:u=\"{SERVICE}\">{inner}</u:{action}Response></s:Body></s:Envelope>"),
    )
}

fn soap_error(code: u32, text: &str) -> (u16, String) {
    (
        500,
        format!("<s:Envelope><s:Body><s:Fault><detail><UPnPError><errorCode>{code}</errorCode><errorDescription>{text}</errorDescription></UPnPError></detail></s:Fault></s:Body></s:Envelope>"),
    )
}

fn handle(router: &Mutex<Router>, request: &str) -> (u16, String) {
    let mut r = router.lock().unwrap();
    if request.starts_with("GET /rootDesc.xml") {
        return (
            200,
            format!(
                "<root><device><serviceList><service>\
                 <serviceType>urn:schemas-upnp-org:service:Layer3Forwarding:1</serviceType>\
                 <controlURL>/ctl/L3F</controlURL></service><service>\
                 <serviceType>{SERVICE}</serviceType><controlURL>/ctl/IPConn</controlURL>\
                 </service></serviceList></device></root>"
            ),
        );
    }
    if !request.starts_with("POST /ctl/IPConn") {
        return (404, String::new());
    }
    if request.contains("#GetExternalIPAddress") {
        let ip = r.external_ip.clone();
        return soap_ok(
            "GetExternalIPAddress",
            &format!("<NewExternalIPAddress>{ip}</NewExternalIPAddress>"),
        );
    }
    if request.contains("#AddPortMapping") {
        let external: u16 = field(request, "NewExternalPort").parse().unwrap();
        let internal: u16 = field(request, "NewInternalPort").parse().unwrap();
        let lease: u64 = field(request, "NewLeaseDuration").parse().unwrap();
        assert_eq!(field(request, "NewProtocol"), "UDP");
        if r.taken.contains(&external) {
            return soap_error(718, "ConflictInMappingEntry");
        }
        if r.permanent_only && lease != 0 {
            return soap_error(725, "OnlyPermanentLeasesSupported");
        }
        let client = field(request, "NewInternalClient").to_owned();
        r.mappings.push((external, internal, client, lease));
        return soap_ok("AddPortMapping", "");
    }
    if request.contains("#DeletePortMapping") {
        r.deleted
            .push(field(request, "NewExternalPort").parse().unwrap());
        return soap_ok("DeletePortMapping", "");
    }
    soap_error(401, "Invalid Action")
}

/// Arranca el router falso y devuelve a dónde mandar el descubrimiento.
async fn spawn_router(router: Router) -> (SocketAddr, Arc<Mutex<Router>>) {
    let router = Arc::new(Mutex::new(router));
    let http = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let http_addr = http.local_addr().unwrap();
    let ssdp = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let ssdp_addr = ssdp.local_addr().unwrap();

    tokio::spawn(async move {
        let mut buf = [0u8; 1024];
        while let Ok((_, from)) = ssdp.recv_from(&mut buf).await {
            let reply = format!(
                "HTTP/1.1 200 OK\r\nCACHE-CONTROL: max-age=120\r\nST: {SERVICE}\r\n\
                 LOCATION: http://{http_addr}/rootDesc.xml\r\n\r\n"
            );
            let _ = ssdp.send_to(reply.as_bytes(), from).await;
        }
    });

    let state = router.clone();
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = http.accept().await {
            let state = state.clone();
            tokio::spawn(async move {
                // Leer cabeceras y, si hay Content-Length, el cuerpo entero.
                let mut raw = Vec::new();
                let mut chunk = [0u8; 2048];
                loop {
                    let Ok(n) = stream.read(&mut chunk).await else {
                        return;
                    };
                    if n == 0 {
                        break;
                    }
                    raw.extend_from_slice(&chunk[..n]);
                    let text = String::from_utf8_lossy(&raw);
                    if let Some((head, body)) = text.split_once("\r\n\r\n") {
                        let expected = head
                            .lines()
                            .find_map(|l| l.strip_prefix("Content-Length: "))
                            .and_then(|v| v.trim().parse::<usize>().ok())
                            .unwrap_or(0);
                        if body.len() >= expected {
                            break;
                        }
                    }
                }
                let (status, body) = handle(&state, &String::from_utf8_lossy(&raw));
                let response = format!(
                    "HTTP/1.1 {status} X\r\nContent-Type: text/xml\r\nContent-Length: {}\r\n\
                     Connection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(response.as_bytes()).await;
            });
        }
    });

    (ssdp_addr, router)
}

const LEASE: Duration = Duration::from_secs(3600);

#[tokio::test]
async fn abre_el_puerto_y_devuelve_la_direccion_publica() {
    let (ssdp, router) = spawn_router(Router {
        external_ip: "203.0.113.9".into(),
        ..Default::default()
    })
    .await;

    let gateway = upnp::discover(ssdp).await.expect("descubrir");
    assert_eq!(gateway.control_path, "/ctl/IPConn");
    let mapping = upnp::map_udp_port_on(gateway, 40_000, LEASE)
        .await
        .expect("abrir");

    assert_eq!(mapping.external, "203.0.113.9:40000".parse().unwrap());
    {
        let r = router.lock().unwrap();
        assert_eq!(r.mappings, vec![(40_000, 40_000, "127.0.0.1".into(), 3600)]);
    }

    mapping.remove().await.expect("cerrar");
    assert_eq!(router.lock().unwrap().deleted, vec![40_000]);
}

#[tokio::test]
async fn si_el_puerto_esta_cogido_prueba_el_siguiente() {
    let (ssdp, router) = spawn_router(Router {
        external_ip: "203.0.113.9".into(),
        taken: vec![40_000, 40_001],
        ..Default::default()
    })
    .await;

    let gateway = upnp::discover(ssdp).await.unwrap();
    let mapping = upnp::map_udp_port_on(gateway, 40_000, LEASE).await.unwrap();

    // El puerto público cambia; el local al que reenvía, no.
    assert_eq!(mapping.external.port(), 40_002);
    assert_eq!(mapping.internal_port, 40_000);
    assert_eq!(router.lock().unwrap().mappings[0].1, 40_000);
}

#[tokio::test]
async fn un_router_que_solo_admite_concesiones_permanentes_tambien_sirve() {
    let (ssdp, router) = spawn_router(Router {
        external_ip: "203.0.113.9".into(),
        permanent_only: true,
        ..Default::default()
    })
    .await;

    let gateway = upnp::discover(ssdp).await.unwrap();
    upnp::map_udp_port_on(gateway, 40_000, LEASE).await.unwrap();
    assert_eq!(router.lock().unwrap().mappings[0].3, 0);
}

#[tokio::test]
async fn doble_nat_se_detecta_y_no_se_abre_nada() {
    // El router está detrás del NAT del operador: su "IP externa" es de
    // CGNAT. Abrir el puerto ahí no hace a nadie alcanzable, y publicar esa
    // dirección sería prometer una conexión que no va a funcionar.
    let (ssdp, router) = spawn_router(Router {
        external_ip: "100.72.14.3".into(),
        ..Default::default()
    })
    .await;

    let gateway = upnp::discover(ssdp).await.unwrap();
    let err = upnp::map_udp_port_on(gateway, 40_000, LEASE)
        .await
        .unwrap_err();
    assert!(matches!(err, UpnpError::NotPublic(_)), "{err}");
    assert!(router.lock().unwrap().mappings.is_empty());
}

#[tokio::test]
async fn sin_router_que_conteste_el_fallo_es_claro_y_rapido() {
    // Un puerto donde no escucha nadie.
    let silent = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let target = silent.local_addr().unwrap();
    let started = std::time::Instant::now();
    let err = upnp::discover(target).await.unwrap_err();
    assert!(matches!(err, UpnpError::NoGateway), "{err}");
    assert!(started.elapsed() < Duration::from_secs(3));
}
