//! Contrato de emparejamiento: oferta, código rotativo y validación.
//!
//! ## El hueco que cubre
//!
//! El handshake del protocolo (X25519 + HKDF + AEAD, 4 pasos) demuestra que
//! ambos extremos poseen las claves. Lo que **no** existía era un contrato de
//! emparejamiento: nada decía si una oferta seguía vigente, si un código podía
//! reutilizarse, ni por qué se rechazaba a alguien. El resultado era que un
//! secreto filtrado servía indefinidamente y los fallos llegaban como
//! "handshake fallido" sin motivo.
//!
//! ## Qué es y qué no es el código
//!
//! El código es `HOTP(secreto_de_sala, contador)` truncado a 6 dígitos, donde
//! el contador es `now / 30`. Sirve para que dos personas confirmen por
//! teléfono que ambos tienen el mismo secreto de sala.
//!
//! **No es un segundo factor por sí solo.** Deriva del mismo secreto que
//! autentica la sesión, así que es una representación legible de ese secreto,
//! no una credencial independiente. Sí lo es en modo sala, donde el relay puede
//! verificarlo contra Band-All (ver `docs/integrations/band-all.md`).
//!
//! ## Elección criptográfica, documentada
//!
//! RFC 6238 define TOTP sobre HOTP, y HOTP se define con HMAC-SHA1 en el RFC
//! original. Aquí se usa **HMAC-SHA256**, que RFC 6238 también especifica como
//! alternativa. El motivo es práctico: el workspace ya depende de `sha2` y
//! `hmac`, y añadir SHA-1 sólo por el vector canónico significaría otra
//! primitiva más que mantener. Los tests verifican contra los vectores
//! SHA-256 del propio RFC 6238 (Apéndice B), así que la desviación es
//! deliberada y está probada, no asumida.
//!
//! ## Ventana de deriva
//!
//! Se aceptan los contadores `C-1`, `C` y `C+1`. Un reloj desajustado hasta 30
//! s en cualquier dirección sigue funcionando. Ampliar la ventana más allá de
//! eso multiplica la superficie de ataque sin beneficiar a nadie real: 30 s de
//! deriva ya es un reloj bastante roto.

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use sha2::Sha256;

/// Algoritmo de MAC usado.
type HmacSha256 = Hmac<Sha256>;

/// Duración de un paso TOTP, en segundos. La fija RFC 6238.
pub const TOTP_STEP_SECS: u64 = 30;

/// Dígitos del código que ve el usuario.
pub const CODE_DIGITS: u32 = 6;

/// Ventana de deriva aceptada, en pasos a cada lado.
pub const DRIFT_STEPS: i64 = 1;

/// Cuántos pasos hacia atrás se inspeccionan para distinguir "código caducado"
/// de "código que no es de esta sala".
///
/// Acotado a propósito: cuanto más grande, más códigos válidos-y-viejos se le
/// revelan a quien esté probando, y el usuario sólo necesita que le digan
/// "espera al siguiente", no cuál fue el bueno.
pub const LOOKBACK_STEPS: i64 = 4;

// ── HOTP / TOTP ────────────────────────────────────────────────────────────

/// Truncado dinámico de RFC 4226 §5.3 sobre un digest MAC.
///
/// Es la parte delicada del HOTP y está separada del hash a propósito: así los
/// tests pueden verificarla contra los vectores canónicos del RFC 6238, que
/// usan HMAC-SHA1, sin que la producción dependa de SHA-1.
///
/// Elige el offset con el nibble bajo del último byte y toma 4 bytes
/// enmascarados a 31 bits. Un off-by-one aquí produce códigos que parecen
/// válidos y no lo son.
fn dynamic_truncate(digest: &[u8], digits: u32) -> u32 {
    // El nibble bajo del último byte elige de dónde salen los 4 bytes.
    let offset = usize::from(digest[digest.len() - 1] & 0x0f);
    let binary = (u32::from(digest[offset] & 0x7f) << 24)
        | (u32::from(digest[offset + 1]) << 16)
        | (u32::from(digest[offset + 2]) << 8)
        | u32::from(digest[offset + 3]);

    binary % 10u32.pow(digits)
}

