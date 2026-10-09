//! Verificación real de audio bidireccional sobre UDP loopback.
//!
//! Estos tests son la respuesta operativa a la pregunta "¿esto funciona o es
//! un concepto?": abren dos sesiones reales del protocolo en el mismo proceso,
//! transmiten audio en las dos direcciones con turnos disjuntos y comprueban
//! que lo recibido es el tono esperado.
//!
//! No necesitan `libopus` ni ALSA: el códec por defecto es PCM, así que corren
//! en cualquier máquina y también en el job `test-no-default-features` de CI.

use std::net::SocketAddr;

use gravital_talk_audit::harness::{
    pcm_frame_duration_ms, run_multi_pair, run_pair, PairOptions, PeerRole,
};

fn base_opts(port: u16, duration_ms: u64) -> PairOptions {
    PairOptions {
        scenario: "p2p".to_string(),
        host_bind: SocketAddr::from(([127, 0, 0, 1], port)),
        duration_ms,
        // 10 ms: es el frame PCM mas largo que cabe en el tope del core.
        frame_duration_ms: pcm_frame_duration_ms(),
        verify_audio: true,
        freq_tolerance_hz: 8.0,
        ..PairOptions::default()
    }
}

fn assert_all_passed(outcomes: &[gravital_talk_audit::PeerOutcome]) {
    for o in outcomes {
        assert!(
            o.passed(),
            "peer {} fallo: {}",
            o.label,
            o.failures.join("; ")
        );
    }
}

/// Audio en las DOS direcciones: el host habla la primera mitad de la
/// ejecución, el join la segunda, y cada uno debe recibir audio del otro.
///
/// Esto es lo que hoy no cubre `make docker-e2e`: allí el host nunca asserta
/// su dirección de recepción.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn audio_flows_in_both_directions() {
    let mut opts = base_opts(34_100, 4_000);
    opts.with_split_turns();
    opts.expect_rx_frames_each = Some(5);
    opts.expect_peer_ptt_activations = Some(1);

    let outcomes = run_pair(&opts)
        .await
        .expect("run_pair must not fail at the transport level");
    assert_all_passed(&outcomes);
    assert_eq!(outcomes.len(), 2);

    let host = outcomes
        .iter()
        .find(|o| o.role == PeerRole::Host)
        .expect("host outcome");
    let join = outcomes
        .iter()
        .find(|o| o.role == PeerRole::Join)
        .expect("join outcome");

    // Cada lado envió durante su ventana y recibió durante la del otro.
    assert!(host.stats.tx_frames > 0, "el host no envio audio");
    assert!(join.stats.tx_frames > 0, "el join no envio audio");
    assert!(
        host.stats.rx_frames > 0,
        "el host no recibio audio del join (direccion join->host sin verificar)"
    );
    assert!(
        join.stats.rx_frames > 0,
        "el join no recibio audio del host"
    );

    // Y el audio recibido es el tono correcto, no ruido ni silencio.
    host.audio
        .expect_tone(opts.freq_hz, opts.freq_tolerance_hz, opts.min_rms)
        .unwrap_or_else(|e| panic!("audio invalido en el host: {e}"));
    join.audio
        .expect_tone(opts.freq_hz, opts.freq_tolerance_hz, opts.min_rms)
        .unwrap_or_else(|e| panic!("audio invalido en el join: {e}"));
}

