//! Handshake Noise (feature `noise`).
//!
//! Implementa el patrón `NN` de Noise (con `psko0` opcional para autenticar
//! salas con token) como reemplazo del handshake X25519 custom:
//!
//! ```text
//! Cliente                                  Servidor
//!   │── NoiseHello1 (0x05) ───────────────►│  e   (+ NoiseHello1 en el payload)
//!   │◄── NoiseHello2 (0x06) ───────────────│  e, ee  (+ NoiseHello2 cifrado)
//!   │  [handshake_hash → HKDF → encrypt_key, decrypt_key]
//!   │── KeyExchange  (0x04, v2 labels) ────►│  auth_tag cliente
//!   │◄── SessionConfirm (0x03, v2 labels) ─│  auth_tag servidor
//! ```
//!
//! El transporte de audio sigue usando el AEAD existente
//! (ChaCha20-Poly1305 con nonce explícito por `sequence` y header como AAD),
//! pero con claves derivadas del `handshake_hash` de Noise. Esto mantiene la
//! ventana anti-replay y el jitter buffer sin cambios, algo que el
//! `TransportState` de Noise no soporta bien sobre UDP (no acepta reorden).

use snow::{Builder, HandshakeState};

/// Patrón Noise sin autenticación de sala.
pub const PATTERN_NN: &str = "Noise_NN_25519_ChaChaPoly_BLAKE2s";
/// Patrón Noise con PSK en la primera posición (token de sala).
pub const PATTERN_NN_PSK0: &str = "Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s";

/// Tamaño máximo de un mensaje Noise de handshake (e + payload + tag).
const MAX_NOISE_MSG: usize = 1024;

#[derive(Debug)]
pub enum NoiseError {
    /// El patrón no se pudo construir (configuración inválida).
    Build(String),
    /// Error interno de snow (mensaje inválido, MAC, etc.).
    Snow(snow::Error),
    /// El handshake no estaba completo al pedir el hash.
    NotComplete,
}

impl core::fmt::Display for NoiseError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Build(e) => write!(f, "noise build: {e}"),
            Self::Snow(e) => write!(f, "noise: {e}"),
            Self::NotComplete => write!(f, "noise handshake not complete"),
        }
    }
}

impl std::error::Error for NoiseError {}

impl From<snow::Error> for NoiseError {
    fn from(e: snow::Error) -> Self {
        Self::Snow(e)
    }
}

/// Estado del handshake Noise de una sesión.
pub struct NoiseHandshake {
    state: HandshakeState,
}

impl core::fmt::Debug for NoiseHandshake {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("NoiseHandshake")
            .field("finished", &self.is_finished())
            .finish()
    }
}

impl NoiseHandshake {
    fn new(psk: Option<&[u8; 32]>, initiator: bool) -> Result<Self, NoiseError> {
        let pattern = if psk.is_some() {
            PATTERN_NN_PSK0
        } else {
            PATTERN_NN
        };
        let params = pattern
            .parse()
            .map_err(|e| NoiseError::Build(format!("{e:?}")))?;
        let mut builder = Builder::new(params);
        if let Some(key) = psk {
            builder = builder.psk(0, key);
        }
        let state = if initiator {
            builder.build_initiator()?
        } else {
            builder.build_responder()?
        };
        Ok(Self { state })
    }

    /// Construye el lado iniciador (cliente).
    pub fn new_initiator(psk: Option<&[u8; 32]>) -> Result<Self, NoiseError> {
        Self::new(psk, true)
    }

    /// Construye el lado respondedor (servidor).
    pub fn new_responder(psk: Option<&[u8; 32]>) -> Result<Self, NoiseError> {
        Self::new(psk, false)
    }

    /// Escribe el próximo mensaje del handshake con `payload` como datos de
    /// aplicación (cifrados cuando el patrón ya derivó clave).
    pub fn write(&mut self, payload: &[u8]) -> Result<Vec<u8>, NoiseError> {
        let mut out = vec![0u8; MAX_NOISE_MSG];
        let n = self.state.write_message(payload, &mut out)?;
        out.truncate(n);
        Ok(out)
    }

    /// Lee un mensaje del peer y devuelve su payload de aplicación.
    pub fn read(&mut self, message: &[u8]) -> Result<Vec<u8>, NoiseError> {
        let mut out = vec![0u8; MAX_NOISE_MSG];
        let n = self.state.read_message(message, &mut out)?;
        out.truncate(n);
        Ok(out)
    }

    /// `true` cuando el handshake terminó (listo para derivar claves).
    pub fn is_finished(&self) -> bool {
        self.state.is_handshake_finished()
    }

    /// Hash del handshake (cubre todos los mensajes del patrón).
    pub fn handshake_hash(&self) -> Result<[u8; 32], NoiseError> {
        let hash = self.state.get_handshake_hash();
        if hash.len() != 32 || !self.is_finished() {
            return Err(NoiseError::NotComplete);
        }
        let mut out = [0u8; 32];
        out.copy_from_slice(hash);
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nn_handshake_roundtrip() {
        let mut client = NoiseHandshake::new_initiator(None).unwrap();
        let mut server = NoiseHandshake::new_responder(None).unwrap();

        let msg1 = client.write(b"hello1").unwrap();
        let payload1 = server.read(&msg1).unwrap();
        assert_eq!(payload1, b"hello1");

        let msg2 = server.write(b"hello2").unwrap();
        let payload2 = client.read(&msg2).unwrap();
        assert_eq!(payload2, b"hello2");

        assert!(client.is_finished());
        assert!(server.is_finished());
        assert_eq!(
            client.handshake_hash().unwrap(),
            server.handshake_hash().unwrap(),
            "ambos extremos deben coincidir en el hash"
        );
    }

    #[test]
    fn nnpsk0_authenticates_token() {
        let psk_a = [7u8; 32];
        let psk_b = [9u8; 32];

        let mut client = NoiseHandshake::new_initiator(Some(&psk_a)).unwrap();
        let mut wrong_server = NoiseHandshake::new_responder(Some(&psk_b)).unwrap();

        let msg1 = client.write(b"").unwrap();
        assert!(
            wrong_server.read(&msg1).is_err(),
            "PSK distinta debe fallar el handshake"
        );

        let mut right_server = NoiseHandshake::new_responder(Some(&psk_a)).unwrap();
        right_server.read(&msg1).unwrap();
        let msg2 = right_server.write(b"").unwrap();
        client.read(&msg2).unwrap();
        assert_eq!(
            client.handshake_hash().unwrap(),
            right_server.handshake_hash().unwrap()
        );
    }

    #[test]
    fn tampered_message_fails() {
        let mut client = NoiseHandshake::new_initiator(None).unwrap();
        let mut server = NoiseHandshake::new_responder(None).unwrap();

        // msg1 lleva la clave efímera (sin MAC): el manipulador no se detecta
        // hasta que el cliente procesa msg2, cuyo MAC depende de ee.
        let mut msg1 = client.write(b"").unwrap();
        msg1[0] ^= 0xFF;
        server.read(&msg1).expect("clave efímera aún válida");

        let msg2 = server.write(b"").unwrap();
        assert!(
            client.read(&msg2).is_err(),
            "el cliente debe rechazar msg2 por MAC inválido"
        );
    }
}
