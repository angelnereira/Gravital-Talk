//! Encuentro y perforación de NAT.
//!
//! ## El problema
//!
//! Un NAT deja salir tráfico pero sólo deja entrar lo que parece respuesta a
//! algo que salió antes. El anfitrión de una sala abre un puerto y espera; el
//! primer paquete del invitado llega al router del anfitrión, que no esperaba
//! nada de esa dirección, y lo tira. Por eso sin esto sólo se podía emparejar
//! dentro de la misma red.
//!
//! ## La solución
//!
//! Si los dos envían un paquete al otro, cada router ve tráfico saliente hacia
//! esa dirección y deja pasar lo que vuelve. Para eso cada extremo tiene que
//! saber la dirección pública del otro, y el anfitrión no puede sacarla de
//! ningún sitio: el QR sólo viaja de anfitrión a invitado.
//!
//! De ahí el **servidor de encuentro**: un servicio UDP al que los dos se
//! registran con la misma clave de sala y que contesta a cada uno con la
//! dirección desde la que ve al otro. No reenvía audio ni ve el token de la
//! sala; sólo empareja direcciones. Corre dentro de `gs-relay`.
//!
//! ## Mensajes
//!
//! Van fuera del formato de paquete de Gravital Talk a propósito: son previos
//! a la sesión y los tiene que poder contestar un servidor que no participa
//! en ella. Empiezan por `GTRV`, que como cabecera de paquete es inválido, así
//! que ni el relay ni una sesión los confunden con tráfico normal.
//!
//! ```text
//! magic "GTRV" (4) | versión (1) | tipo (1) | clave de sala (16) | cuerpo
//! ```
//!
//! | Tipo | Sentido | Cuerpo |
//! |---|---|---|
//! | `REGISTER` | extremo → servidor | relleno hasta 128 B |
//! | `PEERS` | servidor → extremo | tu dirección + las de los demás |
//! | `PUNCH` | extremo → extremo | nonce (8) |
//! | `PUNCH_ACK` | extremo → extremo | el mismo nonce |
//!
//! `REGISTER` va rellenado para que la respuesta nunca sea mayor que la
//! petición: un servidor UDP que contesta más de lo que recibe sirve para
//! amplificar ataques con origen falsificado.
//!
//! ## Lo que no resuelve
//!
//! Un NAT **simétrico** asigna un puerto distinto para cada destino: el que ve
//! el servidor de encuentro no es el que usará hacia el otro extremo. Si los
//! dos lados son así, no hay perforación posible y hace falta que el relay
//! reenvíe el audio. Si sólo lo es uno, suele funcionar: el otro extremo
//! aprende la dirección real del primer `PUNCH` que le llega.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

/// Marca de los mensajes de encuentro.
pub const MAGIC: [u8; 4] = *b"GTRV";

/// Versión del formato. Un mensaje de otra versión se ignora.
pub const VERSION: u8 = 1;

/// Tamaño fijo de un `REGISTER`.
pub const REGISTER_SIZE: usize = 128;

/// Peers por sala que guarda y devuelve el servidor.
///
/// Acota tanto la memoria como el tamaño de `PEERS`, que tiene que caber en
/// [`REGISTER_SIZE`].
pub const MAX_PEERS_PER_ROOM: usize = 4;

/// Cuánto vive un registro sin renovarse.
pub const REGISTRATION_TTL: Duration = Duration::from_secs(30);

const HEADER_SIZE: usize = 4 + 1 + 1 + 16;
const KIND_REGISTER: u8 = 1;
const KIND_PEERS: u8 = 2;
const KIND_PUNCH: u8 = 3;
const KIND_PUNCH_ACK: u8 = 4;
const FAMILY_V4: u8 = 4;
const FAMILY_V6: u8 = 6;

/// Clave con la que dos extremos se reconocen en el servidor de encuentro.
///
/// Deriva del token de la sala pero no lo revela: el servidor empareja a
/// quienes presentan la misma clave sin poder entrar él en la sala.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RoomKey(pub [u8; 16]);

impl RoomKey {
    /// Deriva la clave del token de la sala.
    ///
    /// El prefijo separa este uso del PSK de Noise, que es `SHA-256(token)` a
    /// secas: con el mismo hash, la clave pública de encuentro sería la mitad
    /// del secreto de la sesión.
    #[must_use]
    pub fn from_token(token: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"gravital-talk/rendezvous/v1\0");
        hasher.update(token);
        let digest = hasher.finalize();
        let mut key = [0u8; 16];
        key.copy_from_slice(&digest[..16]);
        Self(key)
    }
}

