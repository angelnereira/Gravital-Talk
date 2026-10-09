//! Agenda de direcciones por sesión.
//!
//! Una sesión P2P 1:1 tiene un solo peer, pero el modo sala enruta por relay y
//! varios participantes comparten `session_id`. El `Session` guardaba un único
//! `peer: Option<SocketAddr>` y descartaba todo lo que no viniera de esa
//! dirección, lo que impedía el audio N-a-N.
//!
//! Esta agenda sustituye a ese campo único con un conjunto de peers
//! conocidos. En P2P sigue habiendo uno solo, así que el comportamiento 1:1 no
//! cambia; en sala, cada handshake válido da de alta a un peer adicional.
//!
//! El orden importa para el floor control: el primer peer registrado es el que
//! arbitra (el host de la sala), y los siguientes se numeran en orden de
//! llegada, de forma estable y determinista para las métricas.

use std::net::SocketAddr;

use std::collections::VecDeque;

/// Peers conocidos de una sesión, en orden de registro.
///
/// No usa `HashSet` a propósito: el orden de inserción es parte del contrato
/// (índice 0 = peer principal) y un hash no lo garantiza.
#[derive(Debug, Default)]
pub struct AddressBook {
    peers: VecDeque<SocketAddr>,
}

/// Máximo de peers por sesión en el cliente.
///
/// El relay permite 50 por defecto (`max_peers_per_session`). El cliente usa un
/// tope propio para no crecer sin límite si un atacante reinyecta handshakes
/// desde direcciones distintas.
pub const MAX_PEERS: usize = 50;

impl AddressBook {
    pub const fn new() -> Self {
        Self {
            peers: VecDeque::new(),
        }
    }

    /// Registra `addr` si es nueva. Devuelve `true` si se añadió.
    pub fn insert(&mut self, addr: SocketAddr) -> bool {
        if self.peers.contains(&addr) {
            return false;
        }
        if self.peers.len() >= MAX_PEERS {
            return false;
        }
        self.peers.push_back(addr);
        true
    }

    /// Marca `addr` como visto sin añadirla (para un peer conocido).
    pub fn touch(&mut self, addr: SocketAddr) {
        if !self.peers.contains(&addr) && self.peers.len() < MAX_PEERS {
            self.peers.push_back(addr);
        }
    }

    /// `true` si la dirección ya está registrada.
    #[must_use]
    pub fn contains(&self, addr: SocketAddr) -> bool {
        self.peers.contains(&addr)
    }

    /// `true` si no hay ningún peer registrado.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.peers.len()
    }

    /// Peer principal: el primero registrado (el host en modo sala).
    #[must_use]
    pub fn primary(&self) -> Option<SocketAddr> {
        self.peers.front().copied()
    }

    /// Todos los peers en orden de registro.
    #[must_use]
    pub fn all(&self) -> Vec<SocketAddr> {
        self.peers.iter().copied().collect()
    }

    /// Destinos de un envío originado en `from` (excluye al emisor).
    #[must_use]
    pub fn others(&self, from: SocketAddr) -> Vec<SocketAddr> {
        self.peers.iter().copied().filter(|p| *p != from).collect()
    }

    /// Quita un peer (al desconectarse o perder la ruta).
    pub fn remove(&mut self, addr: SocketAddr) -> bool {
        let before = self.peers.len();
        self.peers.retain(|p| *p != addr);
        self.peers.len() != before
    }

    /// Vacía la agenda (al cerrar la sesión).
    pub fn clear(&mut self) {
        self.peers.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    #[test]
    fn p2p_keeps_a_single_peer() {
        let mut book = AddressBook::new();
        assert!(book.is_empty());
        assert_eq!(book.primary(), None);

        assert!(book.insert(a(9000)));
        assert!(!book.insert(a(9000)), "no debe duplicar el mismo peer");
        assert_eq!(book.len(), 1);
        assert_eq!(book.primary(), Some(a(9000)));
    }

    #[test]
    fn room_accumulates_peers_in_registration_order() {
        let mut book = AddressBook::new();
        assert!(book.insert(a(9000))); // host primero
        assert!(book.insert(a(9001)));
        assert!(book.insert(a(9002)));
        assert_eq!(book.len(), 3);
        // El host sigue siendo el principal aunque entren mas peers.
        assert_eq!(book.primary(), Some(a(9000)));
        assert_eq!(book.all(), vec![a(9000), a(9001), a(9002)]);
    }

    #[test]
    fn others_excludes_the_sender() {
        let mut book = AddressBook::new();
        book.insert(a(9000));
        book.insert(a(9001));
        book.insert(a(9002));

        assert_eq!(book.others(a(9001)), vec![a(9000), a(9002)]);
        // Un emisor desconocido no filtra nada.
        assert_eq!(book.others(a(9999)).len(), 3);
    }

    #[test]
    fn respects_the_peer_ceiling() {
        let mut book = AddressBook::new();
        for i in 0..MAX_PEERS {
            assert!(book.insert(a(10_000 + i as u16)), "peer {i} debe entrar");
        }
        assert_eq!(book.len(), MAX_PEERS);
        assert!(!book.insert(a(60_000)), "no debe superar el tope de peers");
        assert_eq!(book.len(), MAX_PEERS);
    }

    #[test]
    fn remove_and_clear_preserve_primary_order() {
        let mut book = AddressBook::new();
        book.insert(a(9000));
        book.insert(a(9001));
        book.insert(a(9002));

        assert!(book.remove(a(9001)));
        assert!(!book.remove(a(9001)), "eliminar dos veces no debe fallar");
        assert_eq!(book.all(), vec![a(9000), a(9002)]);

        book.clear();
        assert!(book.is_empty());
        assert_eq!(book.primary(), None);
    }

    #[test]
    fn touch_is_idempotent_and_respects_the_ceiling() {
        let mut book = AddressBook::new();
        book.touch(a(9000));
        book.touch(a(9000));
        assert_eq!(book.len(), 1);

        for i in 0..MAX_PEERS {
            book.touch(a(20_000 + i as u16));
        }
        book.touch(a(60_000));
        assert_eq!(book.len(), MAX_PEERS, "touch tambien respeta el tope");
    }
}
