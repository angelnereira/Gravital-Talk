//! Tratamiento de la voz alrededor del códec: lo que hace que se entienda.
//!
//! Tres piezas pequeñas, cada una con su estado y sin reservar memoria al
//! procesar:
//!
//! - [`Biquad`]: el filtro de segundo orden con el que se hacen el pasa-altos
//!   de entrada (quita retumbe y golpes en el micrófono) y el realce de
//!   presencia del receptor (sube la zona donde viven las consonantes).
//! - [`Leveler`]: lleva la voz a un nivel estable, hable bajo o alto quien
//!   hable, y acota los picos. Sólo aprende el nivel **mientras hay voz**:
//!   si lo hiciera también en las pausas, subiría el ruido de fondo entre
//!   frase y frase.
//! - [`VoiceProfile`] / [`VoiceTuning`]: los ajustes coordinados de todo el
//!   camino (ruido, nivel, tasa, redundancia), para que nadie tenga que
//!   elegirlos uno a uno.
//!
//! Las muestras van en `f32` con la escala de `i16`, igual que en RNNoise.

use core::f32::consts::PI;

/// Fondo de escala de las muestras.
const FULL_SCALE: f32 = 32_767.0;

/// Filtro de segundo orden (forma directa II traspuesta).
#[derive(Debug, Clone, Copy)]
pub struct Biquad {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z1: f32,
    z2: f32,
}

impl Biquad {
    /// Pasa-altos Butterworth con corte en `hz`.
    #[must_use]
    pub fn high_pass(sample_rate: u32, hz: f32) -> Self {
        let w = 2.0 * PI * hz / sample_rate as f32;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * core::f32::consts::FRAC_1_SQRT_2);
        let a0 = 1.0 + alpha;
        Self {
            b0: (1.0 + cos) / 2.0 / a0,
            b1: -(1.0 + cos) / a0,
            b2: (1.0 + cos) / 2.0 / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Campana que sube (o baja) `gain_db` alrededor de `hz`. `q` es lo
    /// estrecha que es: con valores por debajo de 1 el realce es ancho y
    /// suave.
    #[must_use]
    pub fn peaking(sample_rate: u32, hz: f32, q: f32, gain_db: f32) -> Self {
        let a = 10f32.powf(gain_db / 40.0);
        let w = 2.0 * PI * hz / sample_rate as f32;
        let (sin, cos) = w.sin_cos();
        let alpha = sin / (2.0 * q);
        let a0 = 1.0 + alpha / a;
        Self {
            b0: alpha.mul_add(a, 1.0) / a0,
            b1: -2.0 * cos / a0,
            b2: alpha.mul_add(-a, 1.0) / a0,
            a1: -2.0 * cos / a0,
            a2: (1.0 - alpha / a) / a0,
            z1: 0.0,
            z2: 0.0,
        }
    }

    /// Filtra una muestra.
    #[inline]
    pub fn step(&mut self, x: f32) -> f32 {
        let y = self.b0.mul_add(x, self.z1);
        self.z1 = self.a1.mul_add(-y, self.b1.mul_add(x, self.z2));
        self.z2 = self.a2.mul_add(-y, self.b2 * x);
        y
    }

    /// Filtra `pcm` en el sitio.
    pub fn process(&mut self, pcm: &mut [i16]) {
        for s in pcm {
            *s = self.step(f32::from(*s)).clamp(-32_768.0, FULL_SCALE) as i16;
        }
    }
}

/// Nivel RMS al que se lleva la voz: −20 dBFS. Deja unos 14 dB para los picos
/// de una voz normal antes de llegar al limitador.
const LEVEL_TARGET: f32 = FULL_SCALE * 0.1;

/// Cuánto puede subir una voz baja: +15 dB. Más allá lo que se sube es, sobre
/// todo, el ruido del micrófono.
const LEVEL_MAX_GAIN: f32 = 5.6;

/// Cuánto puede bajar una voz alta: −12 dB.
const LEVEL_MIN_GAIN: f32 = 0.25;

/// Fracción del camino hacia el nivel medido que se recorre en cada bloque de
/// 10 ms. Bajar (voz más fuerte de lo previsto) es rápido para que no sature;
/// subir es lento para que no se oiga trabajar.
const LEVEL_ATTACK: f32 = 0.20;
const LEVEL_RELEASE: f32 = 0.02;

/// Por debajo de este RMS un bloque no cuenta como voz aunque lo parezca: es
/// el suelo de ruido de un micrófono.
const LEVEL_FLOOR: f32 = 60.0;

/// Donde el limitador empieza a redondear y de donde no deja pasar.
const LIMIT_KNEE: f32 = FULL_SCALE * 0.70;
const LIMIT_CEILING: f32 = FULL_SCALE * 0.97;

/// Lleva la voz a un nivel estable y acota los picos.
#[derive(Debug, Clone, Copy)]
pub struct Leveler {
    /// Nivel RMS estimado de la voz de quien habla.
    level: f32,
    /// Ganancia con la que terminó el bloque anterior.
    gain: f32,
}

impl Default for Leveler {
    fn default() -> Self {
        Self::new()
    }
}

impl Leveler {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            level: LEVEL_TARGET,
            gain: 1.0,
        }
    }

    /// Ganancia que se está aplicando ahora.
    #[must_use]
    pub const fn gain(&self) -> f32 {
        self.gain
    }

    /// Procesa un bloque en el sitio. `voiced` dice si en él hay voz: sólo
    /// entonces se actualiza el nivel.
    pub fn process(&mut self, block: &mut [f32], voiced: bool) {
        if block.is_empty() {
            return;
        }
        if voiced {
            let energy: f32 = block.iter().map(|s| s * s).sum();
            let rms = (energy / block.len() as f32).sqrt();
            if rms > LEVEL_FLOOR {
                let rate = if rms > self.level {
                    LEVEL_ATTACK
                } else {
                    LEVEL_RELEASE
                };
                self.level = (rms - self.level).mul_add(rate, self.level);
            }
        }
        let target = (LEVEL_TARGET / self.level).clamp(LEVEL_MIN_GAIN, LEVEL_MAX_GAIN);
        // La ganancia cambia en rampa dentro del bloque: un salto de golpe
        // entre bloques se oiría como un chasquido.
        let step = (target - self.gain) / block.len() as f32;
        for s in block {
            self.gain += step;
            *s = limit(*s * self.gain);
        }
        self.gain = target;
    }
}

