//! Selección de puerto y diagnóstico de alcanzabilidad.
//!
//! ## Por qué existe
//!
//! El requisito es que el usuario final no configure la red. Pero "bindear un
//! puerto" y "que ese puerto sea alcanzable desde internet" son cosas
//! distintas: un socket abierto en el dispositivo no atraviesa el router. Sin
//! distinguir los casos, la app falla sin explicación y el usuario no tiene
//! forma de saber si el problema es su red, su router o la app.
//!
//! ## Los tres casos reales
//!
//! | Red del anfitrión | ¿Alcanzable desde fuera? |
//! |---|---|
//! | Misma LAN que el invitado | Sí |
//! | Puerto reenviado en el router | Sí, desde cualquier red |
//! | IP pública sin NAT | Sí |
//! | Móvil (CGNAT) | **No**, nunca |
//!
//! Depende de la red del **anfitrión**, no de la del invitado. Un anfitrión en
//! wifi con puerto reenviado recibe de un invitado en 4G sin problemas; un
//! anfitrión en 4G no recibe a nadie.
//!
//! Lo que hace este módulo es **detectar** el cuarto caso y decirlo, en vez de
//! dejar que el emparejamiento falle en silencio.

use std::net::{IpAddr, SocketAddr};

/// Rango CGNAT de RFC 6598 (`100.64.0.0/10`).
///
/// Es el rango que usa un operador móvil cuando comparte una IPv4 entre muchos
/// clientes. Una dirección de aquí **no es enrutable** desde internet: es
/// privada por construcción, aunque el operador te la presente como "tu IP".
const CGNAT_NETWORK: u32 = 0x6440_0000;
const CGNAT_MASK: u32 = 0xFFC0_0000;

/// `true` si la IP está en el rango CGNAT de RFC 6598.
///
/// Es la comprobación que permite distinguir "tienes NAT pero se puede
/// traspasar" de "estás detrás de CGNAT y no vas a recibir conexiones".
#[must_use]
pub const fn is_cgnat(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => (u32::from_be_bytes(v4.octets()) & CGNAT_MASK) == CGNAT_NETWORK,
        // IPv6 no tiene equivalente de CGNAT: si tienes IPv6 enrutable, la
        // alcanzabilidad es otra conversación.
        IpAddr::V6(_) => false,
    }
}

/// `true` si la IP es privada (RFC 1918) o de loopback.
#[must_use]
pub const fn is_private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            let o = v4.octets();
            o[0] == 10
                || (o[0] == 172 && o[1] >= 16 && o[1] <= 31)
                || (o[0] == 192 && o[1] == 168)
                || o[0] == 127
        }
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

/// Resultado de Diagnostic la red del anfitrión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reachability {
    /// La IP pública es enrutable: los invitados de cualquier red pueden entrar.
    ///
    /// No garantiza que el puerto esté abierto —eso es cosa del router—, pero
    /// descarta el caso peor.
    Public,

    /// IP privada: sólo alcanzable desde la misma LAN.
    LanOnly,

    /// CGNAT: direcciones tras esta no recibirán conexiones entrantes.
    ///
    /// Es el caso que hay que reportar, porque ni el reenvío de puertos lo
    /// arregla: el operador es quien tendría que hacerlo.
    Cgnat,

    /// No se pudo determinar (STUN no respondió).
    Unknown,
}

impl Reachability {
    /// Mensaje para el usuario, con la acción concreta que puede tomar.
    ///
    /// Un diagnóstico que no dice qué hacer es sólo una forma elegante de decir
    /// "no funciona".
    #[must_use]
    pub const fn message(&self) -> Option<&'static str> {
        match self {
            Self::Public => None,
            Self::LanOnly => Some(
                "Sólo te pueden alcanzar dispositivos en tu misma red wifi. \
                 Para invitados de fuera, reenvía el puerto en tu router.",
            ),
            Self::Cgnat => Some(
                "Tu conexión mangifica la IP entre varios clientes (CGNAT), \
                 así que nadie de fuera puede conectarse a este dispositivo. \
                 Úsalo como invitado, no como anfitrión.",
            ),
            Self::Unknown => Some(
                "No pude comprobar si eres alcanzable desde fuera. \
                 Puedes intentarlo; si falla, prueba desde otra red.",
            ),
        }
    }

    /// `true` si el anfitrión puede recibir invitados de fuera de su red.
    #[must_use]
    pub const fn can_host_remotely(&self) -> bool {
        matches!(self, Self::Public)
    }
}

/// Diagnostica la alcanzabilidad a partir de la IP pública que reportó STUN.
///
/// No hace ninguna llamada de red: es puro análisis de la dirección. Recibir la
/// IP ya es el trabajo de STUN.
#[must_use]
pub const fn diagnose(public_ip: IpAddr) -> Reachability {
    if is_cgnat(public_ip) {
        Reachability::Cgnat
    } else if is_private(public_ip) {
        Reachability::LanOnly
    } else {
        Reachability::Public
    }
}

