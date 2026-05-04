//! Contadores monotónicos atómicos lock-free.

use core::sync::atomic::{AtomicU64, Ordering};

#[derive(Debug, Default)]
pub struct Counters {
    packets_sent: AtomicU64,
    packets_received: AtomicU64,
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    integrity_errors: AtomicU64,
}

impl Counters {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            packets_sent: AtomicU64::new(0),
            packets_received: AtomicU64::new(0),
            bytes_sent: AtomicU64::new(0),
            bytes_received: AtomicU64::new(0),
            integrity_errors: AtomicU64::new(0),
        }
    }

    #[inline]
    pub fn record_sent(&self, bytes: u64) {
        self.packets_sent.fetch_add(1, Ordering::Relaxed);
        self.bytes_sent.fetch_add(bytes, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_received(&self, bytes: u64) {
        self.packets_received.fetch_add(1, Ordering::Relaxed);
        self.bytes_received.fetch_add(bytes, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_integrity_error(&self) {
        self.integrity_errors.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    #[must_use]
    pub fn packets_sent(&self) -> u64 {
        self.packets_sent.load(Ordering::Relaxed)
    }

    #[inline]
    #[must_use]
    pub fn packets_received(&self) -> u64 {
        self.packets_received.load(Ordering::Relaxed)
    }

    #[inline]
    #[must_use]
    pub fn bytes_sent(&self) -> u64 {
        self.bytes_sent.load(Ordering::Relaxed)
    }

    #[inline]
    #[must_use]
    pub fn bytes_received(&self) -> u64 {
        self.bytes_received.load(Ordering::Relaxed)
    }

    #[inline]
    #[must_use]
    pub fn integrity_errors(&self) -> u64 {
        self.integrity_errors.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_start_at_zero() {
        let c = Counters::new();
        assert_eq!(c.packets_sent(), 0);
        assert_eq!(c.bytes_sent(), 0);
    }

    #[test]
    fn record_sent_increments() {
        let c = Counters::new();
        c.record_sent(100);
        c.record_sent(200);
        assert_eq!(c.packets_sent(), 2);
        assert_eq!(c.bytes_sent(), 300);
    }

    #[test]
    fn record_received_increments() {
        let c = Counters::new();
        c.record_received(50);
        assert_eq!(c.packets_received(), 1);
        assert_eq!(c.bytes_received(), 50);
    }
}
