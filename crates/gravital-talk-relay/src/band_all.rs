//! Integración con Band-All (TOTP) como factor adicional de emparejamiento.
//!
//! ## Por qué existe
//!
//! Gravital Talk autentica una sala con un **token estático** compartido por
//! todos los participantes (`gs_session_set_room_token`, PSK de Noise). Eso tiene
//! dos problemas que este módulo ataca:
//!
//! 1. **Un token filtrado sirve para siempre.** No caduca, así que quien lo
//!    copie de una captura, un log o un chat puede entrar cuando quiera.
//! 2. **No hay segundo factor.** El token *es* la credencial, así que no hay
//!    nada que añadir.
//!
//! [Band-All](https://github.com/angelnereira/Band-All) es un servicio TOTP
//! (RFC 6238) que expone `/v1/mfa/verify` y `/v1/verify`. Un código TOTP de 30 s
//! convierte el token estático en algo que **caduca solo** y que sólo tiene
//! quien está mirando la pantalla en ese momento. Es decir, un segundo factor
//! real sin pedirle al usuario que memorice nada.
//!
//! ## Cómo se usa
//!
//! Es **opcional y degradable**: si no hay Band-All configurado, todo funciona
//! igual que antes. El fallo de la verificación nunca bloquea el audio: se
//! registra y se sigue (ver `fail_mode`).
//!
//! ```text
//!   participante                    relay                   Band-All
//!        │                            │                         │
//!        │  ClientHello + TOTP        │                         │
//!        │───────────────────────────►│                         │
//!        │                            │  POST /v1/mfa/verify    │
//!        │                            │────────────────────────►│
//!        │                            │  200 (factor válido)    │
//!        │                            │◀────────────────────────│
//!        │  ServerHello               │                         │
//!        │◀───────────────────────────│  (si falla: 403 y no enruta)
//! ```
//!
//! ## Qué se observa para mejorar Band-All
//!
//! Este proyecto es un **cliente real de Band-All con un patrón de uso muy
//! concreto**: sesiones largas (minutos u horas) que se abren con un código TOTP
//! y cuya duración supera con creces la vida del código (30 s). Eso pone a
//! prueba cosas que un flujo de login humano normal no ejercita, y la
//! documentación del comportamiento observado está en
//! `docs/integrations/band-all.md`. Los datos que se recopilan alimentan ese
//! documento, no un canal externo.

use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

/// Versión de la API de Band-All contra la que se integra.
pub const BANDALL_API_VERSION: &str = "v1";

/// Duración de un paso TOTP (RFC 6238).
const TOTP_STEP_SECS: u64 = 30;

/// Qué hacer si Band-All no responde o rechaza.
///
/// El valor por defecto es [`FailMode::Log`], no `Closed`. Motivo: si Band-All
/// se cae, una sala en curso **no** debe perder el audio. La autenticación ya
/// pasó; lo que se pierde es la capacidad de *entrar* nueva. Un fallo del
/// segundo factor tumbando una conversación en marcha es peor que admitir
/// temporalmente participantes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FailMode {
    /// Band-All no responde: se admite el paquete y se registra el hueco.
    ///
    /// Es el default deliberado. Ver la nota de la constante.
    #[default]
    Open,
    /// Band-All no responde: se rechaza el paquete.
    ///
    /// Más seguro, pero un fallo de Band-All deja la sala inaccesible.
    Closed,
    /// Band-All no responde: se admite y se cuentan cuántas veces ha pasado,
    /// para poder alertar cuando el hueco deja de ser excepcional.
    OpenWithCounter,
}