/// Un mensaje de encuentro ya interpretado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    /// "Estoy en esta sala": lo manda cada extremo al servidor.
    Register,
    /// Respuesta del servidor: la dirección pública de quien pregunta y las
    /// de los demás registrados en la sala.
    Peers {
        you: SocketAddr,
        others: Vec<SocketAddr>,
    },
    /// Sonda entre extremos. Abre el NAT de quien la envía.
    Punch { nonce: [u8; 8] },
    /// Confirmación de una sonda: el camino funciona en los dos sentidos.
    PunchAck { nonce: [u8; 8] },
}

/// `true` si el datagrama empieza como un mensaje de encuentro.
///
/// Es la comprobación barata que hace quien comparte socket con otro
/// protocolo, antes de intentar decodificar.
#[must_use]
pub fn is_rendezvous(datagram: &[u8]) -> bool {
    datagram.len() >= HEADER_SIZE && datagram[..4] == MAGIC
}

impl Message {
    /// Serializa el mensaje para la sala `key`.
    #[must_use]
    pub fn encode(&self, key: RoomKey) -> Vec<u8> {
        let mut out = Vec::with_capacity(REGISTER_SIZE);
        out.extend_from_slice(&MAGIC);
        out.push(VERSION);
        out.push(match self {
            Self::Register => KIND_REGISTER,
            Self::Peers { .. } => KIND_PEERS,
            Self::Punch { .. } => KIND_PUNCH,
            Self::PunchAck { .. } => KIND_PUNCH_ACK,
        });
        out.extend_from_slice(&key.0);
        match self {
            Self::Register => out.resize(REGISTER_SIZE, 0),
            Self::Peers { you, others } => {
                put_addr(&mut out, *you);
                let others = &others[..others.len().min(MAX_PEERS_PER_ROOM)];
                out.push(others.len() as u8);
                for addr in others {
                    put_addr(&mut out, *addr);
                }
            }
            Self::Punch { nonce } | Self::PunchAck { nonce } => out.extend_from_slice(nonce),
        }
        out
    }

    /// Interpreta un datagrama. `None` si no es un mensaje de encuentro
    /// válido de esta versión.
    #[must_use]
    pub fn decode(datagram: &[u8]) -> Option<(RoomKey, Self)> {
        if !is_rendezvous(datagram) || datagram[4] != VERSION {
            return None;
        }
        let mut key = [0u8; 16];
        key.copy_from_slice(&datagram[6..HEADER_SIZE]);
        let body = &datagram[HEADER_SIZE..];
        let msg = match datagram[5] {
            // Un REGISTER corto se rechaza: es justo el que permitiría
            // amplificar.
            KIND_REGISTER if datagram.len() >= REGISTER_SIZE => Self::Register,
            KIND_PEERS => {
                let mut rest = body;
                let you = take_addr(&mut rest)?;
                let (&count, mut rest) = rest.split_first()?;
                if usize::from(count) > MAX_PEERS_PER_ROOM {
                    return None;
                }
                let mut others = Vec::with_capacity(usize::from(count));
                for _ in 0..count {
                    others.push(take_addr(&mut rest)?);
                }
                Self::Peers { you, others }
            }
            KIND_PUNCH => Self::Punch {
                nonce: body.get(..8)?.try_into().ok()?,
            },
            KIND_PUNCH_ACK => Self::PunchAck {
                nonce: body.get(..8)?.try_into().ok()?,
            },
            _ => return None,
        };
        Some((RoomKey(key), msg))
    }
}

fn put_addr(out: &mut Vec<u8>, addr: SocketAddr) {
    match addr.ip() {
        IpAddr::V4(ip) => {
            out.push(FAMILY_V4);
            out.extend_from_slice(&ip.octets());
        }
        IpAddr::V6(ip) => {
            out.push(FAMILY_V6);
            out.extend_from_slice(&ip.octets());
        }
    }
    out.extend_from_slice(&addr.port().to_be_bytes());
}

fn take_addr(rest: &mut &[u8]) -> Option<SocketAddr> {
    let (&family, tail) = rest.split_first()?;
    let (ip, tail) = match family {
        FAMILY_V4 => {
            let octets: [u8; 4] = tail.get(..4)?.try_into().ok()?;
            (IpAddr::V4(Ipv4Addr::from(octets)), &tail[4..])
        }
        FAMILY_V6 => {
            let octets: [u8; 16] = tail.get(..16)?.try_into().ok()?;
            (IpAddr::V6(Ipv6Addr::from(octets)), &tail[16..])
        }
        _ => return None,
    };
    let port = u16::from_be_bytes(tail.get(..2)?.try_into().ok()?);
    *rest = &tail[2..];
    Some(SocketAddr::new(ip, port))
}