/// Deja pasar intacto lo que queda bajo la rodilla y redondea lo que la
/// supera, sin llegar nunca al techo.
#[inline]
fn limit(x: f32) -> f32 {
    let mag = x.abs();
    if mag <= LIMIT_KNEE {
        return x;
    }
    let room = LIMIT_CEILING - LIMIT_KNEE;
    room.mul_add(((mag - LIMIT_KNEE) / room).tanh(), LIMIT_KNEE)
        .copysign(x)
}

/// Ajustes de todo el camino de la voz.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VoiceTuning {
    /// Quitar lo que hay por debajo de la voz antes de nada.
    pub high_pass: bool,
    /// Cuánto de la señal sin ruido se usa: `0.0` no reduce ruido, `1.0` deja
    /// sólo lo que RNNoise da por voz. Valores intermedios suenan más
    /// naturales a costa de dejar algo de fondo.
    pub denoise_mix: f32,
    /// Probabilidad de voz a partir de la cual un bloque se da por hablado.
    pub gate_threshold: f32,
    /// Bloques de 10 ms que la puerta sigue abierta tras la última voz.
    pub gate_hangover: u32,
    /// Ganancia con la puerta cerrada.
    pub gate_floor: f32,
    /// Nivelar la voz y acotar los picos.
    pub leveler: bool,
    /// Tope de la tasa de bits.
    pub bitrate: u32,
    /// `true`: cada paquete lleva siempre copia del anterior. `false`: sólo
    /// cuando la red está perdiendo paquetes.
    pub fec_always: bool,
    /// No transmitir los silencios dentro de un turno.
    pub dtx: bool,
    /// Realce de presencia al reproducir, en dB (`0.0` lo apaga).
    pub presence_db: f32,
}

