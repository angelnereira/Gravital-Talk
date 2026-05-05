//! FEC básico XOR para Gravital Talk (FASE 3).
//!
//! Algoritmo: parity de grupo — para cada ventana de `window` frames se
//! envía un paquete de paridad que es el XOR byte a byte de todos los
//! payloads del grupo.  Si exactamente un frame se pierde, el receptor
//! puede recuperarlo haciendo XOR del resto con la paridad.
//!
//! El encoder produce una `FecParity` cuando acumula `window` frames.
//! El decoder recibe los frames de datos y la paridad y devuelve el frame
//! recuperado (si hay exactamente un hueco).

use bytes::Bytes;

use gravital_talk_core::constants::FEC_WINDOW;

// ── Encoder ──────────────────────────────────────────────────────────────────

/// Encoder FEC: acumula frames y produce paquetes de paridad XOR.
#[derive(Debug)]
pub struct FecEncoder {
    /// Tamaño de la ventana (frames por grupo de paridad).
    window: u8,
    /// Acumulador XOR del grupo actual.
    parity_buf: Vec<u8>,
    /// Frames recibidos en el grupo actual (0..window).
    count: u8,
    /// Sequence del primer frame del grupo actual.
    seq_base: u32,
}

impl FecEncoder {
    /// Crea un encoder con el tamaño de ventana dado (`min 2`).
    #[must_use]
    pub fn new(window: u8) -> Self {
        let window = window.max(2);
        Self {
            window,
            parity_buf: Vec::new(),
            count: 0,
            seq_base: 0,
        }
    }

    /// Crea un encoder con la ventana por defecto del protocolo.
    #[must_use]
    pub fn with_default_window() -> Self {
        Self::new(FEC_WINDOW)
    }

    /// Añade un frame al grupo actual. Devuelve `Some(FecParity)` cuando se
    /// completa la ventana; `None` en caso contrario.
    pub fn push(&mut self, sequence: u32, payload: &[u8]) -> Option<FecParity> {
        if self.count == 0 {
            self.seq_base = sequence;
            self.parity_buf = vec![0u8; payload.len()];
        }

        // Extender el buffer de paridad si este frame es más largo.
        if payload.len() > self.parity_buf.len() {
            self.parity_buf.resize(payload.len(), 0);
        }

        xor_into(&mut self.parity_buf, payload);
        self.count += 1;

        if self.count >= self.window {
            let parity = FecParity {
                seq_base: self.seq_base,
                window: self.window,
                payload: Bytes::copy_from_slice(&self.parity_buf),
            };
            // Reiniciar estado para el siguiente grupo.
            self.count = 0;
            self.parity_buf.clear();
            Some(parity)
        } else {
            None
        }
    }
}

// ── Decoder ──────────────────────────────────────────────────────────────────

/// Decoder FEC: intenta recuperar un frame perdido cuando llega la paridad.
#[derive(Debug)]
pub struct FecDecoder {
    /// Tamaño de la ventana activa.
    window: u8,
    /// Sequence base del grupo activo.
    seq_base: u32,
    /// Frames recibidos del grupo activo (índice = offset desde seq_base).
    received: Vec<Option<Bytes>>,
    /// Paridad del grupo activo (si ya llegó).
    parity: Option<Bytes>,
    /// `true` si hay un grupo activo.
    active: bool,
}

impl FecDecoder {
    /// Crea un decoder para ventanas del tamaño indicado.
    #[must_use]
    pub fn new(window: u8) -> Self {
        let window = window.max(2);
        Self {
            window,
            seq_base: 0,
            received: Vec::new(),
            parity: None,
            active: false,
        }
    }

    /// Crea un decoder con la ventana por defecto del protocolo.
    #[must_use]
    pub fn with_default_window() -> Self {
        Self::new(FEC_WINDOW)
    }

    /// Registra un frame de datos recibido. Devuelve `true` si pertenece al
    /// grupo activo (o inicializa uno nuevo).
    pub fn push_data(&mut self, sequence: u32, payload: Bytes) -> bool {
        let window = self.window as u32;
        // Inferir seq_base alineando al límite de ventana.
        let group_base = sequence - (sequence % window);

        if !self.active || self.seq_base != group_base {
            self.start_group(group_base);
        }

        let offset = (sequence - self.seq_base) as usize;
        if offset < self.window as usize {
            self.received[offset] = Some(payload);
            true
        } else {
            false
        }
    }

    /// Registra un paquete de paridad. Devuelve `Some((seq, payload))` si se
    /// puede recuperar exactamente un frame perdido; `None` si no hay pérdida
    /// o hay más de un hueco.
    pub fn push_parity(&mut self, parity: FecParity) -> Option<(u32, Bytes)> {
        // Si la paridad es de un grupo distinto al activo, cambiar de grupo.
        if !self.active || self.seq_base != parity.seq_base {
            self.start_group(parity.seq_base);
        }
        self.parity = Some(parity.payload);
        self.try_recover()
    }

    /// Reinicia el estado del decoder (para cambios de sesión, etc.).
    pub fn reset(&mut self) {
        self.active = false;
        self.received.clear();
        self.parity = None;
    }

    // ── privados ──────────────────────────────────────────────────────────

    fn start_group(&mut self, seq_base: u32) {
        self.seq_base = seq_base;
        self.received = vec![None; self.window as usize];
        self.parity = None;
        self.active = true;
    }