/// Nonce aleatorio para una sonda.
#[must_use]
pub fn random_nonce() -> [u8; 8] {
    use rand_core::RngCore;
    let mut nonce = [0u8; 8];
    rand_core::OsRng.fill_bytes(&mut nonce);
    nonce
}

/// `true` si la dirección es de una red local (o de la propia máquina).
///
/// Entre dos caminos que funcionan se prefiere el local: no sale a internet,
/// así que es el de menor latencia.
#[must_use]
pub const fn is_local(addr: &SocketAddr) -> bool {
    match addr.ip() {
        IpAddr::V4(ip) => ip.is_private() || ip.is_loopback() || ip.is_link_local(),
        IpAddr::V6(ip) => ip.is_loopback() || ip.is_unique_local() || ip.is_unicast_link_local(),
    }
}

// ── Lado servidor ─────────────────────────────────────────────────────────

/// Registro de quién está en cada sala, para el servidor de encuentro.
///
/// Es sólo estado: no abre sockets. Lo usa `gs-relay` desde su lazo UDP, y los
/// tests desde un socket local.
#[derive(Debug)]
pub struct Registry {
    rooms: Mutex<HashMap<RoomKey, Vec<(SocketAddr, Instant)>>>,
    max_rooms: usize,
}

impl Registry {
    /// Crea un registro que guarda como mucho `max_rooms` salas a la vez.
    #[must_use]
    pub fn new(max_rooms: usize) -> Self {
        Self {
            rooms: Mutex::new(HashMap::new()),
            max_rooms,
        }
    }

    /// Procesa un datagrama y devuelve la respuesta para su remitente.
    ///
    /// `None` si no es un `REGISTER` válido o no hay sitio: en ambos casos el
    /// servidor calla, que es lo que debe hacer ante tráfico que no entiende.
    pub fn handle(&self, datagram: &[u8], from: SocketAddr) -> Option<Vec<u8>> {
        let (key, msg) = Message::decode(datagram)?;
        if msg != Message::Register {
            return None;
        }
        let others = self.register(key, from, Instant::now())?;
        Some(Message::Peers { you: from, others }.encode(key))
    }

    /// Da de alta (o renueva) a `from` en la sala y devuelve a los demás.
    ///
    /// `None` si la sala está llena o se alcanzó el tope de salas.
    pub fn register(
        &self,
        key: RoomKey,
        from: SocketAddr,
        now: Instant,
    ) -> Option<Vec<SocketAddr>> {
        let mut rooms = match self.rooms.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        if !rooms.contains_key(&key) && rooms.len() >= self.max_rooms {
            // Antes de rechazar se hace sitio: lo caducado no debe bloquear a
            // nadie.
            rooms.retain(|_, peers| {
                peers.retain(|(_, seen)| now.duration_since(*seen) < REGISTRATION_TTL);
                !peers.is_empty()
            });
            if rooms.len() >= self.max_rooms {
                return None;
            }
        }
        let peers = rooms.entry(key).or_default();
        peers.retain(|(_, seen)| now.duration_since(*seen) < REGISTRATION_TTL);
        if let Some(entry) = peers.iter_mut().find(|(addr, _)| *addr == from) {
            entry.1 = now;
        } else if peers.len() < MAX_PEERS_PER_ROOM {
            peers.push((from, now));
        } else {
            return None;
        }
        let others = peers
            .iter()
            .map(|(addr, _)| *addr)
            .filter(|addr| *addr != from)
            .collect();
        drop(rooms);
        Some(others)
    }