/// Configuración de la integración con Band-All.
#[derive(Debug, Clone)]
pub struct BandAllConfig {
    /// URL base del servicio, p. ej. `http://bandall:8080`.
    pub base_url: String,
    /// Credencial servicio-a-servicio (`x-service-key`).
    pub service_key: String,
    /// Identificador del tenant en Band-All.
    pub tenant_id: String,
    /// Identificador del sujeto/factor que representa la sala.
    ///
    /// Todos los participantes de una sala comparten el mismo factor: el código
    /// sale del secreto de la sala, no de cada dispositivo.
    pub subject_id: String,
    /// Tiempo máximo de espera por verificación.
    ///
    /// Corto a propósito: el handshake tiene su propio plazo
    /// (`HANDSHAKE_TIMEOUT_MS`) y no conviene sumarle una espera de red.
    pub timeout: Duration,
    /// Comportamiento cuando Band-All falla. Ver [`FailMode`].
    pub fail_mode: FailMode,
}

impl Default for BandAllConfig {
    fn default() -> Self {
        Self {
            base_url: String::new(),
            service_key: String::new(),
            tenant_id: String::new(),
            subject_id: String::new(),
            timeout: Duration::from_millis(1500),
            fail_mode: FailMode::default(),
        }
    }
}

/// Respuesta de `POST /v1/mfa/verify` y `POST /v1/verify`.
#[derive(Debug, Clone, Deserialize)]
pub struct VerifyResponse {
    /// `true` si el factor (o la sesión) es válido.
    pub ok: bool,
    /// Momento en que el código deja de valer, si Band-All lo informa.
    #[serde(default)]
    pub expires_at: Option<u64>,
    /// Scopes asociados, si los hay.
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// Resultado de verificar un código contra Band-All.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerifyOutcome {
    /// Band-All confirma que el código es válido.
    Valid,
    /// Band-All responde que no es válido.
    Invalid,
    /// Band-All no respondió (red, timeout, 5xx).
    ///
    /// Lo que se hace con esto depende de [`FailMode`].
    Unavailable,
}

/// Registro de qué ha pasado con Band-All, para la documentación observada.
///
/// No es telemetría hacia fuera: vive en memoria del relay y se expone por
/// `/metrics` para poder leerla con Prometheus. La idea es que los números que
/// se citan en `docs/integrations/band-all.md` salgan de aquí, no de una
/// estimación.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BandAllMetrics {
    /// Verificaciones correctas.
    pub valid: u64,
    /// Códigos rechazados por Band-All.
    pub invalid: u64,
    /// Veces que Band-All no respondió.
    pub unavailable: u64,
    /// Solicitudes rechazadas por [`FailMode::Closed`].
    pub rejected_closed: u64,
    /// Solicitudes admitidas pese a no poder verificar.
    pub admitted_open: u64,
    /// Latencia acumulada, para calcular la media.
    pub total_latency_ms: u64,
}

impl BandAllMetrics {
    /// Latencia media observada, en milisegundos.
    ///
    /// Devuelve 0 si no hay ninguna muestra: una media sobre cero verificaciones
    /// no es información.
    pub const fn mean_latency_ms(&self) -> u64 {
        let total = self.valid + self.invalid + self.unavailable;
        if total == 0 {
            return 0;
        }
        self.total_latency_ms / total
    }
}

/// Cliente HTTP mínimo de Band-All.
///
/// Se hizo a mano (sin `reqwest`) porque el relay ya tiene su propio cliente
/// HTTP para el plano de control y añadir otra dependencia sólo para dos
/// llamadas no compensa.
pub struct BandAllClient {
    config: BandAllConfig,
    metrics: Arc<Mutex<BandAllMetrics>>,
}

impl BandAllClient {
    pub fn new(config: BandAllConfig) -> Self {
        Self {
            config,
            metrics: Arc::new(Mutex::new(BandAllMetrics::default())),
        }
    }

    /// Snapshot de las métricas, para exponerlas.
    pub async fn metrics(&self) -> BandAllMetrics {
        self.metrics.lock().await.clone()
    }