    fn try_recover(&self) -> Option<(u32, Bytes)> {
        let parity = self.parity.as_ref()?;

        let missing: Vec<usize> = self
            .received
            .iter()
            .enumerate()
            .filter(|(_, f)| f.is_none())
            .map(|(i, _)| i)
            .collect();

        // Solo podemos recuperar exactamente un frame perdido.
        if missing.len() != 1 {
            return None;
        }
        let lost_idx = missing[0];

        // recovered = parity XOR (todos los demás frames presentes)
        let mut recovered = parity.to_vec();
        for (i, frame) in self.received.iter().enumerate() {
            if i == lost_idx {
                continue;
            }
            if let Some(data) = frame {
                xor_into(&mut recovered, data);
            }
        }

        let seq = self.seq_base.wrapping_add(lost_idx as u32);
        Some((seq, Bytes::from(recovered)))
    }
}

// ── Tipos públicos ────────────────────────────────────────────────────────────

/// Paquete de paridad FEC producido por el encoder.
#[derive(Debug, Clone)]
pub struct FecParity {
    /// Sequence del primer frame del grupo.
    pub seq_base: u32,
    /// Tamaño de la ventana.
    pub window: u8,
    /// Payload de paridad (XOR de todos los frames del grupo).
    pub payload: Bytes,
}

// ── Utilidades ────────────────────────────────────────────────────────────────

/// XOR byte a byte de `data` sobre `accumulator`. Si `data` es más corto,
/// solo se actualiza hasta `data.len()`.
#[inline]
fn xor_into(accumulator: &mut [u8], data: &[u8]) {
    let len = accumulator.len().min(data.len());
    for i in 0..len {
        accumulator[i] ^= data[i];
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_payload(seq: u32, size: usize) -> Vec<u8> {
        (0..size).map(|i| (seq as u8).wrapping_add(i as u8)).collect()
    }

    #[test]
    fn encoder_emits_parity_after_window() {
        let mut enc = FecEncoder::new(4);
        for i in 0..3 {
            assert!(enc.push(i, &make_payload(i, 16)).is_none());
        }
        let parity = enc.push(3, &make_payload(3, 16));
        assert!(parity.is_some());
        let p = parity.unwrap();
        assert_eq!(p.seq_base, 0);
        assert_eq!(p.window, 4);
        assert_eq!(p.payload.len(), 16);
    }

    #[test]
    fn encoder_second_window_resets() {
        let mut enc = FecEncoder::new(2);
        let p1 = enc.push(0, &[0x01, 0x02]);
        assert!(p1.is_none());
        let p2 = enc.push(1, &[0x03, 0x04]);
        assert!(p2.is_some());
        // Segundo grupo.
        let p3 = enc.push(2, &[0xAA]);
        assert!(p3.is_none());
        let p4 = enc.push(3, &[0xBB]);
        assert!(p4.is_some());
        let p = p4.unwrap();
        assert_eq!(p.seq_base, 2);
    }

    #[test]
    fn decoder_recovers_lost_frame() {
        let window = 4u8;
        let payloads: Vec<Vec<u8>> = (0..4).map(|i| make_payload(i, 16)).collect();

        // Encoder produce paridad.
        let mut enc = FecEncoder::new(window);
        let mut parity = None;
        for i in 0..4u32 {
            parity = enc.push(i, &payloads[i as usize]);
        }
        let parity = parity.unwrap();

        // Decoder recibe frames 0, 1, 3 (pierde frame 2).
        let mut dec = FecDecoder::new(window);
        dec.push_data(0, Bytes::from(payloads[0].clone()));
        dec.push_data(1, Bytes::from(payloads[1].clone()));
        dec.push_data(3, Bytes::from(payloads[3].clone()));

        let result = dec.push_parity(parity);
        assert!(result.is_some(), "debe recuperar el frame perdido");
        let (seq, recovered) = result.unwrap();
        assert_eq!(seq, 2);
        assert_eq!(recovered.as_ref(), payloads[2].as_slice());
    }

    #[test]
    fn decoder_no_recovery_with_two_losses() {
        let window = 4u8;
        let payloads: Vec<Vec<u8>> = (0..4).map(|i| make_payload(i, 8)).collect();

        let mut enc = FecEncoder::new(window);
        let mut parity = None;
        for i in 0..4u32 {
            parity = enc.push(i, &payloads[i as usize]);
        }
        let parity = parity.unwrap();

        let mut dec = FecDecoder::new(window);
        // Solo frames 0 y 1 llegan (pierden 2 y 3).
        dec.push_data(0, Bytes::from(payloads[0].clone()));
        dec.push_data(1, Bytes::from(payloads[1].clone()));

        let result = dec.push_parity(parity);
        assert!(result.is_none(), "no puede recuperar con dos pérdidas");
    }

    #[test]
    fn decoder_no_recovery_without_loss() {
        let window = 2u8;
        let payloads: Vec<Vec<u8>> = (0..2).map(|i| make_payload(i, 8)).collect();

        let mut enc = FecEncoder::new(window);
        let mut parity = None;
        for i in 0..2u32 {
            parity = enc.push(i, &payloads[i as usize]);
        }
        let parity = parity.unwrap();

        let mut dec = FecDecoder::new(window);
        dec.push_data(0, Bytes::from(payloads[0].clone()));
        dec.push_data(1, Bytes::from(payloads[1].clone()));

        let result = dec.push_parity(parity);
        assert!(result.is_none(), "sin pérdidas no devuelve frame recuperado");
    }

    #[test]
    fn xor_roundtrip() {
        let a = [0xDE, 0xAD, 0xBE, 0xEF];
        let b = [0xCA, 0xFE, 0xBA, 0xBE];
        let mut parity = [0u8; 4];
        xor_into(&mut parity, &a);
        xor_into(&mut parity, &b);
        // Recuperar a desde parity XOR b
        let mut recovered_a = parity;
        xor_into(&mut recovered_a, &b);
        assert_eq!(recovered_a, a);
    }
}
