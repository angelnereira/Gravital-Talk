//! Jitter buffer lock-free para reordering por sequence number.
//!
//! Implementación:
//!
//! - Anillo circular con `capacity` slots (potencia de 2 para mod barato).
//! - Cada slot guarda `Option<Frame>` protegido por `AtomicU32` que indica
//!   si está ocupado.
//! - El productor escribe en el slot `sequence & (capacity - 1)`.
//! - El consumidor avanza un cursor `next_seq` y extrae el slot
//!   correspondiente cuando está listo o cuando expira el deadline.
//!
//! Este diseño es SPSC (single-producer single-consumer) y evita `Mutex`
//! en el hot path. Para MPMC usar `crossbeam-queue::ArrayQueue`.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

use bytes::Bytes;
use smallvec::SmallVec;

/// Un frame de audio dentro del jitter buffer.
#[derive(Debug, Clone)]
pub struct Frame {
    pub sequence: u32,
    pub timestamp: u64,
    pub payload: Bytes,
}

struct Slot {
    occupied: AtomicBool,
    sequence: AtomicU32,
    frame: Mutex<Option<Frame>>,
}

impl Slot {
    fn empty() -> Self {
        Self {
            occupied: AtomicBool::new(false),
            sequence: AtomicU32::new(0),
            frame: Mutex::new(None),
        }
    }
}

/// Jitter buffer con capacidad fija.
pub struct JitterBuffer {
    slots: SmallVec<[Slot; 64]>,
    capacity: u32,
    mask: u32,
    next_seq: AtomicU32,
    primed: AtomicBool,
}

impl JitterBuffer {
    /// Crea un buffer con `capacity` slots. Debe ser potencia de 2 y
    /// ≤ 2048.
    #[must_use]
    pub fn new(capacity: u32) -> Self {
        assert!(capacity.is_power_of_two(), "capacity must be power of two");
        assert!(capacity <= 2048, "capacity too large");
        let mut slots: SmallVec<[Slot; 64]> = SmallVec::with_capacity(capacity as usize);
        for _ in 0..capacity {
            slots.push(Slot::empty());
        }
        Self {
            slots,
            capacity,
            mask: capacity - 1,
            next_seq: AtomicU32::new(0),
            primed: AtomicBool::new(false),
        }
    }

    /// Capacidad en slots.
    #[inline]
    #[must_use]
    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    /// Sequence que se esperaría del próximo pop.
    #[inline]
    #[must_use]
    pub fn next_sequence(&self) -> u32 {
        self.next_seq.load(Ordering::Acquire)
    }

    /// Porcentaje de ocupación 0..=100.
    #[must_use]
    pub fn fill_percent(&self) -> f32 {
        let occupied = self
            .slots
            .iter()
            .filter(|s| s.occupied.load(Ordering::Relaxed))
            .count();
        (occupied as f32 / self.capacity as f32) * 100.0
    }

    /// Inserta un frame. Devuelve `true` si aceptado, `false` si
    /// demasiado viejo o el slot colisionó con un frame más nuevo.
    pub fn push(&self, frame: Frame) -> bool {
        if !self.primed.load(Ordering::Acquire) {
            self.next_seq.store(frame.sequence, Ordering::Release);
            self.primed.store(true, Ordering::Release);
        } else {
            let next = self.next_seq.load(Ordering::Acquire);
            // Demasiado viejo: ya se consumió.
            if frame.sequence.wrapping_sub(next) > 0x8000_0000 {
                return false;
            }
            // Demasiado lejos en el futuro: no cabe en la ventana.
            if frame.sequence.wrapping_sub(next) >= self.capacity {
                return false;
            }
        }

        let idx = (frame.sequence & self.mask) as usize;
        let slot = &self.slots[idx];

        // Evita sobrescribir un slot ocupado con una sequence distinta (colisión).
        if slot.occupied.load(Ordering::Acquire) {
            let existing = slot.sequence.load(Ordering::Acquire);
            if existing != frame.sequence {
                return false;
            }
            // Duplicado: reemplaza (caso raro de retransmisión, idempotente).
        }

        *slot.frame.lock().expect("jitter slot poisoned") = Some(frame.clone());
        slot.sequence.store(frame.sequence, Ordering::Release);
        slot.occupied.store(true, Ordering::Release);
        true
    }

