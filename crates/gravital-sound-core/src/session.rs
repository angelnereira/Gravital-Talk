//! Máquina de estados de sesión.
//!
//! Tres estrategias conviven en este módulo:
//!
//! 1. Un `enum SessionState` para representación dinámica (usada en FFI,
//!    logging, métricas).
//! 2. Una `SessionStateMachine` imperativa con método `transition` que
//!    valida en runtime.
//! 3. Tipos marcador (phantom types) listados en `phantom` para construir
//!    APIs type-safe desde los crates superiores (por ejemplo,
//!    `Session<Active>::send_audio` no existe en otros estados).

use crate::error::Error;

/// Identificador numérico de sesión.
pub type SessionId = u32;

/// Estado dinámico de una sesión.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Idle,
    Handshaking,
    Active,
    Paused,
    Closing,
    Closed,
}

impl SessionState {
    /// Código numérico estable para la FFI.
    #[must_use]
    pub const fn code(self) -> u8 {
        match self {
            Self::Idle => 0,
            Self::Handshaking => 1,
            Self::Active => 2,
            Self::Paused => 3,
            Self::Closing => 4,
            Self::Closed => 5,
        }
    }

    pub const fn from_code(code: u8) -> Result<Self, Error> {
        Ok(match code {
            0 => Self::Idle,
            1 => Self::Handshaking,
            2 => Self::Active,
            3 => Self::Paused,
            4 => Self::Closing,
            5 => Self::Closed,
            _ => return Err(Error::MalformedPayload),
        })
    }
}

/// Eventos que disparan transiciones de estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEvent {
    StartConnect,
    StartAccept,
    HandshakeOk,
    HandshakeTimeout,
    Pause,
    Resume,
    Close,
    PeerClosed,
    PeerTimeout,
}

/// Error de transición. Diferente de `Error::InvalidStateTransition` para
/// llevar información del par estado/evento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateTransitionError {
    pub from: SessionState,
    pub event: SessionEvent,
}

/// Máquina de estados imperativa.
#[derive(Debug, Clone, Copy)]
pub struct SessionStateMachine {
    state: SessionState,
}

impl Default for SessionStateMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionStateMachine {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: SessionState::Idle,
        }
    }

    #[inline]
    #[must_use]
    pub const fn state(&self) -> SessionState {
        self.state
    }

    /// Aplica un evento. Si la transición es inválida, el estado no cambia
    /// y se devuelve `Err`.
    pub fn transition(
        &mut self,
        event: SessionEvent,
    ) -> Result<SessionState, StateTransitionError> {
        let next = match (self.state, event) {
            (SessionState::Idle, SessionEvent::StartConnect)
            | (SessionState::Idle, SessionEvent::StartAccept) => SessionState::Handshaking,

            (SessionState::Handshaking, SessionEvent::HandshakeOk) => SessionState::Active,
            (SessionState::Handshaking, SessionEvent::HandshakeTimeout)
            | (SessionState::Handshaking, SessionEvent::Close) => SessionState::Closed,

            (SessionState::Active, SessionEvent::Pause) => SessionState::Paused,
            (SessionState::Active, SessionEvent::Close) => SessionState::Closing,
            (SessionState::Active, SessionEvent::PeerClosed) => SessionState::Closing,
            (SessionState::Active, SessionEvent::PeerTimeout) => SessionState::Closing,

            (SessionState::Paused, SessionEvent::Resume) => SessionState::Active,
            (SessionState::Paused, SessionEvent::Close) => SessionState::Closing,
            (SessionState::Paused, SessionEvent::PeerClosed) => SessionState::Closing,
            (SessionState::Paused, SessionEvent::PeerTimeout) => SessionState::Closing,

            (SessionState::Closing, SessionEvent::PeerClosed) => SessionState::Closed,
            (SessionState::Closing, SessionEvent::Close) => SessionState::Closed,

            (from, event) => return Err(StateTransitionError { from, event }),
        };
        self.state = next;
        Ok(next)
    }
}

/// Tipos marcador para APIs type-safe desde otros crates.
///
/// Ejemplo de uso:
///
/// ```ignore
/// pub struct Session<S> { marker: core::marker::PhantomData<S>, ... }
/// impl Session<phantom::Active> { pub fn send_audio(...) { ... } }
/// impl Session<phantom::Idle>   { pub fn connect(...) -> Session<phantom::Handshaking> {...} }
/// ```
pub mod phantom {
    /// Sesión en estado `Idle`.
    #[derive(Debug)]
    pub struct Idle;
    /// Sesión en estado `Handshaking`.
    #[derive(Debug)]
    pub struct Handshaking;
    /// Sesión en estado `Active`.
    #[derive(Debug)]
    pub struct Active;
    /// Sesión en estado `Paused`.
    #[derive(Debug)]
    pub struct Paused;
    /// Sesión en estado `Closing`.
    #[derive(Debug)]
    pub struct Closing;
    /// Sesión en estado `Closed`.
    #[derive(Debug)]
    pub struct Closed;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_client() {
        let mut sm = SessionStateMachine::new();
        assert_eq!(sm.state(), SessionState::Idle);
        sm.transition(SessionEvent::StartConnect).unwrap();
        assert_eq!(sm.state(), SessionState::Handshaking);
        sm.transition(SessionEvent::HandshakeOk).unwrap();
        assert_eq!(sm.state(), SessionState::Active);
        sm.transition(SessionEvent::Close).unwrap();
        assert_eq!(sm.state(), SessionState::Closing);
        sm.transition(SessionEvent::PeerClosed).unwrap();
        assert_eq!(sm.state(), SessionState::Closed);
    }

    #[test]
    fn pause_resume_cycle() {
        let mut sm = SessionStateMachine::new();
        sm.transition(SessionEvent::StartAccept).unwrap();
        sm.transition(SessionEvent::HandshakeOk).unwrap();
        sm.transition(SessionEvent::Pause).unwrap();
        assert_eq!(sm.state(), SessionState::Paused);
        sm.transition(SessionEvent::Resume).unwrap();
        assert_eq!(sm.state(), SessionState::Active);
    }

    #[test]
    fn invalid_transition_preserves_state() {
        let mut sm = SessionStateMachine::new();
        // No se puede `Resume` desde Idle.
        let err = sm.transition(SessionEvent::Resume).unwrap_err();
        assert_eq!(err.from, SessionState::Idle);
        assert_eq!(err.event, SessionEvent::Resume);
        assert_eq!(sm.state(), SessionState::Idle);
    }

    #[test]
    fn handshake_timeout_to_closed() {
        let mut sm = SessionStateMachine::new();
        sm.transition(SessionEvent::StartConnect).unwrap();
        sm.transition(SessionEvent::HandshakeTimeout).unwrap();
        assert_eq!(sm.state(), SessionState::Closed);
    }

    #[test]
    fn peer_timeout_from_active() {
        let mut sm = SessionStateMachine::new();
        sm.transition(SessionEvent::StartAccept).unwrap();
        sm.transition(SessionEvent::HandshakeOk).unwrap();
        sm.transition(SessionEvent::PeerTimeout).unwrap();
        assert_eq!(sm.state(), SessionState::Closing);
    }

    #[test]
    fn state_code_roundtrip() {
        for s in [
            SessionState::Idle,
            SessionState::Handshaking,
            SessionState::Active,
            SessionState::Paused,
            SessionState::Closing,
            SessionState::Closed,
        ] {
            assert_eq!(SessionState::from_code(s.code()).unwrap(), s);
        }
    }
}
