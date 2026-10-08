//! Limitador de tasa fixed-window por dirección (IP).
//!
//! Protección simple contra inundación (DoS) del relay: cada dirección tiene
//! un presupuesto de paquetes por ventana de 1 s. Superado el límite, los
//! paquetes se descartan hasta la siguiente ventana.
//!
//! El estado vive en un `DashMap` y se limpia perezosamente contra un
//! umbral de entrada máximo (`EVICT_THRESHOLD`) + una pasada de GC en cada
//! `allow()` (sin locks globales: solo un `retain` periódico).

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Máximo de entradas antes de forzar una limpieza de ventanas viejas.
const EVICT_THRESHOLD: usize = 10_000;
/// Caducidad de una ventana sin tráfico.
const WINDOW_TTL: Duration = Duration::from_secs(10);

#[derive(Debug)]
struct Window {
    started: Instant,
    count: u64,
}

/// Limitador fixed-window (1 s) por IP.
///
/// `limit_per_sec: 0` desactiva el límite (deny-all sería absurdo; se trata
/// como no limitador).
#[derive(Debug, Default)]
pub struct RateLimiter {
    limit_per_sec: u64,
    state: Mutex<HashMap<IpAddr, Window>>,
}

impl RateLimiter {
    pub fn new(limit_per_sec: u64) -> Self {
        Self {
            limit_per_sec,
            state: Mutex::new(HashMap::new()),
        }
    }

    /// Devuelve `true` si la dirección puede enviar otro paquete.
    pub fn allow(&self, from: IpAddr) -> bool {
        if self.limit_per_sec == 0 {
            return true;
        }
        let mut map = self.state.lock().unwrap_or_else(|p| p.into_inner());

        if map.len() >= EVICT_THRESHOLD {
            let now = Instant::now();
            map.retain(|_, w| now.duration_since(w.started) < WINDOW_TTL);
        }

        let now = Instant::now();
        match map.get_mut(&from) {
            Some(win) => {
                if now.duration_since(win.started) >= Duration::from_secs(1) {
                    win.started = now;
                    win.count = 1;
                    true
                } else if win.count < self.limit_per_sec {
                    win.count += 1;
                    true
                } else {
                    false
                }
            }
            None => {
                map.insert(
                    from,
                    Window {
                        started: now,
                        count: 1,
                    },
                );
                true
            }
        }
    }

    /// Entradas actualmente trackeadas (diagnóstico/tests).
    pub fn tracked(&self) -> usize {
        self.state.lock().unwrap_or_else(|p| p.into_inner()).len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_limit_disables() {
        let rl = RateLimiter::new(0);
        for _ in 0..10_000 {
            assert!(rl.allow(IpAddr::from([10, 0, 0, 1])));
        }
    }

    #[test]
    fn allows_burst_up_to_limit_then_blocks() {
        let rl = RateLimiter::new(3);
        let ip = IpAddr::from([10, 0, 0, 2]);
        assert!(rl.allow(ip));
        assert!(rl.allow(ip));
        assert!(rl.allow(ip));
        assert!(
            !rl.allow(ip),
            "4º paquete en la misma ventana debe bloquearse"
        );
        assert!(!rl.allow(ip));
    }

    #[test]
    fn different_ips_are_independent() {
        let rl = RateLimiter::new(2);
        let a = IpAddr::from([10, 0, 0, 3]);
        let b = IpAddr::from([10, 0, 0, 4]);
        assert!(rl.allow(a));
        assert!(rl.allow(a));
        assert!(!rl.allow(a));
        assert!(rl.allow(b)); // b tiene su propio presupuesto
        assert!(rl.allow(b));
        assert_eq!(rl.tracked(), 2);
    }

    #[test]
    fn window_resets_after_one_second() {
        let rl = RateLimiter::new(1);
        let ip = IpAddr::from([10, 0, 0, 5]);
        assert!(rl.allow(ip));
        assert!(!rl.allow(ip));

        // Manually fast-forward la ventana insertando una con timestamp viejo.
        {
            let mut map = rl.state.lock().unwrap();
            let w = map.get_mut(&ip).unwrap();
            w.started = Instant::now() - Duration::from_secs(2);
            w.count = 0;
            drop(map);
        }
        assert!(rl.allow(ip), "nueva ventana debe volver a permitir");
    }

    #[test]
    fn evicts_old_windows_at_threshold() {
        let rl = RateLimiter::new(10);
        // Llenar hasta el umbral con IPs frescas.
        for i in 0..EVICT_THRESHOLD {
            let ip = IpAddr::from([10, 0, (i >> 8) as u8, (i & 0xff) as u8]);
            assert!(rl.allow(ip));
        }
        assert_eq!(rl.tracked(), EVICT_THRESHOLD);

        // Envejecer la mitad de las ventanas.
        {
            let mut map = rl.state.lock().unwrap();
            for (n, w) in map.values_mut().enumerate() {
                if n % 2 == 0 {
                    w.started = Instant::now() - WINDOW_TTL - Duration::from_secs(1);
                }
            }
        }

        // Un allow adicional dispara el retain y poda las viejas.
        assert!(rl.allow(IpAddr::from([10, 99, 0, 1])));
        let tracked = rl.tracked();
        assert!(
            tracked < EVICT_THRESHOLD,
            "debe podar entradas viejas (tracked={tracked})"
        );
    }
}
