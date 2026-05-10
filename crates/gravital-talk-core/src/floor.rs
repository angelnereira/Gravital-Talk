//! Floor Controller — máquina de estados PTT.
//!
//! En PTT, el "floor" es el permiso para transmitir. Solo un participante
//! puede tener el floor en un momento dado. Este módulo implementa:
//!
//! - FSM del floor: IDLE → REQUESTED → GRANTED → RELEASED
//! - Payloads binarios de los mensajes de floor control
//! - Timeout de transmisión máxima (por defecto 30 s, estándar 3GPP MCX)
//!
//! ## Flujo normal de dos participantes
//!
//! ```text
//! Peer A                    Árbitro/Peer B
//!   │── FloorRequest ───────►│
//!   │◄── FloorGrant ─────────│
//!   │  [transmite audio]
//!   │── FloorRelease ─────────►│
//!   │◄── (floor libre) ──────│
//! ```
//!
//! ## Flujo con denegación (otro peer ya tiene el floor)
//!
//! ```text
//! Peer A                    Árbitro
//!   │── FloorRequest ───────►│  (Peer B ya tiene el floor)
//!   │◄── FloorDeny ──────────│
//! ```

use crate::error::Error;

/// Duración máxima de transmisión sin liberación explícita (ms). Estándar 3GPP MCX.
pub const FLOOR_TIMEOUT_MS: u64 = 30_000;

/// Estado del floor controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloorState {
    /// Nadie tiene el floor.
    Idle,
    /// Se ha enviado/recibido una solicitud; esperando respuesta del árbitro.
    Requested,
    /// El floor ha sido concedido a un participante.
    Granted,
    /// El floor fue liberado; transitorio antes de volver a Idle.
    Released,
}

impl core::fmt::Display for FloorState {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Idle => write!(f, "idle"),
            Self::Requested => write!(f, "requested"),
            Self::Granted => write!(f, "granted"),
            Self::Released => write!(f, "released"),
        }
    }
}

/// Eventos que disparan transiciones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloorEvent {
    /// El usuario local solicita el floor.
    Request,
    /// El árbitro concede el floor (puede ser al usuario local o a otro).
    Grant,
    /// El árbitro niega la solicitud.
    Deny,
    /// El usuario local o remoto libera el floor.
    Release,
    /// Otro participante tomó el floor mientras el local estaba en disputa.
    Taken,
    /// Timeout de transmisión máxima superado.
    Timeout,
    /// Reset forzado (reconexión, pérdida de señal).
    Reset,
}

/// Error de transición del floor controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloorTransitionError {
    pub from: FloorState,
    pub event: FloorEvent,
}

impl core::fmt::Display for FloorTransitionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(
            f,
            "invalid floor transition {:?} + {:?}",
            self.from, self.event
        )
    }
}

/// Máquina de estados del floor controller.
///
/// Diseñada para operar tanto en el cliente (modo optimista: asume que el
/// grant llegará) como en el servidor/árbitro (mode autoritativo: decide
/// quién recibe el floor y notifica a todos los participantes).
#[derive(Debug, Clone, Copy)]
pub struct FloorController {
    state: FloorState,
    /// SSRC del participante que actualmente tiene el floor (0 = nadie).
    holder_ssrc: u32,
    /// Timeout de transmisión máxima en ms.
    max_transmit_ms: u64,
}

impl Default for FloorController {
    fn default() -> Self {
        Self::new(FLOOR_TIMEOUT_MS)
    }
}

impl FloorController {
    #[must_use]
    pub const fn new(max_transmit_ms: u64) -> Self {
        Self {
            state: FloorState::Idle,
            holder_ssrc: 0,
            max_transmit_ms,
        }
    }

    #[must_use]
    pub const fn state(&self) -> FloorState {
        self.state
    }

    /// SSRC del participante con el floor (0 si nadie lo tiene).
    #[must_use]
    pub const fn holder(&self) -> u32 {
        self.holder_ssrc
    }

    #[must_use]
    pub const fn max_transmit_ms(&self) -> u64 {
        self.max_transmit_ms
    }

    /// `true` si el floor está disponible para solicitar.
    #[must_use]
    pub const fn is_available(&self) -> bool {
        matches!(self.state, FloorState::Idle | FloorState::Released)
    }

    /// `true` si alguien tiene el floor en este momento.
    #[must_use]
    pub const fn is_granted(&self) -> bool {
        matches!(self.state, FloorState::Granted)
    }

    /// `true` si el participante con `ssrc` tiene el floor.
    #[must_use]
    pub fn is_holder(&self, ssrc: u32) -> bool {
        self.state == FloorState::Granted && self.holder_ssrc == ssrc
    }