    /// Verifica un código TOTP contra Band-All.
    ///
    /// `code` es lo que teclea el participante. No se valida su formato aquí:
    /// eso es trabajo de Band-All, y duplicar la validación en el cliente
    /// significaría que los dos pueden discrepar.
    pub async fn verify_code(&self, code: &str) -> VerifyOutcome {
        if self.config.base_url.is_empty() || self.config.service_key.is_empty() {
            // Sin configurar el cliente está desactivado, no roto.
            return VerifyOutcome::Unavailable;
        }

        let started = std::time::Instant::now();
        let url = format!(
            "{}/{}/mfa/verify",
            self.config.base_url.trim_end_matches('/'),
            BANDALL_API_VERSION
        );
        let body = serde_json::json!({
            "tenant_id": self.config.tenant_id,
            "subject_id": self.config.subject_id,
            "factor_id": code,
        });

        let result = self.post_json(&url, &body).await;
        let latency = started.elapsed().as_millis() as u64;

        let outcome = match result {
            Ok(resp) => {
                let mut m = self.metrics.lock().await;
                m.total_latency_ms += latency;
                if resp.ok {
                    m.valid += 1;
                    VerifyOutcome::Valid
                } else {
                    m.invalid += 1;
                    VerifyOutcome::Invalid
                }
            }
            Err(_) => {
                let mut m = self.metrics.lock().await;
                m.unavailable += 1;
                m.total_latency_ms += latency;
                VerifyOutcome::Unavailable
            }
        };
        outcome
    }

    /// Aplica el [`FailMode`] configurado a un resultado de verificación.
    ///
    /// Separa "qué dijo Band-All" de "qué hacemos con ello": así el modo se
    /// puede cambiar sin tocar la lógica de verificación.
    pub async fn apply_fail_mode(&self, outcome: VerifyOutcome) -> bool {
        match outcome {
            VerifyOutcome::Valid => true,
            VerifyOutcome::Invalid => false,
            VerifyOutcome::Unavailable => match self.config.fail_mode {
                FailMode::Open => {
                    self.metrics.lock().await.admitted_open += 1;
                    true
                }
                FailMode::OpenWithCounter => {
                    // Igual que `Open`, pero el contador queda arriba para que
                    // un umbral de alerta se pueda calcular sin código extra.
                    self.metrics.lock().await.admitted_open += 1;
                    true
                }
                FailMode::Closed => {
                    self.metrics.lock().await.rejected_closed += 1;
                    false
                }
            },
        }
    }

    /// POST JSON con el cliente HTTP mínimo del relay.
    async fn post_json(
        &self,
        url: &str,
        body: &serde_json::Value,
    ) -> Result<VerifyResponse, BandAllError> {
        let payload = serde_json::to_vec(body).map_err(|e| BandAllError::Encode(e.to_string()))?;
        let resp = crate::http_client::post_json(
            url,
            &payload,
            self.config.timeout,
            &[
                ("x-service-key", self.config.service_key.as_str()),
                ("content-type", "application/json"),
            ],
        )
        .await
        .map_err(|e| BandAllError::Transport(e.to_string()))?;
        serde_json::from_slice(&resp).map_err(|e| BandAllError::Decode(e.to_string()))
    }
}

/// Errores del cliente de Band-All.
///
/// Se distinguen encode/decode/red porque documentar el comportamiento
/// observado requiere saber *dónde* falló, no sólo que falló.
#[derive(Debug)]
pub enum BandAllError {
    /// No se pudo serializar la petición.
    Encode(String),
    /// La respuesta no era el JSON esperado.
    Decode(String),
    /// Fallo de red o timeout.
    Transport(String),
}

impl std::fmt::Display for BandAllError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Encode(e) => write!(f, "band-all encode: {e}"),
            Self::Decode(e) => write!(f, "band-all decode: {e}"),
            Self::Transport(e) => write!(f, "band-all transport: {e}"),
        }
    }
}

impl std::error::Error for BandAllError {}

/// Estado de la integración, para exponerlo por `/metrics`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandAllStatus {
    /// `true` si hay URL y service key configuradas.
    pub configured: bool,
    /// Modo de fallo en vigor.
    pub fail_mode: &'static str,
    /// Métricas acumuladas.
    pub metrics: BandAllMetrics,
}

