//! Identidad basada en Ed25519 para autenticación challenge-response.
//!
//! Cada nodo Gravital Talk tiene una identidad permanente representada como
//! un par de llaves Ed25519:
//!
//! - La **llave pública** es el identificador permanente del nodo (análogo
//!   al número de radio en TETRA).
//! - La **llave privada** nunca abandona el dispositivo. Nunca se transmite.
//!
//! ## Autenticación
//!
//! ```text
//! Servidor                              Cliente
//!   │── AuthChallenge (nonce 32B) ────►│
//!   │◄── AuthResponse (pubkey+sig) ────│  sig = Ed25519_sign(privkey, nonce)
//!   │  [verifica: Ed25519_verify(pubkey, nonce, sig)]
//!   │── AuthAccepted / AuthRejected ──►│
//! ```
//!
//! No se almacenan contraseñas. No hay sesiones persistentes en el servidor.
//!
//! ## Nota sobre `no_std`
//!
//! Este módulo requiere la feature `identity` del crate, que habilita
//! `ed25519-dalek`. En entornos `no_std` con `alloc` funciona correctamente.

/// Payload de AuthChallenge enviado por el servidor al cliente (32 bytes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AuthChallengePayload {
    pub nonce: [u8; 32],
}

impl AuthChallengePayload {
    pub const SIZE: usize = 32;

    pub fn encode(&self, buf: &mut [u8]) -> Result<(), crate::error::Error> {
        if buf.len() < Self::SIZE {
            return Err(crate::error::Error::BufferTooSmall);
        }
        buf[0..32].copy_from_slice(&self.nonce);
        Ok(())
    }

    pub fn decode(buf: &[u8]) -> Result<Self, crate::error::Error> {
        if buf.len() < Self::SIZE {
            return Err(crate::error::Error::MalformedPayload);
        }
        let mut nonce = [0u8; 32];
        nonce.copy_from_slice(&buf[0..32]);
        Ok(Self { nonce })
    }
}

/// Payload de AuthResponse enviado por el cliente al servidor (96 bytes).
///
/// ```text
/// public_key [32] — llave pública Ed25519 del cliente
/// signature  [64] — Ed25519_sign(private_key, nonce) donde nonce viene de AuthChallenge
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthResponsePayload {
    pub public_key: [u8; 32],
    pub signature: [u8; 64],
}

impl AuthResponsePayload {
    pub const SIZE: usize = 96;

    pub fn encode(&self, buf: &mut [u8]) -> Result<(), crate::error::Error> {
        if buf.len() < Self::SIZE {
            return Err(crate::error::Error::BufferTooSmall);
        }
        buf[0..32].copy_from_slice(&self.public_key);
        buf[32..96].copy_from_slice(&self.signature);
        Ok(())
    }

    pub fn decode(buf: &[u8]) -> Result<Self, crate::error::Error> {
        if buf.len() < Self::SIZE {
            return Err(crate::error::Error::MalformedPayload);
        }
        let mut public_key = [0u8; 32];
        let mut signature = [0u8; 64];
        public_key.copy_from_slice(&buf[0..32]);
        signature.copy_from_slice(&buf[32..96]);
        Ok(Self { public_key, signature })
    }
}

/// Llave pública de identidad Ed25519 (identificador permanente del nodo).
///
/// Se usa para verificar firmas del challenge-response de autenticación.
/// Se puede compartir libremente; es el "número de radio" del participante.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityPublic {
    pub bytes: [u8; 32],
}

impl IdentityPublic {
    /// Verifica una firma Ed25519 sobre un mensaje.
    ///
    /// Devuelve `true` si la firma es válida para esta llave pública.
    /// Implementado con `ed25519-dalek` cuando la feature `identity` está activa.
    #[cfg(feature = "identity")]
    pub fn verify(&self, message: &[u8], signature_bytes: &[u8; 64]) -> bool {
        use ed25519_dalek::{Signature, Verifier, VerifyingKey};
        let Ok(vk) = VerifyingKey::from_bytes(&self.bytes) else {
            return false;
        };
        let Ok(sig) = Signature::from_bytes(signature_bytes) else {
            return false;
        };
        vk.verify(message, &sig).is_ok()
    }

    /// Versión sin criptografía real (solo compila en entornos sin la feature).
    /// Siempre devuelve `false` para evitar falsos positivos.
    #[cfg(not(feature = "identity"))]
    pub fn verify(&self, _message: &[u8], _signature_bytes: &[u8; 64]) -> bool {
        false
    }
}

/// Identidad completa (par de llaves Ed25519).
///
/// La llave privada nunca debe salir del dispositivo. Úsala solo para firmar
/// challenges del servidor.
#[cfg(feature = "identity")]
pub struct Identity {
    signing_key: ed25519_dalek::SigningKey,
}