    /// Aplica un evento. Si la transición es inválida el estado no cambia
    /// y se devuelve `Err`. `ssrc` es el SSRC del participante al que aplica
    /// el evento (0 para eventos sin sujeto como Timeout/Reset).
    pub fn transition(
        &mut self,
        event: FloorEvent,
        ssrc: u32,
    ) -> Result<FloorState, FloorTransitionError> {
        let next = match (self.state, event) {
            // ── Desde Idle ────────────────────────────────────────────────────
            (FloorState::Idle, FloorEvent::Request) => {
                self.holder_ssrc = ssrc;
                FloorState::Requested
            }
            (FloorState::Idle, FloorEvent::Reset) => FloorState::Idle,
            // Grant directo sin Request previo (árbitro autoritativo puede emitir esto).
            (FloorState::Idle, FloorEvent::Grant) => {
                self.holder_ssrc = ssrc;
                FloorState::Granted
            }

            // ── Desde Requested ───────────────────────────────────────────────
            (FloorState::Requested, FloorEvent::Grant) => {
                self.holder_ssrc = ssrc;
                FloorState::Granted
            }
            (FloorState::Requested, FloorEvent::Deny) => {
                self.holder_ssrc = 0;
                FloorState::Idle
            }
            (FloorState::Requested, FloorEvent::Taken) => {
                // Otro peer fue concedido primero.
                self.holder_ssrc = ssrc;
                FloorState::Granted
            }
            (FloorState::Requested, FloorEvent::Reset)
            | (FloorState::Requested, FloorEvent::Timeout) => {
                self.holder_ssrc = 0;
                FloorState::Idle
            }

            // ── Desde Granted ─────────────────────────────────────────────────
            (FloorState::Granted, FloorEvent::Release) => {
                self.holder_ssrc = 0;
                FloorState::Released
            }
            (FloorState::Granted, FloorEvent::Timeout) => {
                self.holder_ssrc = 0;
                FloorState::Released
            }
            (FloorState::Granted, FloorEvent::Taken) => {
                // El árbitro asignó el floor a otro participante.
                self.holder_ssrc = ssrc;
                FloorState::Granted
            }
            (FloorState::Granted, FloorEvent::Reset) => {
                self.holder_ssrc = 0;
                FloorState::Idle
            }

            // ── Desde Released ────────────────────────────────────────────────
            (FloorState::Released, FloorEvent::Reset)
            | (FloorState::Released, FloorEvent::Release) => {
                self.holder_ssrc = 0;
                FloorState::Idle
            }
            (FloorState::Released, FloorEvent::Request) => {
                self.holder_ssrc = ssrc;
                FloorState::Requested
            }
            (FloorState::Released, FloorEvent::Grant) => {
                self.holder_ssrc = ssrc;
                FloorState::Granted
            }

            (from, event) => {
                return Err(FloorTransitionError { from, event });
            }
        };
        self.state = next;
        Ok(next)
    }
}

// ── Payload binario ──────────────────────────────────────────────────────────

/// Payload compartido para FloorRequest / FloorGrant / FloorDeny /
/// FloorRelease / FloorTaken.
///
/// ```text
/// ssrc [4 bytes, big-endian] — SSRC del participante al que aplica el mensaje
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloorPayload {
    pub ssrc: u32,
}

impl FloorPayload {
    pub const SIZE: usize = 4;