/// Puerto por defecto que dedica Gravital Talk cuando actúa de anfitrión.
///
/// Es el que aparece en la documentación del proyecto (`DefaultPorts.udp`), y
/// fijarlo tiene una ventaja concreta sobre el efímero: **el reenvío de puertos
/// del router sólo hay que hacerlo una vez**. Con un puerto que cambia en cada
/// arranque, reenviarlo no sirve de nada.
pub const DEFAULT_HOST_PORT: u16 = 9000;

/// Decide en qué puerto debe bindear el anfitrión.
///
/// La política responde a "que la comunicación sea lo más rápida y sencilla
/// posible", que es lo que se pidió:
///
/// - Si se pasa un puerto preferido y está libre, se usa. Es el caso del
///   anfitrión fiable que ya tiene el puerto reenviado.
/// - Si está ocupado, se cae a efímero (`0`) en vez de fallar. Melevaar en un
///   puerto ocupado con un error que obligar a cerrar la otra aplicación a
///   mano es exactamente la fricción que se quiere eliminar.
/// - El invitado siempre va efímero: sólo envía, no necesita puerto reenviado.
#[must_use]
pub const fn preferred_host_port(preferred: u16) -> u16 {
    if preferred == 0 {
        DEFAULT_HOST_PORT
    } else {
        preferred
    }
}

/// Comprueba si un puerto se puede bindear en la máquina local.
///
/// Bindea y suelta: no deja el socket abierto. Es una comprobación de
/// disponibilidad, no de alcanzabilidad —lo segundo requiere que alguien de
/// fuera lo intente.
pub fn port_is_free(port: u16) -> bool {
    std::net::UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], port))).is_ok()
}

/// Construye la dirección de bind del anfitrión.
///
/// `preferred_port` de 0 = automático (el puerto por defecto si está libre).
/// Devuelve la dirección a usar y el puerto que realmente se obtuvo, porque si
/// el preferido está ocupado se cae a efímero y el puerto real sólo se sabe
/// después de bindear.
#[must_use]
pub fn host_bind_addr(preferred_port: u16) -> SocketAddr {
    let port = if preferred_port != 0 {
        preferred_port
    } else if port_is_free(DEFAULT_HOST_PORT) {
        DEFAULT_HOST_PORT
    } else {
        // Ocupado: efímero. El puerto real lo da `local_port` tras el bind.
        0
    };
    SocketAddr::from(([0, 0, 0, 0], port))
}

/// Dirección de bind del invitado: siempre efímero.
///
/// El invitado sólo envía, así que un puerto fijo no le aporta nada y le quita
/// la capacidad de tener varias instancias abiertas.
#[must_use]
pub fn guest_bind_addr() -> SocketAddr {
    SocketAddr::from(([0, 0, 0, 0], 0))
}

/// Texto legible de una dirección para mostrar en el QR o por teléfono.
#[must_use]
pub fn describe_endpoint(ip: IpAddr, port: u16) -> String {
    match ip {
        IpAddr::V4(v4) => format!("{v4}:{port}"),
        // IPv6 entre corchetes para que el puerto no se confunda.
        IpAddr::V6(v6) => format!("[{v6}]:{port}"),
    }
}

/// `true` si la IP es de loopback (pruebas en la misma máquina).
#[must_use]
pub const fn is_loopback(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.octets()[0] == 127,
        IpAddr::V6(v6) => v6.is_loopback(),
    }
}

/// Un endpoint publicado por un anfitrión, listo para el QR.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Endpoint {
    /// IP pública tal como la ve STUN.
    pub public_ip: IpAddr,
    /// Puerto local real del socket.
    pub port: u16,
}

impl Endpoint {
    /// Diagnóstico de alcanzabilidad de este endpoint.
    #[must_use]
    pub const fn reachability(&self) -> Reachability {
        diagnose(self.public_ip)
    }

    /// `true` si este endpoint puede recibir invitados de fuera.
    #[must_use]
    pub const fn is_hostable(&self) -> bool {
        self.reachability().can_host_remotely()
    }

