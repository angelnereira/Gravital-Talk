//! Estimación de MOS-LQ (Mean Opinion Score, listening quality).
//!
//! Usa el modelo E simplificado de ITU-T G.107. Partimos de un R-factor
//! base de 93 (buena calidad sin degradación), restamos penalizaciones por
//! RTT, jitter y pérdida, y mapeamos a MOS 1-5.

/// Mapa R-factor → MOS (ITU-T G.107 §B.3).
fn r_to_mos(r: f32) -> f32 {
    if r < 0.0 {
        1.0
    } else if r > 100.0 {
        4.5
    } else {
        (r * (r - 60.0) * (100.0 - r)).mul_add(7.0e-6, 0.035f32.mul_add(r, 1.0))
    }
}

/// Estimación de MOS-LQ desde métricas de red.
#[must_use]
pub fn estimate_mos(rtt_ms: f32, loss_percent: f32, jitter_ms: f32) -> f32 {
    // R-factor base.
    let mut r: f32 = 93.0;

    // Penalización por latencia (Id en G.107). One-way delay ≈ RTT/2.
    let one_way = rtt_ms / 2.0;
    if one_way > 160.0 {
        r -= 0.11f32.mul_add(one_way - 177.3, 0.024 * one_way);
    } else {
        r = 0.024f32.mul_add(-one_way, r);
    }

    // Penalización por pérdida (Ie-eff simplificado).
    // Opus es robusto: caída de ~0.5 MOS por 5% pérdida.
    r = loss_percent.mul_add(-2.5, r);

    // Penalización por jitter (no está en G.107; aproximamos).
    if jitter_ms > 30.0 {
        r = (jitter_ms - 30.0).mul_add(-0.5, r);
    }

    r_to_mos(r).clamp(1.0, 5.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pristine_network_high_mos() {
        let mos = estimate_mos(5.0, 0.0, 1.0);
        assert!(mos > 4.0, "expected high MOS, got {mos}");
    }

    #[test]
    fn high_loss_degrades_mos() {
        let good = estimate_mos(10.0, 0.0, 2.0);
        let bad = estimate_mos(10.0, 20.0, 2.0);
        assert!(bad < good);
    }

    #[test]
    fn high_rtt_degrades_mos() {
        let good = estimate_mos(5.0, 0.0, 1.0);
        let bad = estimate_mos(500.0, 0.0, 1.0);
        assert!(bad < good);
    }

    #[test]
    fn high_jitter_degrades_mos() {
        let good = estimate_mos(10.0, 0.0, 5.0);
        let bad = estimate_mos(10.0, 0.0, 100.0);
        assert!(bad < good);
    }

    #[test]
    fn mos_bounded() {
        assert!(estimate_mos(10000.0, 100.0, 1000.0) >= 1.0);
        assert!(estimate_mos(0.0, 0.0, 0.0) <= 5.0);
    }
}