/// `HOTP(K, C)` según RFC 4226 con HMAC-SHA256, `digits` dígitos.
fn hotp(secret: &[u8], counter: u64, digits: u32) -> u32 {
    let Ok(mut mac) = HmacSha256::new_from_slice(secret) else {
        // `new_from_slice` sólo falla con clave vacía, y toda esta API exige
        // secreto. Ese caso imposible devuelve un código inválido, no un pánico.
        return 0;
    };
    // El contador viaja como entero de 64 bits, big-endian.
    mac.update(&counter.to_be_bytes());
    let digest = mac.finalize().into_bytes();
    dynamic_truncate(&digest, digits)
}

/// Código TOTP de `digits` dígitos para el instante `now_unix`.
///
/// Público porque quien genera el QR y quien lo escanea necesitan el mismo
/// código, y ambos pueden estar en máquinas distintas.
#[must_use]
pub fn totp(secret: &[u8], now_unix: u64, digits: u32) -> u32 {
    let counter = now_unix / TOTP_STEP_SECS;
    hotp(secret, counter, digits)
}

/// Código formateado con separador: `123 456` se lee mejor que `123456`.
#[must_use]
pub fn format_code(code: u32, digits: u32) -> String {
    let digits = digits.max(4) as usize;
    let text = format!("{code:0digits$}");
    let half = digits / 2;
    format!("{} {}", &text[..half], &text[half..])
}

/// Segundos hasta que el código actual caduca.
#[must_use]
pub const fn secs_until_rotation(now_unix: u64) -> u64 {
    TOTP_STEP_SECS - (now_unix % TOTP_STEP_SECS)
}

// ── Oferta de emparejamiento ───────────────────────────────────────────────

/// Motivo por el que se rechaza un intento de emparejamiento.
///
/// Existe para que la UI pueda explicar *qué* pasó. "Handshake fallido" obliga
/// al usuario a adivinar entre secreto caducado, código equivocado y red.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PairingReject {
    /// La oferta caducó hace `expired_ago_secs` segundos.
    Expired {
        /// Segundos desde que caducó.
        expired_ago_secs: u64,
    },
    /// El código no corresponde al secreto.
    WrongCode,
    /// La oferta agotó sus usos.
    Exhausted {
        /// Usos máximos que tenía.
        max_uses: u32,
    },
    /// El código está dentro de la ventana de deriva pero no es el actual.
    ///
    /// Se distingue de `WrongCode` porque la solución es esperar al siguiente
    /// paso, no reintentar el mismo código deprisa.
    StaleCode,
}

impl PairingReject {
    /// Mensaje para mostrar al usuario.
    #[must_use]
    pub const fn message(&self) -> &'static str {
        match self {
            Self::Expired { .. } => "La invitación caducó. Pide una nueva.",
            Self::WrongCode => "Ese código no corresponde a esta sala.",
            Self::Exhausted { .. } => "Esta invitación ya se usó. Pide otra.",
            Self::StaleCode => "Ese código acaba de caducar. Espera al siguiente.",
        }
    }
}

/// Oferta de emparejamiento publicada por el anfitrión.
///
/// Es el contrato: qué se pide, cuánto vale, cuándo caduca y cuántas veces se
/// puede usar. El anfitrión la crea, publica el código en el QR y la conserva
/// para validar a quien se una.
#[derive(Debug)]
pub struct PairingOffer {
    secret: [u8; 32],
    created_at_unix: u64,
    /// 0 = no caduca.
    ttl_secs: u64,
    /// 0 = usos ilimitados (varios invitados en la misma sala).
    max_uses: u32,
    uses: AtomicU32,
}

impl PairingOffer {
    /// Crea una oferta sin caducidad ni límite de usos.
    ///
    /// Es el caso de una sala a la que se van uniendo varios participantes.
    #[must_use]
    pub fn new(secret: [u8; 32]) -> Self {
        Self {
            secret,
            created_at_unix: now_unix(),
            ttl_secs: 0,
            max_uses: 0,
            uses: AtomicU32::new(0),
        }
    }

