//! Primitivas AEAD para cifrado autenticado de payloads.
//!
//! Algoritmo: ChaCha20-Poly1305 (RFC 8439).
//! Nonce: derivado de `sequence || session_id || 0000` — nunca se transmite.
//! AAD: los 24 bytes del header del paquete — autentica sin cifrar.
//! Tag: 16 bytes Poly1305, embebido al final del ciphertext en el wire.
//!
//! Layout del payload cifrado en el wire:
//! ```text
//! [ ciphertext (N bytes) ][ poly1305-tag (16 bytes) ]
//! ```
//! El campo `payload_len` del paquete incluye ambos (N + 16).

use chacha20poly1305::{
    aead::{AeadInPlace, KeyInit},
    ChaCha20Poly1305, Key, Nonce, Tag,
};

use crate::error::Error;

/// Tamaño de clave (32 bytes).
pub const KEY_SIZE: usize = 32;
/// Tamaño del tag Poly1305 (16 bytes).
pub const TAG_SIZE: usize = 16;
/// Tamaño del nonce (12 bytes).
pub const NONCE_SIZE: usize = 12;

/// Clave de sesión AEAD.
pub type SessionKey = [u8; KEY_SIZE];

/// Construye el nonce de 12 bytes a partir del número de secuencia y el
/// session_id. Nunca se transmite: ambos lados derivan el mismo valor.
#[inline]
#[must_use]
pub fn make_nonce(sequence: u32, session_id: u32) -> [u8; NONCE_SIZE] {
    let mut n = [0u8; NONCE_SIZE];
    n[..4].copy_from_slice(&sequence.to_be_bytes());
    n[4..8].copy_from_slice(&session_id.to_be_bytes());
    // bytes 8-11 = 0 (padding)
    n
}

/// Cifra `buffer[..plaintext_len]` in-place usando ChaCha20-Poly1305.
///
/// Al terminar, `buffer` contiene:
/// ```text
/// [ ciphertext (plaintext_len bytes) ][ tag (16 bytes) ]
/// ```
/// El buffer debe tener capacidad para al menos `plaintext_len + TAG_SIZE` bytes.
///
/// `aad` se autentica pero no se cifra (usa los 24 bytes del header).
///
/// Devuelve el número de bytes escritos (`plaintext_len + TAG_SIZE`).
pub fn encrypt_in_place(
    key: &SessionKey,
    nonce: &[u8; NONCE_SIZE],
    aad: &[u8],
    buffer: &mut [u8],
    plaintext_len: usize,
) -> Result<usize, Error> {
    if buffer.len() < plaintext_len + TAG_SIZE {
        return Err(Error::BufferTooSmall);
    }

    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce_obj = Nonce::from_slice(nonce);

    let tag: Tag = cipher
        .encrypt_in_place_detached(nonce_obj, aad, &mut buffer[..plaintext_len])
        .map_err(|_| Error::DecryptionFailed)?;

    buffer[plaintext_len..plaintext_len + TAG_SIZE].copy_from_slice(&tag);
    Ok(plaintext_len + TAG_SIZE)
}

/// Descifra y autentica `buffer[..ciphertext_len]` in-place.
///
/// `ciphertext_len` incluye los 16 bytes del tag al final.
///
/// Al terminar, `buffer[..plaintext_len]` contiene el plaintext.
/// Devuelve el número de bytes de plaintext (`ciphertext_len - TAG_SIZE`).
pub fn decrypt_in_place(
    key: &SessionKey,
    nonce: &[u8; NONCE_SIZE],
    aad: &[u8],
    buffer: &mut [u8],
    ciphertext_len: usize,
) -> Result<usize, Error> {
    if ciphertext_len < TAG_SIZE {
        return Err(Error::TooShort);
    }
    if buffer.len() < ciphertext_len {
        return Err(Error::BufferTooSmall);
    }

    let plaintext_len = ciphertext_len - TAG_SIZE;
    let tag_bytes: [u8; TAG_SIZE] = buffer[plaintext_len..ciphertext_len]
        .try_into()
        .map_err(|_| Error::MalformedPayload)?;
    let tag = Tag::from(tag_bytes);

    let cipher = ChaCha20Poly1305::new(Key::from_slice(key));
    let nonce_obj = Nonce::from_slice(nonce);

    cipher
        .decrypt_in_place_detached(nonce_obj, aad, &mut buffer[..plaintext_len], &tag)
        .map_err(|_| Error::DecryptionFailed)?;

    Ok(plaintext_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_key() -> SessionKey {
        [0x42u8; KEY_SIZE]
    }

    #[test]
    fn encrypt_decrypt_roundtrip() {
        let key = test_key();
        let nonce = make_nonce(42, 0xDEAD_BEEF);
        let aad = [0u8; 24];
        let plaintext = b"hello gravital sound!";

        let mut buf = [0u8; 64];
        buf[..plaintext.len()].copy_from_slice(plaintext);

        let enc_len = encrypt_in_place(&key, &nonce, &aad, &mut buf, plaintext.len()).unwrap();
        assert_eq!(enc_len, plaintext.len() + TAG_SIZE);

        let dec_len = decrypt_in_place(&key, &nonce, &aad, &mut buf, enc_len).unwrap();
        assert_eq!(dec_len, plaintext.len());
        assert_eq!(&buf[..dec_len], plaintext);
    }

    #[test]
    fn wrong_key_fails() {
        let key = test_key();
        let nonce = make_nonce(1, 1);
        let aad = [0u8; 24];
        let plaintext = b"secret";

        let mut buf = [0u8; 32];
        buf[..plaintext.len()].copy_from_slice(plaintext);
        let enc_len = encrypt_in_place(&key, &nonce, &aad, &mut buf, plaintext.len()).unwrap();

        let bad_key = [0x00u8; KEY_SIZE];
        assert_eq!(
            decrypt_in_place(&bad_key, &nonce, &aad, &mut buf, enc_len),
            Err(Error::DecryptionFailed)
        );
    }

    #[test]
    fn tampered_ciphertext_fails() {
        let key = test_key();
        let nonce = make_nonce(0, 0);
        let aad = [0u8; 24];
        let plaintext = b"tamper test";

        let mut buf = [0u8; 32];
        buf[..plaintext.len()].copy_from_slice(plaintext);
        let enc_len = encrypt_in_place(&key, &nonce, &aad, &mut buf, plaintext.len()).unwrap();

        buf[2] ^= 0xFF;
        assert_eq!(
            decrypt_in_place(&key, &nonce, &aad, &mut buf, enc_len),
            Err(Error::DecryptionFailed)
        );
    }

    #[test]
    fn tampered_aad_fails() {
        let key = test_key();
        let nonce = make_nonce(0, 0);
        let aad = [0u8; 24];
        let plaintext = b"aad test";

        let mut buf = [0u8; 32];
        buf[..plaintext.len()].copy_from_slice(plaintext);
        let enc_len = encrypt_in_place(&key, &nonce, &aad, &mut buf, plaintext.len()).unwrap();

        let bad_aad = [0xFFu8; 24];
        assert_eq!(
            decrypt_in_place(&key, &nonce, &bad_aad, &mut buf, enc_len),
            Err(Error::DecryptionFailed)
        );
    }

    #[test]
    fn nonce_deterministic() {
        assert_eq!(make_nonce(1, 2), make_nonce(1, 2));
        assert_ne!(make_nonce(1, 2), make_nonce(2, 1));
    }
}