impl VoiceTuning {
    /// Lo que hacía el códec antes de que hubiera perfiles: reducción de
    /// ruido completa, sin nivelar, con copia de respaldo en cada paquete.
    /// Es con lo que arranca un [`VoiceEncoder`](crate::VoiceEncoder).
    pub const LEGACY: Self = Self {
        high_pass: false,
        denoise_mix: 1.0,
        gate_threshold: 0.5,
        gate_hangover: 30,
        gate_floor: 0.12,
        leveler: false,
        bitrate: crate::voice::MAX_BITRATE,
        fec_always: true,
        dtx: false,
        presence_db: 0.0,
    };
}

/// Perfiles de voz: cada uno es un equilibrio distinto entre naturalidad,
/// limpieza y datos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum VoiceProfile {
    /// Sin tratamiento. Para tonos de prueba y medidas: cualquier etapa de
    /// voz alteraría una señal que no lo es.
    Raw = 0,
    /// La voz más natural: poco filtrado y la tasa más alta.
    Clear = 1,
    /// Como una buena llamada: limpia y con la mitad de datos.
    Call = 2,
    /// Para sitios ruidosos: se prioriza que se entienda.
    Noisy = 3,
    /// El mínimo de datos, para coberturas malas.
    Light = 4,
}

impl VoiceProfile {
    /// Perfil con ese código, o `None` si no existe.
    #[must_use]
    pub const fn from_code(code: u8) -> Option<Self> {
        Some(match code {
            0 => Self::Raw,
            1 => Self::Clear,
            2 => Self::Call,
            3 => Self::Noisy,
            4 => Self::Light,
            _ => return None,
        })
    }