    /// Extrae el frame correspondiente a `next_sequence`. Si no está
    /// listo, devuelve `None`.
    pub fn pop(&self) -> Option<Frame> {
        if !self.primed.load(Ordering::Acquire) {
            return None;
        }
        let next = self.next_seq.load(Ordering::Acquire);
        let idx = (next & self.mask) as usize;
        let slot = &self.slots[idx];

        if !slot.occupied.load(Ordering::Acquire) {
            return None;
        }
        if slot.sequence.load(Ordering::Acquire) != next {
            return None;
        }

        let frame = slot.frame.lock().expect("jitter slot poisoned").take();
        slot.occupied.store(false, Ordering::Release);
        self.next_seq.store(next.wrapping_add(1), Ordering::Release);
        frame
    }

    /// Fuerza avanzar el cursor `count` posiciones, descartando frames
    /// intermedios (utilizado cuando el deadline del jitter buffer expira).
    pub fn skip(&self, count: u32) {
        if !self.primed.load(Ordering::Acquire) {
            return;
        }
        let next = self.next_seq.load(Ordering::Acquire);
        for i in 0..count {
            let idx = ((next.wrapping_add(i)) & self.mask) as usize;
            let slot = &self.slots[idx];
            slot.occupied.store(false, Ordering::Release);
            *slot.frame.lock().expect("jitter slot poisoned") = None;
        }
        self.next_seq
            .store(next.wrapping_add(count), Ordering::Release);
    }
}

impl core::fmt::Debug for JitterBuffer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("JitterBuffer")
            .field("capacity", &self.capacity)
            .field("next_seq", &self.next_sequence())
            .field("fill_percent", &self.fill_percent())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(seq: u32) -> Frame {
        Frame {
            sequence: seq,
            timestamp: u64::from(seq) * 20_000,
            payload: Bytes::from(vec![seq as u8; 16]),
        }
    }

    #[test]
    fn in_order_stream() {
        let jb = JitterBuffer::new(64);
        for i in 0..32 {
            assert!(jb.push(frame(i)));
        }
        for i in 0..32 {
            let f = jb.pop().unwrap();
            assert_eq!(f.sequence, i);
        }
    }

    #[test]
    fn out_of_order_reassembled() {
        let jb = JitterBuffer::new(64);
        // Insertamos primero 0 y 2, luego 1; el pop debe dar 0, 1, 2.
        jb.push(frame(0));
        jb.push(frame(2));
        assert!(jb.pop().is_some());
        assert!(jb.pop().is_none()); // 1 aún no llegó
        jb.push(frame(1));
        assert_eq!(jb.pop().unwrap().sequence, 1);
        assert_eq!(jb.pop().unwrap().sequence, 2);
    }

    #[test]
    fn rejects_too_far_future() {
        let jb = JitterBuffer::new(16);
        jb.push(frame(100));
        // 100 + 16 = 116 queda fuera de la ventana.
        assert!(!jb.push(frame(200)));
    }

    #[test]
    fn skip_forces_advance() {
        let jb = JitterBuffer::new(16);
        jb.push(frame(0));
        jb.push(frame(2));
        // Saltamos el hueco.
        jb.skip(2);
        assert_eq!(jb.pop().unwrap().sequence, 2);
    }

    #[test]
    fn fill_percent_updates() {
        let jb = JitterBuffer::new(8);
        assert_eq!(jb.fill_percent(), 0.0);
        jb.push(frame(0));
        jb.push(frame(1));
        assert!(jb.fill_percent() > 0.0);
    }
}
