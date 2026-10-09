//! Verificación de audio bidireccional con el códec Opus.
//!
//! Requiere la feature `opus` (y `libopus` en el sistema):
//!
//! ```bash
//! cargo test -p gravital-talk-audit --features opus
//! ```
//!
//! Opus sí admite frames de 20 ms (a 64 kbps ocupan ~160 bytes, muy por debajo
//! del tope de 1176 B del protocolo), así que este test también cubre el tamaño
//! de frame que el README documenta. El análisis del audio recibido es
//! tolerante: Opus es un códec con pérdida, así que la frecuencia medida puede
//! desviarse un poco respecto de la fuente.

#![cfg(feature = "opus")]

use std::net::SocketAddr;

use gravital_talk::CodecId;
use gravital_talk_audit::harness::{run_pair, PairOptions, PeerRole};

/// Opus es con pérdida, así que la tolerancia de frecuencia es mayor que en PCM.
const OPUS_FREQ_TOLERANCE_HZ: f64 = 20.0;

/// El codec debe negotiating como Opus en ambas direcciones.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn opus_audio_flows_in_both_directions() {
    let mut opts = PairOptions {
        scenario: "opus".to_string(),
        host_bind: SocketAddr::from(([127, 0, 0, 1], 34_500)),
        codec: CodecId::Opus,
        duration_ms: 4_000,
        // 20 ms: el tamaño de frame que documenta el README.
        frame_duration_ms: 20,
        verify_audio: true,
        freq_tolerance_hz: OPUS_FREQ_TOLERANCE_HZ,
        min_rms: 0.005,
        expect_rx_frames_each: Some(5),
        expect_peer_ptt_activations: Some(1),
        ..PairOptions::default()
    };
    opts.with_split_turns();

    let outcomes = run_pair(&opts).await.expect("run_pair must not fail");
    assert_eq!(outcomes.len(), 2, "se esperaban host y join");

    for o in &outcomes {
        assert!(o.passed(), "{} fallo: {}", o.label, o.failures.join("; "));
        assert_eq!(o.codec, "Opus", "el códec negociado debe ser Opus");
        assert!(o.stats.tx_frames > 0, "{} no envió audio", o.label);
        assert!(
            o.stats.rx_frames > 0,
            "{} no recibió audio en la dirección contraria",
            o.label
        );
        assert!(o.audio.samples > 0, "{} no recibió muestras", o.label);
        assert!(
            (o.audio.freq_hz - opts.freq_hz).abs() < OPUS_FREQ_TOLERANCE_HZ,
            "{} midió {:.1} Hz y esperaba {:.1} Hz",
            o.label,
            o.audio.freq_hz,
            opts.freq_hz
        );
        assert_eq!(
            o.stats.send_errors, 0,
            "{} tuvo errores de envío con Opus",
            o.label
        );
    }

    let host = outcomes.iter().find(|o| o.role == PeerRole::Host).unwrap();
    let join = outcomes.iter().find(|o| o.role == PeerRole::Join).unwrap();
    assert!(
        host.stats.rx_frames > 0,
        "el host no recibió audio del join"
    );
    assert!(
        join.stats.rx_frames > 0,
        "el join no recibió audio del host"
    );
}