    /// Los ajustes del perfil.
    #[must_use]
    pub const fn tuning(self) -> VoiceTuning {
        let voice = VoiceTuning {
            high_pass: true,
            denoise_mix: 0.9,
            gate_threshold: 0.5,
            gate_hangover: 30,
            gate_floor: 0.12,
            leveler: true,
            bitrate: 16_000,
            fec_always: false,
            // Medido con voz: las pausas de un turno son un 4–6 % de los
            // frames, y a cambio el otro extremo oye un silencio absoluto en
            // ellas. Sólo compensa en el perfil que lo sacrifica todo a los
            // datos.
            dtx: false,
            presence_db: 2.5,
        };
        match self {
            Self::Raw => VoiceTuning {
                denoise_mix: 0.0,
                ..VoiceTuning::LEGACY
            },
            Self::Clear => VoiceTuning {
                denoise_mix: 0.75,
                gate_floor: 0.25,
                bitrate: 32_000,
                ..voice
            },
            Self::Call => voice,
            Self::Noisy => VoiceTuning {
                denoise_mix: 1.0,
                gate_threshold: 0.6,
                gate_hangover: 20,
                gate_floor: 0.05,
                bitrate: 24_000,
                presence_db: 3.0,
                ..voice
            },
            Self::Light => VoiceTuning {
                bitrate: 12_000,
                dtx: true,
                ..voice
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;

    /// RMS de `hz` tras pasar por `filter`, con una entrada de RMS 1000.
    fn response(mut filter: Biquad, hz: f32) -> f32 {
        let amp = 1000.0 * core::f32::consts::SQRT_2;
        let out: Vec<f32> = (0..RATE as usize)
            .map(|i| filter.step(amp * (2.0 * PI * hz * i as f32 / RATE as f32).sin()))
            .collect();
        // El primer tramo es el transitorio del filtro.
        let tail = &out[RATE as usize / 2..];
        (tail.iter().map(|s| s * s).sum::<f32>() / tail.len() as f32).sqrt()
    }

    fn db(ratio: f32) -> f32 {
        20.0 * ratio.log10()
    }

    #[test]
    fn el_pasa_altos_quita_el_zumbido_y_respeta_la_voz() {
        let hp = Biquad::high_pass(RATE, 90.0);
        assert!(db(response(hp, 30.0) / 1000.0) < -15.0);
        assert!(db(response(hp, 50.0) / 1000.0) < -8.0);
        assert!(db(response(hp, 300.0) / 1000.0).abs() < 0.5);
        assert!(db(response(hp, 1000.0) / 1000.0).abs() < 0.1);
    }

    #[test]
    fn la_presencia_sube_las_consonantes_y_no_los_graves() {
        let eq = Biquad::peaking(RATE, 3000.0, 0.9, 2.5);
        let at = |hz| db(response(eq, hz) / 1000.0);
        assert!((at(3000.0) - 2.5).abs() < 0.2, "3 kHz: {}", at(3000.0));
        assert!(at(300.0).abs() < 0.3, "300 Hz: {}", at(300.0));
        assert!(at(12_000.0).abs() < 0.6, "12 kHz: {}", at(12_000.0));
    }

    fn block(rms: f32, phase: &mut f32) -> [f32; 480] {
        let mut out = [0.0; 480];
        for s in &mut out {
            *s = rms * core::f32::consts::SQRT_2 * phase.sin();
            *phase += 2.0 * PI * 220.0 / RATE as f32;
        }
        out
    }

    fn rms(samples: &[f32]) -> f32 {
        (samples.iter().map(|s| s * s).sum::<f32>() / samples.len() as f32).sqrt()
    }

    #[test]
    fn una_voz_baja_sube_hasta_el_nivel_objetivo() {
        let mut leveler = Leveler::new();
        let mut phase = 0.0;
        let mut last = 0.0;
        // Dos segundos de voz a −32 dBFS.
        for _ in 0..200 {
            let mut b = block(820.0, &mut phase);
            leveler.process(&mut b, true);
            last = rms(&b);
        }
        assert!(db(last / LEVEL_TARGET).abs() < 2.0, "quedó en {last}");
    }

    #[test]
    fn una_voz_alta_baja_y_nunca_pasa_del_techo() {
        let mut leveler = Leveler::new();
        let mut phase = 0.0;
        let mut peak = 0f32;
        for _ in 0..100 {
            // Fondo de escala: lo peor que puede entrar.
            let mut b = block(FULL_SCALE / core::f32::consts::SQRT_2, &mut phase);
            leveler.process(&mut b, true);
            peak = b.iter().fold(peak, |m, s| m.max(s.abs()));
        }
        assert!(peak <= LIMIT_CEILING, "pico {peak}");
        assert!(leveler.gain() < 0.5);
    }

    #[test]
    fn en_las_pausas_no_se_sube_el_ruido() {
        let mut leveler = Leveler::new();
        let mut phase = 0.0;
        for _ in 0..100 {
            let mut b = block(3000.0, &mut phase);
            leveler.process(&mut b, true);
        }
        let talking = leveler.gain();
        // Tres segundos de fondo flojo sin voz: la ganancia no se mueve.
        for _ in 0..300 {
            let mut b = block(100.0, &mut phase);
            leveler.process(&mut b, false);
        }
        assert!((leveler.gain() - talking).abs() < 1e-3);
    }

    #[test]
    fn el_limitador_no_toca_lo_que_no_satura() {
        assert_eq!(limit(1000.0), 1000.0);
        assert_eq!(limit(-LIMIT_KNEE), -LIMIT_KNEE);
        assert!(limit(1.0e6) <= LIMIT_CEILING);
        assert!(limit(-1.0e6) >= -LIMIT_CEILING);
    }

    #[test]
    fn cada_perfil_gasta_menos_que_el_anterior_salvo_el_ruidoso() {
        let rate = |p: VoiceProfile| p.tuning().bitrate;
        assert!(rate(VoiceProfile::Clear) > rate(VoiceProfile::Call));
        assert!(rate(VoiceProfile::Call) > rate(VoiceProfile::Light));
        assert!(rate(VoiceProfile::Noisy) > rate(VoiceProfile::Call));
        // El crudo no trata la voz de ninguna manera.
        let raw = VoiceProfile::Raw.tuning();
        assert!(!raw.high_pass && !raw.leveler && raw.denoise_mix == 0.0);
        assert_eq!(VoiceProfile::from_code(4), Some(VoiceProfile::Light));
        assert_eq!(VoiceProfile::from_code(9), None);
    }
}