    /// Salas con algún registro vigente.
    #[must_use]
    pub fn rooms(&self) -> usize {
        match self.rooms.lock() {
            Ok(g) => g.len(),
            Err(p) => p.into_inner().len(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(s: &str) -> SocketAddr {
        s.parse().unwrap()
    }

    #[test]
    fn los_mensajes_sobreviven_a_ida_y_vuelta() {
        let key = RoomKey::from_token(b"secreto");
        let cases = [
            Message::Register,
            Message::Peers {
                you: addr("203.0.113.7:40000"),
                others: vec![addr("198.51.100.2:9000"), addr("[2001:db8::1]:5000")],
            },
            Message::Punch { nonce: [7; 8] },
            Message::PunchAck { nonce: [9; 8] },
        ];
        for msg in cases {
            let bytes = msg.encode(key);
            assert_eq!(Message::decode(&bytes), Some((key, msg)));
        }
    }

    #[test]
    fn la_respuesta_nunca_es_mayor_que_la_peticion() {
        // Es la propiedad que impide usar el servidor para amplificar: se
        // comprueba en el peor caso, sala llena de direcciones IPv6.
        let key = RoomKey::from_token(b"x");
        let v6 = addr("[2001:db8:ffff:ffff:ffff:ffff:ffff:ffff]:65535");
        let worst = Message::Peers {
            you: v6,
            others: vec![v6; MAX_PEERS_PER_ROOM],
        };
        assert!(worst.encode(key).len() <= REGISTER_SIZE);
        assert_eq!(Message::Register.encode(key).len(), REGISTER_SIZE);
    }

    #[test]
    fn un_register_corto_se_rechaza() {
        let key = RoomKey::from_token(b"x");
        let full = Message::Register.encode(key);
        assert!(Message::decode(&full[..HEADER_SIZE]).is_none());
    }

    #[test]
    fn un_paquete_de_gravital_no_es_un_mensaje_de_encuentro() {
        // Versión 1 en el primer byte: lo que manda una sesión normal.
        let mut packet = [0u8; 64];
        packet[0] = 1;
        assert!(!is_rendezvous(&packet));
        assert!(Message::decode(&packet).is_none());
    }

    #[test]
    fn la_clave_no_es_el_psk_de_la_sala() {
        let token = b"mismo token";
        let psk = Sha256::digest(token);
        assert_ne!(RoomKey::from_token(token).0, psk[..16]);
        assert_ne!(RoomKey::from_token(token), RoomKey::from_token(b"otro"));
    }

    #[test]
    fn el_registro_presenta_a_los_dos_extremos() {
        let reg = Registry::new(8);
        let key = RoomKey::from_token(b"sala");
        let (a, b) = (addr("203.0.113.1:1000"), addr("203.0.113.2:2000"));
        let now = Instant::now();

        assert_eq!(reg.register(key, a, now), Some(vec![]));
        assert_eq!(reg.register(key, b, now), Some(vec![a]));
        // Al renovar, el primero ya ve al segundo.
        assert_eq!(reg.register(key, a, now), Some(vec![b]));
        // Otra sala no ve a ninguno.
        let other = RoomKey::from_token(b"otra");
        assert_eq!(reg.register(other, a, now), Some(vec![]));
    }

    #[test]
    fn los_registros_caducan_y_la_sala_llena_rechaza() {
        let reg = Registry::new(8);
        let key = RoomKey::from_token(b"sala");
        let t0 = Instant::now();
        for i in 0..MAX_PEERS_PER_ROOM {
            let a = addr(&format!("203.0.113.1:{}", 1000 + i));
            assert!(reg.register(key, a, t0).is_some());
        }
        let late = addr("203.0.113.9:9");
        assert_eq!(reg.register(key, late, t0), None, "sala llena");

        // Pasado el TTL sin renovar, los antiguos desaparecen.
        let later = t0 + REGISTRATION_TTL + Duration::from_secs(1);
        assert_eq!(reg.register(key, late, later), Some(vec![]));
    }

    #[test]
    fn el_tope_de_salas_no_lo_bloquea_lo_caducado() {
        let reg = Registry::new(1);
        let t0 = Instant::now();
        let a = addr("203.0.113.1:1");
        assert!(reg.register(RoomKey::from_token(b"a"), a, t0).is_some());
        assert!(reg.register(RoomKey::from_token(b"b"), a, t0).is_none());
        let later = t0 + REGISTRATION_TTL + Duration::from_secs(1);
        assert!(reg.register(RoomKey::from_token(b"b"), a, later).is_some());
        assert_eq!(reg.rooms(), 1);
    }

    #[test]
    fn el_servidor_solo_contesta_a_un_register() {
        let reg = Registry::new(8);
        let key = RoomKey::from_token(b"sala");
        let from = addr("203.0.113.1:1000");
        assert!(reg
            .handle(&Message::Punch { nonce: [0; 8] }.encode(key), from)
            .is_none());
        assert!(reg.handle(b"basura", from).is_none());
        let reply = reg.handle(&Message::Register.encode(key), from).unwrap();
        assert_eq!(
            Message::decode(&reply),
            Some((
                key,
                Message::Peers {
                    you: from,
                    others: vec![]
                }
            ))
        );
    }
}