    pub fn encode(&self, buf: &mut [u8]) -> Result<(), Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::BufferTooSmall);
        }
        buf[0..4].copy_from_slice(&self.ssrc.to_be_bytes());
        Ok(())
    }

    pub fn decode(buf: &[u8]) -> Result<Self, Error> {
        if buf.len() < Self::SIZE {
            return Err(Error::MalformedPayload);
        }
        Ok(Self {
            ssrc: u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_two_party_ptt() {
        let mut fc = FloorController::default();
        assert_eq!(fc.state(), FloorState::Idle);
        assert!(fc.is_available());

        fc.transition(FloorEvent::Request, 0x1111).unwrap();
        assert_eq!(fc.state(), FloorState::Requested);

        fc.transition(FloorEvent::Grant, 0x1111).unwrap();
        assert_eq!(fc.state(), FloorState::Granted);
        assert_eq!(fc.holder(), 0x1111);
        assert!(fc.is_holder(0x1111));
        assert!(!fc.is_available());

        fc.transition(FloorEvent::Release, 0x1111).unwrap();
        assert_eq!(fc.state(), FloorState::Released);
        assert_eq!(fc.holder(), 0);
    }

    #[test]
    fn denial_returns_to_idle() {
        let mut fc = FloorController::default();
        fc.transition(FloorEvent::Request, 0xAAAA).unwrap();
        fc.transition(FloorEvent::Deny, 0xAAAA).unwrap();
        assert_eq!(fc.state(), FloorState::Idle);
        assert_eq!(fc.holder(), 0);
    }

    #[test]
    fn timeout_releases_from_granted() {
        let mut fc = FloorController::default();
        fc.transition(FloorEvent::Request, 0x1).unwrap();
        fc.transition(FloorEvent::Grant, 0x1).unwrap();
        fc.transition(FloorEvent::Timeout, 0).unwrap();
        assert_eq!(fc.state(), FloorState::Released);
        assert_eq!(fc.holder(), 0);
    }

    #[test]
    fn taken_changes_holder_while_granted() {
        let mut fc = FloorController::default();
        fc.transition(FloorEvent::Request, 0x1).unwrap();
        fc.transition(FloorEvent::Grant, 0x1).unwrap();
        // Árbitro reasigna a peer 0x2
        fc.transition(FloorEvent::Taken, 0x2).unwrap();
        assert_eq!(fc.state(), FloorState::Granted);
        assert_eq!(fc.holder(), 0x2);
    }

    #[test]
    fn taken_during_request_becomes_granted() {
        let mut fc = FloorController::default();
        fc.transition(FloorEvent::Request, 0x1).unwrap();
        fc.transition(FloorEvent::Taken, 0x2).unwrap();
        assert_eq!(fc.state(), FloorState::Granted);
        assert_eq!(fc.holder(), 0x2);
    }

    #[test]
    fn reset_from_any_state_goes_to_idle() {
        for setup_events in [
            vec![],
            vec![(FloorEvent::Request, 0x1u32)],
            vec![(FloorEvent::Request, 0x1), (FloorEvent::Grant, 0x1)],
        ] {
            let mut fc = FloorController::default();
            for (ev, ssrc) in setup_events {
                fc.transition(ev, ssrc).unwrap();
            }
            fc.transition(FloorEvent::Reset, 0).unwrap();
            assert_eq!(fc.state(), FloorState::Idle);
            assert_eq!(fc.holder(), 0);
        }
    }

    #[test]
    fn invalid_transition_preserved() {
        let mut fc = FloorController::default();
        // Idle + Release = inválido
        let err = fc.transition(FloorEvent::Release, 0);
        assert!(err.is_err());
        assert_eq!(fc.state(), FloorState::Idle);
    }

    #[test]
    fn released_can_go_directly_to_granted() {
        let mut fc = FloorController::default();
        fc.transition(FloorEvent::Request, 0x1).unwrap();
        fc.transition(FloorEvent::Grant, 0x1).unwrap();
        fc.transition(FloorEvent::Release, 0x1).unwrap();
        assert_eq!(fc.state(), FloorState::Released);
        // Peer 2 toma el floor inmediatamente
        fc.transition(FloorEvent::Grant, 0x2).unwrap();
        assert_eq!(fc.state(), FloorState::Granted);
        assert_eq!(fc.holder(), 0x2);
    }

    #[test]
    fn floor_payload_roundtrip() {
        let p = FloorPayload { ssrc: 0xDEAD_BEEF };
        let mut buf = [0u8; FloorPayload::SIZE];
        p.encode(&mut buf).unwrap();
        assert_eq!(FloorPayload::decode(&buf).unwrap(), p);
    }

    #[test]
    fn floor_payload_zero_ssrc() {
        let p = FloorPayload { ssrc: 0 };
        let mut buf = [0u8; FloorPayload::SIZE];
        p.encode(&mut buf).unwrap();
        assert_eq!(FloorPayload::decode(&buf).unwrap().ssrc, 0);
    }

    #[test]
    fn floor_payload_too_short() {
        assert!(FloorPayload::decode(&[]).is_err());
        assert!(FloorPayload::decode(&[0; 3]).is_err());
    }

    #[test]
    fn display_states() {
        assert_eq!(FloorState::Idle.to_string(), "idle");
        assert_eq!(FloorState::Granted.to_string(), "granted");
        assert_eq!(FloorState::Released.to_string(), "released");
    }

    #[test]
    fn default_max_transmit_is_30s() {
        let fc = FloorController::default();
        assert_eq!(fc.max_transmit_ms(), FLOOR_TIMEOUT_MS);
        assert_eq!(fc.max_transmit_ms(), 30_000);
    }
}