/// El contenido del audio es correcto y no sólo "llegaron bytes": los dos
/// lados midieron 440 Hz con el RMS esperado del tono de prueba.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn received_audio_is_the_expected_tone() {
    let mut opts = base_opts(34_200, 3_000);
    opts.expect_rx_frames_each = Some(5);

    let outcomes = run_pair(&opts).await.expect("pair failed");
    assert_all_passed(&outcomes);

    for o in &outcomes {
        assert!(o.audio.samples > 0, "{} no recibio muestras", o.label);
        assert!(
            !o.audio.silent,
            "{} recibio audio silencioso (rms={})",
            o.label, o.audio.rms
        );
        assert!(
            (o.audio.freq_hz - 440.0).abs() < 8.0,
            "{} midio {:.1} Hz en vez de 440 Hz",
            o.label,
            o.audio.freq_hz
        );
        // Un tono puro no satura y conserva la amplitud de la fuente (~0.49).
        assert!(
            o.audio.peak > 0.4 && o.audio.peak <= 1.0,
            "{} con pico {} fuera de rango",
            o.label,
            o.audio.peak
        );
        // Un error de envio es siempre un fallo. En recepción hay un caso
        // normal: el CLOSE del peer al terminar se manifiesta como un error,
        // asi que sólo se exige que no haya mas de uno.
        assert_eq!(
            o.stats.send_errors, 0,
            "{} acumulo errores de envio",
            o.label
        );
        assert!(
            o.stats.recv_errors <= 1,
            "{} acumulo {} errores de recepcion (solo se espera el CLOSE final)",
            o.label,
            o.stats.recv_errors
        );
    }
}

/// N sesiones P2P simultáneas, cada una con una frecuencia distinta.
///
/// Si el tráfico se mezclara entre sesiones, algún par mediría la frecuencia
/// de otro: por eso cada par se verifica contra *su* tono y no contra un
/// valor genérico.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn parallel_sessions_do_not_cross_talk() {
    let n = 3;
    let mut opts = base_opts(34_300, 4_000);
    opts.expect_rx_frames_each = Some(5);

    let outcomes = run_multi_pair(&opts, n)
        .await
        .expect("run_multi_pair must not fail");
    assert_all_passed(&outcomes);
    assert_eq!(outcomes.len(), n * 2, "se esperaban {n} pares");

    let step = gravital_talk_audit::harness::DEFAULT_FREQ_STEP_HZ;
    let base = opts.freq_hz;
    for o in &outcomes {
        let index: usize = o
            .label
            .split('#')
            .nth(1)
            .and_then(|s| s.split('/').next())
            .and_then(|s| s.parse().ok())
            .unwrap_or_else(|| panic!("no se pudo parsear el indice de {}", o.label));
        let expected = step.mul_add(index as f64, base);

        assert!(o.stats.rx_frames > 0, "la sesion #{index} no recibio audio",);
        assert!(
            (o.audio.freq_hz - expected).abs() < 8.0,
            "la sesion #{index} recibio {:.1} Hz y esperaba {expected:.1} Hz: hay cruce de trafico",
            o.audio.freq_hz
        );
    }
}

/// Un par donde un lado nunca habla sigue siendo válido: el que habla debe
/// ser recibido, y el que sólo escucha no debe enviar audio.
///
/// Verifica que el plan de turnos se respeta en la dirección contraria al
/// error más común (enviar siempre).
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn turn_plan_is_respected_in_both_directions() {
    let mut opts = base_opts(34_400, 1_500);
    // El join no habla nunca; el host habla toda la ejecución.
    opts.host_turn = gravital_talk_audit::harness::TurnPlan::all();
    opts.join_turn = gravital_talk_audit::harness::TurnPlan::never();
    opts.expect_rx_frames_each = Some(5);

    let outcomes = run_pair(&opts).await.expect("pair failed");

    let host = outcomes
        .iter()
        .find(|o| o.role == PeerRole::Host)
        .expect("host");
    let join = outcomes
        .iter()
        .find(|o| o.role == PeerRole::Join)
        .expect("join");

    assert!(host.stats.tx_frames > 0, "el host debio hablar");
    assert_eq!(
        join.stats.tx_frames, 0,
        "el join no debio enviar audio con un plan de turnos vacio"
    );
    // El join recibe todo lo que hablo el host, asi que su tono es correcto.
    assert!(join.stats.rx_frames > 0, "el join no recibio audio");
    join.audio
        .expect_tone(opts.freq_hz, opts.freq_tolerance_hz, opts.min_rms)
        .expect("el join recibio un tono valido");

    // El host no recibio audio de nadie: por eso su analisis debe fallar si se
    // le exige un tono. Esto confirma que el criterio detecta ausencia de audio
    // en vez de darla por buena.
    assert!(
        host.audio
            .expect_tone(opts.freq_hz, opts.freq_tolerance_hz, opts.min_rms)
            .is_err(),
        "el host no debio haber recibido audio"
    );
}