/// Instante actual en segundos desde el epoch.
pub fn now_unix_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Segundos hasta el siguiente paso TOTP.
///
/// Se usa para decirle al usuario cuánto le queda al código antes de caducar,
/// que es la diferencia entre "introdúcelo" y "date prisa".
pub const fn secs_until_rotation(now: u64) -> u64 {
    TOTP_STEP_SECS - (now % TOTP_STEP_SECS)
}

/// Ventana aceptable de deriva del reloj, en pasos.
///
/// Band-All acepta el paso anterior y el siguiente por el RFC. Se documenta
/// porque es la explicación de por qué un código "caducado" a veces funciona.
pub const DRIFT_WINDOW_STEPS: u64 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_config_the_client_is_off_not_broken() {
        // Un relay sin Band-All configurado debe comportarse exactamente igual
        // que antes: la integración es opcional.
        let client = BandAllClient::new(BandAllConfig::default());
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let outcome = rt.block_on(client.verify_code("123456"));
        assert_eq!(outcome, VerifyOutcome::Unavailable);
    }

    #[test]
    fn fail_mode_open_admits_when_band_all_is_down() {
        // El default: un fallo de Band-All no debe tumbar una sala en curso.
        let client = BandAllClient::new(BandAllConfig {
            fail_mode: FailMode::Open,
            ..BandAllConfig::default()
        });
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(rt.block_on(client.apply_fail_mode(VerifyOutcome::Unavailable)));
    }

    #[test]
    fn fail_mode_closed_rechaza_cuando_band_all_no_responde() {
        let client = BandAllClient::new(BandAllConfig {
            fail_mode: FailMode::Closed,
            ..BandAllConfig::default()
        });
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(!rt.block_on(client.apply_fail_mode(VerifyOutcome::Unavailable)));
    }

    #[test]
    fn invalid_is_always_rejected_regardless_of_fail_mode() {
        // Un código rechazado por Band-All es un rechazo, no un fallo. Ningún
        // modo de fallo debe admitirlo.
        for mode in [FailMode::Open, FailMode::Closed, FailMode::OpenWithCounter] {
            let client = BandAllClient::new(BandAllConfig {
                fail_mode: mode,
                ..BandAllConfig::default()
            });
            let rt = tokio::runtime::Builder::new_current_thread()
                .build()
                .unwrap();
            assert!(
                !rt.block_on(client.apply_fail_mode(VerifyOutcome::Invalid)),
                "el modo {mode:?} admitió un código inválido"
            );
        }
    }

    #[test]
    fn valid_is_always_admitted() {
        let client = BandAllClient::new(BandAllConfig::default());
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        assert!(rt.block_on(client.apply_fail_mode(VerifyOutcome::Valid)));
    }

    #[test]
    fn mean_latency_is_zero_without_samples() {
        // Una media sobre cero muestras sería división por cero disfrazada.
        let m = BandAllMetrics::default();
        assert_eq!(m.mean_latency_ms(), 0);
    }

    #[test]
    fn mean_latency_ignores_nothing() {
        let m = BandAllMetrics {
            valid: 1,
            invalid: 1,
            unavailable: 1,
            total_latency_ms: 300,
            ..BandAllMetrics::default()
        };
        assert_eq!(m.mean_latency_ms(), 100);
    }

    #[test]
    fn rotation_timer_is_within_one_step() {
        let now = 1_000;
        let left = secs_until_rotation(now);
        assert!((1..=TOTP_STEP_SECS).contains(&left));
        // Al inicio del paso queda el paso completo.
        assert_eq!(secs_until_rotation(TOTP_STEP_SECS), TOTP_STEP_SECS);
    }

    #[test]
    fn drift_window_matches_the_rfc_expectation() {
        // Un paso a cada lado es lo que permite un reloj desajustado sin
        // rechazar usuarios legítimos.
        assert_eq!(DRIFT_WINDOW_STEPS, 1);
    }
}
