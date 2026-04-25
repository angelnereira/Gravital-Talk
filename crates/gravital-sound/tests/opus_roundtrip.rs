//! Integration test: sine 440 Hz through the full CodecSession pipeline.
//!
//! PCM roundtrip: validates exact sample-level SNR > 60 dB.
//! Opus roundtrip: validates that decoded signal has non-trivial energy
//!   (opus has algorithmic delay + network jitter that preclude exact sample
//!   alignment, so we only check that the decoder produces real output).

use std::sync::Arc;
use std::time::Duration;

use gravital_sound::{
    codec_session::CodecSessionError, CodecId, CodecSession, Config, SessionRole, Transport,
    UdpConfig, UdpTransport,
};

fn sine_samples(n: usize, sample_rate: u32) -> Vec<i16> {
    let step = 2.0 * std::f32::consts::PI * 440.0 / sample_rate as f32;
    let mut phase = 0.0f32;
    (0..n)
        .map(|_| {
            let s = (phase.sin() * 16_000.0) as i16;
            phase += step;
            if phase > std::f32::consts::TAU {
                phase -= std::f32::consts::TAU;
            }
            s
        })
        .collect()
}

fn snr_db(original: &[i16], recovered: &[i16]) -> f64 {
    let n = original.len().min(recovered.len());
    if n == 0 {
        return 0.0;
    }
    let signal_power: f64 = original[..n]
        .iter()
        .map(|&s| (s as f64).powi(2))
        .sum::<f64>()
        / n as f64;
    let noise_power: f64 = original[..n]
        .iter()
        .zip(recovered[..n].iter())
        .map(|(&a, &b)| (a as f64 - b as f64).powi(2))
        .sum::<f64>()
        / n as f64;
    if noise_power < 1e-12 {
        return 120.0;
    }
    10.0 * (signal_power / noise_power).log10()
}

fn energy(samples: &[i16]) -> f64 {
    samples.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / samples.len().max(1) as f64
}

async fn roundtrip(codec_id: CodecId, frames: usize) -> anyhow::Result<(Vec<i16>, Vec<i16>)> {
    let ts = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: "127.0.0.1:0".parse()?,
            ..Default::default()
        })
        .await?,
    );
    let tc = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: "127.0.0.1:0".parse()?,
            ..Default::default()
        })
        .await?,
    );

    let srv_local = ts.local_addr()?;
    let cli_local = tc.local_addr()?;

    let config = Config {
        frame_duration_ms: 10,
        ..Config::default()
    };
    let srv = Arc::new(CodecSession::new(ts, config.clone(), codec_id)?);
    let cli = Arc::new(CodecSession::new(tc, config.clone(), codec_id)?);

    let s = srv.clone();
    let hs_srv = tokio::spawn(async move { s.handshake(SessionRole::Server, cli_local).await });
    cli.handshake(SessionRole::Client, srv_local).await?;
    hs_srv.await??;

    let frame_samples = (config.sample_rate as usize * config.frame_duration_ms as usize) / 1000;
    let original = sine_samples(frame_samples * frames, config.sample_rate);

    let c = cli.clone();
    let original_clone = original.clone();
    let send_task = tokio::spawn(async move {
        for chunk in original_clone.chunks(frame_samples) {
            c.send_samples(chunk).await?;
            tokio::time::sleep(Duration::from_millis(config.frame_duration_ms as u64)).await;
        }
        anyhow::Ok(())
    });

    let mut recovered = Vec::with_capacity(original.len());
    for _ in 0..frames {
        match tokio::time::timeout(Duration::from_millis(500), srv.recv_samples()).await {
            Ok(Ok(s)) => recovered.extend_from_slice(&s),
            Ok(Err(e)) => return Err(e.into()),
            Err(_) => break,
        }
    }
    send_task.await??;
    cli.close().await?;
    srv.close().await?;

    Ok((original, recovered))
}

#[tokio::test(flavor = "multi_thread")]
async fn pcm_roundtrip_snr_above_60db() {
    let (original, recovered) = roundtrip(CodecId::Pcm, 50).await.unwrap();
    assert!(!recovered.is_empty(), "received no samples");
    let snr = snr_db(&original, &recovered);
    assert!(snr > 60.0, "PCM SNR {snr:.1} dB < 60 dB");
}

/// El client pide Opus, el server sólo soporta PCM. CodecSession del client
/// debe retornar `CodecMismatch` aunque el handshake del transport tenga éxito.
#[cfg(feature = "opus")]
#[tokio::test(flavor = "multi_thread")]
async fn codec_session_rejects_mismatch() {
    let ts = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    let tc = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            ..Default::default()
        })
        .await
        .unwrap(),
    );
    let srv_local = ts.local_addr().unwrap();
    let cli_local = tc.local_addr().unwrap();

    let server_cfg = Config {
        supported_codecs: vec![0x01], // sólo PCM
        ..Config::default()
    };
    let client_cfg = Config {
        codec_preferred: 0x02,
        supported_codecs: vec![0x01, 0x02], // acepta fallback
        ..Config::default()
    };

    let srv = Arc::new(CodecSession::new(ts, server_cfg, CodecId::Pcm).unwrap());
    let cli = Arc::new(CodecSession::new(tc, client_cfg, CodecId::Opus).unwrap());

    let s = srv.clone();
    let hs_srv = tokio::spawn(async move { s.handshake(SessionRole::Server, cli_local).await });
    let cli_result = cli.handshake(SessionRole::Client, srv_local).await;
    let _ = hs_srv.await;

    match cli_result {
        Err(CodecSessionError::CodecMismatch {
            requested,
            negotiated,
        }) => {
            assert_eq!(requested, CodecId::Opus);
            assert_eq!(negotiated, CodecId::Pcm);
        }
        other => panic!("expected CodecMismatch, got {other:?}"),
    }
}

#[cfg(feature = "opus")]
#[tokio::test(flavor = "multi_thread")]
async fn opus_roundtrip_produces_signal() {
    // Opus is lossy and introduces algorithmic delay; we verify that:
    // 1. We receive frames at all.
    // 2. The decoded signal has at least 10% of the original energy
    //    (not silence or all-zero PLC output).
    let (original, recovered) = roundtrip(CodecId::Opus, 80).await.unwrap();
    assert!(!recovered.is_empty(), "received no samples from Opus codec");

    let orig_energy = energy(&original);
    let recv_energy = energy(&recovered);
    let ratio = recv_energy / orig_energy.max(1.0);
    assert!(
        ratio > 0.1,
        "Opus output energy ({recv_energy:.0}) < 10% of original ({orig_energy:.0}): codec may be broken"
    );
}