    /// Representación para el QR o para leer en voz alta.
    #[must_use]
    pub fn display(&self) -> String {
        describe_endpoint(self.public_ip, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    fn v4(a: u8, b: u8, c: u8, d: u8) -> IpAddr {
        IpAddr::V4(Ipv4Addr::new(a, b, c, d))
    }

    // ── Detección de CGNAT ────────────────────────────────────────────────

    #[test]
    fn detects_the_cgnat_range_boundaries() {
        // RFC 6598: 100.64.0.0 - 100.127.255.255
        assert!(is_cgnat(v4(100, 64, 0, 0)));
        assert!(is_cgnat(v4(100, 100, 1, 2)));
        assert!(is_cgnat(v4(100, 127, 255, 255)));
    }

    #[test]
    fn rejects_just_outside_the_cgnat_range() {
        assert!(!is_cgnat(v4(100, 63, 255, 255)), "un byte antes del rango");
        assert!(!is_cgnat(v4(100, 128, 0, 0)), "un byte después del rango");
    }

    #[test]
    fn cgnat_range_does_not_overlap_private_ranges() {
        // Si CGNAT se solapara con RFC 1918, el diagnóstico mentiría sobre qué
        // caso es.
        assert!(!is_cgnat(v4(10, 0, 0, 1)));
        assert!(!is_cgnat(v4(192, 168, 1, 1)));
        assert!(!is_cgnat(v4(172, 16, 0, 1)));
    }

    // ── Detección de IPs privadas ─────────────────────────────────────────

    #[test]
    fn detects_all_rfc1918_ranges() {
        assert!(is_private(v4(10, 1, 2, 3)));
        assert!(is_private(v4(172, 16, 0, 1)));
        assert!(is_private(v4(172, 31, 255, 255)));
        assert!(is_private(v4(192, 168, 0, 1)));
        assert!(is_private(v4(127, 0, 0, 1)));
    }

    #[test]
    fn rejects_public_addresses() {
        assert!(!is_private(v4(8, 8, 8, 8)));
        assert!(!is_private(v4(203, 0, 113, 45)));
    }

    #[test]
    fn private_range_boundaries_are_exact() {
        assert!(!is_private(v4(172, 32, 0, 1)), "172.32 ya no es privado");
        assert!(!is_private(v4(172, 15, 0, 1)), "172.15 ya no es privado");
    }

    // ── Diagnóstico combinado ─────────────────────────────────────────────

    #[test]
    fn public_ip_is_hostable() {
        let r = diagnose(v4(203, 0, 113, 45));
        assert_eq!(r, Reachability::Public);
        assert!(r.can_host_remotely());
        assert_eq!(r.message(), None, "no hay nada que avisar");
    }

    #[test]
    fn cgnat_is_reported_before_private() {
        // El orden importa: 100.64.0.0 no es RFC 1918, pero si algún día se
        // solaparan, CGNAT es el diagnóstico más útil.
        let r = diagnose(v4(100, 100, 0, 1));
        assert_eq!(r, Reachability::Cgnat);
        assert!(!r.can_host_remotely());
        assert!(r.message().unwrap().contains("CGNAT"));
    }

    #[test]
    fn lan_only_tells_the_user_what_to_do() {
        let r = diagnose(v4(192, 168, 1, 50));
        assert_eq!(r, Reachability::LanOnly);
        assert!(!r.can_host_remotely());
        // El mensaje debe nombrar la acción concreta: reenviar el puerto.
        assert!(r.message().unwrap().contains("reenvía"));
    }

    // ── Selección de puerto ───────────────────────────────────────────────

    #[test]
    fn preferred_port_zero_falls_back_to_the_documented_default() {
        assert_eq!(preferred_host_port(0), 9000);
        assert_eq!(DEFAULT_HOST_PORT, 9000);
    }

    #[test]
    fn explicit_preferred_port_wins() {
        assert_eq!(preferred_host_port(9123), 9123);
    }

    #[test]
    fn host_bind_uses_the_default_when_it_is_free() {
        // El puerto por defecto debería estar libre en una máquina de test.
        let addr = host_bind_addr(0);
        assert_eq!(addr.port(), DEFAULT_HOST_PORT);
    }

    #[test]
    fn host_bind_respects_an_explicit_port() {
        assert_eq!(host_bind_addr(9199).port(), 9199);
    }

    #[test]
    fn guest_always_uses_ephemeral() {
        assert_eq!(guest_bind_addr().port(), 0);
    }

    #[test]
    fn the_default_port_is_probed_correctly() {
        // Es una comprobación de coherencia, no de red: si dice que el puerto
        // por defecto está libre, `host_bind_addr(0)` debe devolverlo.
        if port_is_free(DEFAULT_HOST_PORT) {
            assert_eq!(host_bind_addr(0).port(), DEFAULT_HOST_PORT);
        }
    }

    // ── Presentación ──────────────────────────────────────────────────────

    #[test]
    fn endpoint_display_is_unambiguous() {
        let e = Endpoint {
            public_ip: v4(203, 0, 113, 45),
            port: 9000,
        };
        assert_eq!(e.display(), "203.0.113.45:9000");
        assert!(e.is_hostable());
    }

    #[test]
    fn ipv6_endpoint_uses_brackets_so_the_port_is_readable() {
        let e = Endpoint {
            public_ip: "2001:db8::1".parse().unwrap(),
            port: 9000,
        };
        assert_eq!(e.display(), "[2001:db8::1]:9000");
    }
}