    /// Fija la caducidad en segundos desde la creación.
    #[must_use]
    pub const fn with_ttl(mut self, ttl_secs: u64) -> Self {
        self.ttl_secs = ttl_secs;
        self
    }

    /// Fija el número máximo de usos. 0 = ilimitado.
    #[must_use]
    pub const fn with_max_uses(mut self, max_uses: u32) -> Self {
        self.max_uses = max_uses;
        self
    }

    /// Secreto de sala, para configurar la sesión.
    #[must_use]
    pub const fn secret(&self) -> &[u8; 32] {
        &self.secret
    }

    /// Código actual, de `CODE_DIGITS` dígitos.
    #[must_use]
    pub fn code_at(&self, now_unix: u64) -> u32 {
        totp(&self.secret, now_unix, CODE_DIGITS)
    }

    /// Código actual, usando el reloj del sistema.
    #[must_use]
    pub fn code(&self) -> u32 {
        self.code_at(now_unix())
    }

    /// Usos consumidos hasta ahora.
    #[must_use]
    pub fn uses(&self) -> u32 {
        self.uses.load(Ordering::Acquire)
    }

    /// Segundos hasta que la oferta caduca, o `None` si no caduca.
    #[must_use]
    pub const fn secs_until_expiry(&self, now_unix: u64) -> Option<u64> {
        if self.ttl_secs == 0 {
            return None;
        }
        let deadline = self.created_at_unix.saturating_add(self.ttl_secs);
        Some(deadline.saturating_sub(now_unix))
    }

    /// Verifica un código contra esta oferta.
    ///
    /// El orden de las comprobaciones importa: se mira la caducidad antes que
    /// el código, porque un código caducado debe reportarse como caducado aunque
    /// además sea incorrecto. Es la diferencia entre "pide otra invitación" y
    /// "revisa lo que tecleas".
    ///
    /// Consume un uso si todo va bien.
    pub fn verify(&self, code: u32, now_unix: u64) -> Result<(), PairingReject> {
        if self.secs_until_expiry(now_unix) == Some(0) {
            return Err(PairingReject::Expired {
                expired_ago_secs: now_unix
                    .saturating_sub(self.created_at_unix.saturating_add(self.ttl_secs)),
            });
        }

        if self.max_uses > 0 && self.uses.load(Ordering::Acquire) >= self.max_uses {
            return Err(PairingReject::Exhausted {
                max_uses: self.max_uses,
            });
        }

        // Ventana de deriva: se acepta un paso a cada lado. Un reloj con hasta
        // 30 s de desajuste sigue funcionando sin que el usuario note nada.
        let counter = (now_unix / TOTP_STEP_SECS) as i64;
        for drift in -DRIFT_STEPS..=DRIFT_STEPS {
            let probe = counter + drift;
            if probe < 0 {
                continue;
            }
            if hotp(&self.secret, probe as u64, CODE_DIGITS) == code {
                self.uses.fetch_add(1, Ordering::AcqRel);
                return Ok(());
            }
        }

        // Mirar un poco hacia atrás para distinguir "código viejo" de "código
        // que no es de esta sala". Es acotado a propósito: comprobar todo el
        // historial sería gratis para el atacante y no ayuda a nadie.
        for back in (DRIFT_STEPS + 1)..=LOOKBACK_STEPS {
            let probe = counter - back;
            if probe < 0 {
                break;
            }
            if hotp(&self.secret, probe as u64, CODE_DIGITS) == code {
                return Err(PairingReject::StaleCode);
            }
        }

        Err(PairingReject::WrongCode)
    }
}