#[cfg(feature = "identity")]
impl Identity {
    /// Genera una nueva identidad aleatoria.
    pub fn generate() -> Self {
        use ed25519_dalek::SigningKey;
        use rand_core::OsRng;
        Self {
            signing_key: SigningKey::generate(&mut OsRng),
        }
    }

    /// Carga una identidad desde 32 bytes de llave privada.
    pub fn from_bytes(secret_bytes: &[u8; 32]) -> Self {
        use ed25519_dalek::SigningKey;
        Self {
            signing_key: SigningKey::from_bytes(secret_bytes),
        }
    }

    /// Exporta los 32 bytes de la llave privada.
    /// Almacena esto de forma segura; nunca lo transmitas.
    pub fn to_bytes(&self) -> [u8; 32] {
        self.signing_key.to_bytes()
    }

    /// Retorna la llave pública (identificador permanente).
    pub fn public(&self) -> IdentityPublic {
        IdentityPublic {
            bytes: self.signing_key.verifying_key().to_bytes(),
        }
    }

    /// Firma un mensaje (típicamente el nonce de AuthChallenge).
    pub fn sign(&self, message: &[u8]) -> [u8; 64] {
        use ed25519_dalek::Signer;
        self.signing_key.sign(message).to_bytes()
    }

    /// Construye el payload AuthResponse para un challenge dado.
    pub fn respond_to_challenge(&self, nonce: &[u8; 32]) -> AuthResponsePayload {
        AuthResponsePayload {
            public_key: self.public().bytes,
            signature: self.sign(nonce),
        }
    }
}

/// Versión stub de Identity cuando la feature `identity` no está activa.
#[cfg(not(feature = "identity"))]
pub struct Identity {
    secret_bytes: [u8; 32],
}

#[cfg(not(feature = "identity"))]
impl Identity {
    pub fn generate() -> Self {
        Self { secret_bytes: [0u8; 32] }
    }

    pub fn from_bytes(secret_bytes: &[u8; 32]) -> Self {
        Self { secret_bytes: *secret_bytes }
    }

    pub fn to_bytes(&self) -> [u8; 32] {
        self.secret_bytes
    }

    pub fn public(&self) -> IdentityPublic {
        IdentityPublic { bytes: [0u8; 32] }
    }

    pub fn sign(&self, _message: &[u8]) -> [u8; 64] {
        [0u8; 64]
    }

    pub fn respond_to_challenge(&self, _nonce: &[u8; 32]) -> AuthResponsePayload {
        AuthResponsePayload {
            public_key: [0u8; 32],
            signature: [0u8; 64],
        }
    }
}

impl core::fmt::Debug for Identity {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Identity")
            .field("public_key", &self.public().bytes)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_challenge_payload_roundtrip() {
        let p = AuthChallengePayload { nonce: [0xABu8; 32] };
        let mut buf = [0u8; AuthChallengePayload::SIZE];
        p.encode(&mut buf).unwrap();
        assert_eq!(AuthChallengePayload::decode(&buf).unwrap(), p);
    }

    #[test]
    fn auth_response_payload_roundtrip() {
        let p = AuthResponsePayload {
            public_key: [0x11u8; 32],
            signature: [0x22u8; 64],
        };
        let mut buf = [0u8; AuthResponsePayload::SIZE];
        p.encode(&mut buf).unwrap();
        assert_eq!(AuthResponsePayload::decode(&buf).unwrap(), p);
    }

    #[test]
    fn identity_public_verify_wrong_sig_fails() {
        let pk = IdentityPublic { bytes: [0u8; 32] };
        assert!(!pk.verify(b"hello", &[0u8; 64]));
    }

    #[cfg(feature = "identity")]
    #[test]
    fn identity_sign_and_verify() {
        let identity = Identity::generate();
        let public = identity.public();
        let message = b"test challenge nonce 0123456789AB";
        let sig = identity.sign(message);
        assert!(public.verify(message, &sig));
        // Firma incorrecta falla
        let mut bad_sig = sig;
        bad_sig[0] ^= 0xFF;
        assert!(!public.verify(message, &bad_sig));
    }

    #[cfg(feature = "identity")]
    #[test]
    fn respond_to_challenge_verifies() {
        let identity = Identity::generate();
        let nonce = [0x42u8; 32];
        let response = identity.respond_to_challenge(&nonce);
        let pk = IdentityPublic { bytes: response.public_key };
        assert!(pk.verify(&nonce, &response.signature));
    }

    #[cfg(feature = "identity")]
    #[test]
    fn from_bytes_roundtrip() {
        let id1 = Identity::generate();
        let bytes = id1.to_bytes();
        let id2 = Identity::from_bytes(&bytes);
        assert_eq!(id1.public().bytes, id2.public().bytes);
    }
}
