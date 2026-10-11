//! Llamada a un contacto: un datagrama cifrado que dice "quiero hablar".
//!
//! Dos dispositivos que ya se emparejaron una vez comparten un secreto (el
//! token de aquella sala). Con él, uno puede pedirle al otro que entre en una
//! sala nueva sin repetir el QR: le manda una invitación cifrada a la última
//! dirección que le conoce. Sólo quien tiene el secreto puede leerla o
//! fabricarla, así que un tercero ni la entiende ni puede hacer sonar el
//! teléfono de nadie.
//!
//! ```text
//! [ "GTKN" ][ versión ][ id de contacto: 8 B ][ nonce: 12 B ][ cifrado + tag ]
//! ```
//!
//! El id (derivado del secreto, no reversible) permite al receptor saber de
//! qué contacto viene sin probar todas sus claves. La cabecera va autenticada.
//! La protección contra repeticiones la pone quien interpreta el contenido
//! (lleva fecha): este módulo sólo sella y abre.

use gravital_talk_core::crypto::{decrypt_in_place, encrypt_in_place, NONCE_SIZE, TAG_SIZE};
use rand_core::RngCore;
use sha2::{Digest, Sha256};

const MAGIC: [u8; 4] = *b"GTKN";
const VERSION: u8 = 1;

/// Tamaño del id de contacto en el datagrama.
pub const ID_SIZE: usize = 8;
const HEADER_SIZE: usize = MAGIC.len() + 1 + ID_SIZE;

/// Bytes que el sellado añade al contenido.
pub const OVERHEAD: usize = HEADER_SIZE + NONCE_SIZE + TAG_SIZE;

/// Contenido máximo: el datagrama entero cabe en un paquete sin fragmentar.
pub const MAX_PAYLOAD: usize = 1200 - OVERHEAD;

/// Por qué no se pudo sellar o abrir una llamada.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnockError {
    /// El contenido no cabe en un datagrama.
    TooLong,
    /// No es una llamada de esta versión, o no es de este contacto.
    NotForUs,
    /// Es para este contacto pero no supera la autenticación.
    Forged,
}

fn derive(secret: &[u8], label: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(label);
    hasher.update(secret);
    hasher.finalize().into()
}

fn key(secret: &[u8]) -> [u8; 32] {
    // Etiqueta propia: la clave de llamada no puede coincidir con ninguna
    // otra cosa que se derive del mismo token.
    derive(secret, b"gravital-talk/knock/key/v1\0")
}

/// Identificador público del contacto que comparte `secret`.
#[must_use]
pub fn contact_id(secret: &[u8]) -> [u8; ID_SIZE] {
    let digest = derive(secret, b"gravital-talk/knock/id/v1\0");
    let mut id = [0u8; ID_SIZE];
    id.copy_from_slice(&digest[..ID_SIZE]);
    id
}

/// Id del contacto al que va dirigido `datagram`, si es una llamada.
///
/// Es la comprobación barata con la que el receptor elige el secreto.
#[must_use]
pub fn addressed_to(datagram: &[u8]) -> Option<[u8; ID_SIZE]> {
    if datagram.len() < OVERHEAD || datagram[..4] != MAGIC || datagram[4] != VERSION {
        return None;
    }
    datagram[5..HEADER_SIZE].try_into().ok()
}

/// Sella `payload` para el contacto que comparte `secret`.
pub fn seal(secret: &[u8], payload: &[u8]) -> Result<Vec<u8>, KnockError> {
    if payload.len() > MAX_PAYLOAD {
        return Err(KnockError::TooLong);
    }
    let mut out = Vec::with_capacity(OVERHEAD + payload.len());
    out.extend_from_slice(&MAGIC);
    out.push(VERSION);
    out.extend_from_slice(&contact_id(secret));
    let mut nonce = [0u8; NONCE_SIZE];
    rand_core::OsRng.fill_bytes(&mut nonce);
    out.extend_from_slice(&nonce);
    let body = out.len();
    out.extend_from_slice(payload);
    out.resize(body + payload.len() + TAG_SIZE, 0);
    let (header, sealed) = out.split_at_mut(body);
    encrypt_in_place(
        &key(secret),
        &nonce,
        &header[..HEADER_SIZE],
        sealed,
        payload.len(),
    )
    .map_err(|_| KnockError::TooLong)?;
    Ok(out)
}

/// Abre una llamada sellada con `secret` y devuelve su contenido.
pub fn open(secret: &[u8], datagram: &[u8]) -> Result<Vec<u8>, KnockError> {
    if addressed_to(datagram) != Some(contact_id(secret)) {
        return Err(KnockError::NotForUs);
    }
    let (header, rest) = datagram.split_at(HEADER_SIZE);
    let (nonce, sealed) = rest.split_at(NONCE_SIZE);
    let nonce: [u8; NONCE_SIZE] = nonce.try_into().map_err(|_| KnockError::NotForUs)?;
    let mut body = sealed.to_vec();
    let len = body.len();
    let plain = decrypt_in_place(&key(secret), &nonce, header, &mut body, len)
        .map_err(|_| KnockError::Forged)?;
    body.truncate(plain);
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lo_sellado_lo_abre_quien_comparte_el_secreto() {
        let sealed = seal(b"token-de-sala", b"hola").unwrap();
        assert_eq!(sealed.len(), OVERHEAD + 4);
        assert_eq!(addressed_to(&sealed), Some(contact_id(b"token-de-sala")));
        assert_eq!(open(b"token-de-sala", &sealed).unwrap(), b"hola");
    }

    #[test]
    fn otro_secreto_ni_lo_reconoce_como_suyo() {
        let sealed = seal(b"uno", b"hola").unwrap();
        assert_eq!(open(b"otro", &sealed), Err(KnockError::NotForUs));
    }

    #[test]
    fn un_byte_cambiado_no_pasa_la_autenticacion() {
        let sealed = seal(b"uno", b"hola").unwrap();
        // Cualquier byte tras el id: nonce, contenido o tag.
        for i in HEADER_SIZE..sealed.len() {
            let mut bad = sealed.clone();
            bad[i] ^= 0x01;
            assert_eq!(open(b"uno", &bad), Err(KnockError::Forged), "byte {i}");
        }
    }

    #[test]
    fn dos_sellados_del_mismo_contenido_no_se_parecen() {
        let a = seal(b"uno", b"hola").unwrap();
        let b = seal(b"uno", b"hola").unwrap();
        assert_ne!(a[HEADER_SIZE..], b[HEADER_SIZE..]);
    }

    #[test]
    fn lo_que_no_es_una_llamada_se_descarta_sin_caerse() {
        for junk in [
            &b""[..],
            b"GTKN",
            b"GTRV\x01aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            &[0u8; 64],
        ] {
            assert_eq!(addressed_to(junk), None);
            assert_eq!(open(b"uno", junk), Err(KnockError::NotForUs));
        }
        assert_eq!(
            seal(b"uno", &vec![0u8; MAX_PAYLOAD + 1]),
            Err(KnockError::TooLong)
        );
        assert!(seal(b"uno", &vec![0u8; MAX_PAYLOAD]).unwrap().len() <= 1200);
    }

    #[test]
    fn el_id_no_es_la_clave() {
        assert_ne!(contact_id(b"uno")[..], key(b"uno")[..ID_SIZE]);
    }
}