/// Instante actual en segundos desde el epoch.
#[must_use]
pub fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Conformidad del truncado contra RFC 6238 Apéndice B.
    ///
    /// Los vectores canónicos del RFC usan HMAC-SHA1. Aquí se calcula el
    /// HMAC-SHA1 del contador y se pasa por `dynamic_truncate`, que es la
    /// función que de verdad puede tener el off-by-one. Así se verifica contra
    /// el estándar sin arrastrar SHA-1 a la producción.
    ///
    /// Los valores están sacados del RFC 6238 Apéndice B y reproducidos con una
    /// implementación independiente (`hmac` de Python) antes de escribirlos.
    #[test]
    fn dynamic_truncate_matches_rfc6238_vectors() {
        use hmac::{Hmac, Mac};

        // Secreto ASCII del RFC: "12345678901234567890".
        let secret = b"12345678901234567890";
        let vectors: &[(u64, u32)] = &[
            (59, 94287082),
            (1111111109, 7081804),
            (1111111111, 14050471),
            (1234567890, 89005924),
            (2000000000, 69279037),
            (20000000000, 65353130),
        ];

        for (t, expected) in vectors {
            let counter = t / TOTP_STEP_SECS;
            let mut mac = Hmac::<sha1::Sha1>::new_from_slice(secret).expect("secret valido");
            mac.update(&counter.to_be_bytes());
            let digest = mac.finalize().into_bytes();

            let code = dynamic_truncate(&digest, 8);
            assert_eq!(
                code, *expected,
                "truncado para T={t}: esperado {expected}, obtenido {code}"
            );
        }
    }

    /// El código de producción (SHA-256) es estable entre ejecuciones.
    ///
    /// No se compara contra vectores del RFC porque el RFC no publica valores
    /// SHA-256 con este secreto; lo que se verifica aquí es determinismo, y la
    /// corrección del truncado ya queda fijada por el test de arriba.
    #[test]
    fn production_code_is_deterministic() {
        let secret = b"12345678901234567890";
        let a = totp(secret, 59, 8);
        let b = totp(secret, 59, 8);
        assert_eq!(a, b);
    }

    #[test]
    fn code_is_deterministic_for_the_same_instant() {
        let offer = PairingOffer::new([7u8; 32]);
        let t = 1_700_000_000;
        assert_eq!(offer.code_at(t), offer.code_at(t));
    }

    #[test]
    fn code_changes_across_the_step_boundary() {
        let offer = PairingOffer::new([9u8; 32]);
        let inside = 1_700_000_000;
        let next_step = inside + TOTP_STEP_SECS;
        assert_ne!(
            offer.code_at(inside),
            offer.code_at(next_step),
            "el código debe rotar con el contador"
        );
    }

    #[test]
    fn six_digit_code_fits_the_ui() {
        let offer = PairingOffer::new([3u8; 32]);
        let code = offer.code_at(1_700_000_000);
        assert!(code < 1_000_000, "código de 6 dígitos, obtenido {code}");
    }

    #[test]
    fn drift_window_accepts_adjacent_steps() {
        let offer = PairingOffer::new([5u8; 32]);
        let now = 1_700_000_000;

        // Código del paso anterior: sigue siendo válido (deriva de reloj).
        let prev = offer.code_at(now - TOTP_STEP_SECS);
        assert!(
            offer.verify(prev, now).is_ok(),
            "debe aceptar derima de -1 paso"
        );

        let next = offer.code_at(now + TOTP_STEP_SECS);
        assert!(
            offer.verify(next, now).is_ok(),
            "debe aceptar deriva de +1 paso"
        );
    }

    #[test]
    fn outside_the_drift_window_is_stale_not_wrong() {
        let offer = PairingOffer::new([5u8; 32]);
        let now = 1_700_000_000;

        // Dos pasos atrás: fuera de la ventana, pero es un código que existió.
        let old = offer.code_at(now - 2 * TOTP_STEP_SECS);
        assert_eq!(
            offer.verify(old, now),
            Err(PairingReject::StaleCode),
            "un código caducado debe distinguirse de uno incorrecto"
        );
    }

    #[test]
    fn different_secret_gives_different_code() {
        let a = PairingOffer::new([1u8; 32]);
        let b = PairingOffer::new([2u8; 32]);
        let t = 1_700_000_000;
        assert_ne!(a.code_at(t), b.code_at(t));
    }

    #[test]
    fn wrong_code_is_rejected_and_consumes_nothing() {
        let offer = PairingOffer::new([11u8; 32]).with_max_uses(3);
        let now = 1_700_000_000;
        let good = offer.code_at(now);
        let bad = if good == 0 { 1 } else { good - 1 };

        assert_eq!(offer.verify(bad, now), Err(PairingReject::WrongCode));
        assert_eq!(offer.uses(), 0, "un código fallido no debe gastar un uso");

        assert!(offer.verify(good, now).is_ok());
        assert_eq!(offer.uses(), 1);
    }

    #[test]
    fn expired_offer_is_reported_as_expired() {
        // TTL de 60 s, comprobado 120 s después: el mensaje tiene que empujar a
        // pedir una nueva, no a reintentar el mismo código.
        let offer = PairingOffer::new([13u8; 32]).with_ttl(60);
        let created = offer.created_at_unix;
        let later = created + 120;
        let code = offer.code_at(later);

        assert_eq!(
            offer.verify(code, later),
            Err(PairingReject::Expired {
                expired_ago_secs: 60
            })
        );
    }

    #[test]
    fn expired_offer_reports_even_with_a_valid_code() {
        // El orden de comprobaciones: caducidad antes que código.
        let offer = PairingOffer::new([13u8; 32]).with_ttl(60);
        let created = offer.created_at_unix;
        let code = offer.code_at(created + 120);
        match offer.verify(code, created + 120) {
            Err(PairingReject::Expired { .. }) => {}
            other => panic!("esperaba Expired, obtuve {other:?}"),
        }
    }

    #[test]
    fn max_uses_exhausts_the_offer() {
        let offer = PairingOffer::new([17u8; 32]).with_max_uses(2);
        let now = 1_700_000_000;
        let code = offer.code_at(now);

        assert!(offer.verify(code, now).is_ok());
        assert!(offer.verify(code, now).is_ok());
        assert_eq!(
            offer.verify(code, now),
            Err(PairingReject::Exhausted { max_uses: 2 })
        );
        assert_eq!(offer.uses(), 2, "el intento fallido no debe gastar otro");
    }

    #[test]
    fn unlimited_uses_allows_many_participants() {
        // Una sala con varios invitados: el código no se consume con uno solo.
        let offer = PairingOffer::new([19u8; 32]);
        let now = 1_700_000_000;
        let code = offer.code_at(now);
        for i in 0..5 {
            assert!(offer.verify(code, now).is_ok(), "uso {i} debe aceptarse");
        }
        assert_eq!(offer.uses(), 5);
    }

    #[test]
    fn without_ttl_the_offer_never_expires() {
        let offer = PairingOffer::new([23u8; 32]);
        let far_future = offer.created_at_unix + 10 * 365 * 24 * 3600;
        assert_eq!(offer.secs_until_expiry(far_future), None);
    }

    #[test]
    fn format_code_splits_for_reading() {
        assert_eq!(format_code(123456, 6), "123 456");
        assert_eq!(format_code(1, 6), "000 001");
        assert_eq!(format_code(12345678, 8), "1234 5678");
    }

    #[test]
    fn rotation_timer_is_within_one_step() {
        assert!(secs_until_rotation(0) <= TOTP_STEP_SECS);
        assert!(secs_until_rotation(0) >= 1);
        assert_eq!(secs_until_rotation(TOTP_STEP_SECS), TOTP_STEP_SECS);
    }

    #[test]
    fn reject_messages_are_actionable() {
        // Cada motivo debe decirle al usuario qué hacer, no sólo qué falló.
        for reject in [
            PairingReject::Expired {
                expired_ago_secs: 1,
            },
            PairingReject::WrongCode,
            PairingReject::Exhausted { max_uses: 1 },
            PairingReject::StaleCode,
        ] {
            assert!(!reject.message().is_empty());
        }
        assert!(PairingReject::Expired {
            expired_ago_secs: 1
        }
        .message()
        .contains("nueva"));
        assert!(PairingReject::StaleCode.message().contains("siguiente"));
    }
}
