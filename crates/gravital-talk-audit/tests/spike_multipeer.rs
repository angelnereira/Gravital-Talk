//! Spike S0: ¿cuántos peers puede oír realmente una sesión?
//!
//! Esto NO es un test que deba pasar: es una medición que decide el alcance
//! del escenario N-peers. El resultado esperado es desconocido, y por eso el
//! test imprime el hallazgo en vez de afirmar nada.
//!
//! Contexto: el relay soporta hasta `max_peers_per_session` (50 por defecto) y
//! difunde a todos los peers registrados. La duda es del lado del cliente:
//! `Session` guarda un único `peer: Mutex<Option<SocketAddr>>` y el handshake
//! filtra por esa dirección en cuatro sitios. Pero los cuatro filtros están en
//! la ruta del handshake, NO en la de audio: `recv_audio` descarta la dirección
//! de origen (`Ok(Ok((n, _)))`) y `dispatch_packet` no filtra por peer al
//! procesar audio.
//!
//! Eso sugiere que el audio entrante no se restringe al peer del handshake. La
//! medición comprueba si además el audio *sale* hacia todos: `send_audio` envía
//! al único `peer` conocido, así que en una topología de multidifusión real
//! (todos detrás de un relay) eso significaría 1-a-N y no N-a-N.
//!
//! Cómo se mide: un host (handshake abierto) y varios joins, todos contra el
//! mismo host. Cada join usa una frecuencia distinta para poder atribuir el
//! audio recibido a su emisor.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use gravital_talk::{
    CodecId, CodecSession, Config, SessionRole, Transport, UdpConfig, UdpTransport,
};
use gravital_talk_audit::analyze::AudioStats;
use gravital_talk_audit::harness::frame_samples;
use gravital_talk_audit::sine::sine_frames_i16;
use tokio::sync::Mutex;

const SAMPLE_RATE: u32 = 48_000;
const CHANNELS: u8 = 1;
const FRAME_MS: u64 = 10;
const SAMPLES: usize = frame_samples(FRAME_MS);

fn config() -> Config {
    Config {
        sample_rate: SAMPLE_RATE,
        channels: CHANNELS,
        frame_duration_ms: FRAME_MS as u8,
        // 10 ms PCM son 480 muestras = 960 B, dentro del tope del core.
        mtu: 1200,
        ..Config::default()
    }
}

/// Emisor: intenta abrir sesión contra el host y hablar `freq_hz`.
///
/// Devuelve `Ok(sent)` si logró handshake y envió, `Err` si no pudo
/// conectarse. El error NO es un fallo del spike: es el dato que se quiere
/// medir.
async fn speaker(freq_hz: f64, hold: Duration) -> anyhow::Result<u64> {
    let transport = UdpTransport::bind(UdpConfig {
        bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
        ..Default::default()
    })
    .await?;
    let transport: Arc<dyn Transport> = Arc::new(transport);
    let cs = Arc::new(CodecSession::new(transport, config(), CodecId::Pcm)?);
    let session = cs.session();
    // Rol servidor contra la dirección del host, que ya está escuchando.
    session
        .handshake(SessionRole::Client, host_addr())
        .await
        .map_err(|e| anyhow::anyhow!("handshake fallido: {e}"))?;

    let _ = session.ptt_press().await;
    let mut sine = sine_frames_i16(SAMPLES, CHANNELS, SAMPLE_RATE, freq_hz);
    let sent = Arc::new(AtomicU64::new(0));
    let deadline = std::time::Instant::now() + hold;
    while std::time::Instant::now() < deadline {
        if let Some(frame) = sine.next() {
            cs.send_samples(&frame).await?;
            sent.fetch_add(1, Ordering::Relaxed);
        }
        tokio::time::sleep(Duration::from_millis(FRAME_MS)).await;
    }
    let _ = session.ptt_release().await;
    let _ = cs.close().await;
    Ok(sent.load(Ordering::Relaxed))
}

/// Receptor: abre sesión, escucha `hold` y analiza el audio recibido.
async fn listener(hold: Duration) -> anyhow::Result<(u64, AudioStats)> {
    let transport = UdpTransport::bind(UdpConfig {
        bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
        ..Default::default()
    })
    .await?;
    let transport: Arc<dyn Transport> = Arc::new(transport);
    let cs = Arc::new(CodecSession::new(transport, config(), CodecId::Pcm)?);
    let session = cs.session();
    session.handshake_open().await?;

    let samples: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::new()));
    let frames = Arc::new(AtomicU64::new(0));
    let quit = Arc::new(AtomicBool::new(false));
    {
        let cs = Arc::clone(&cs);
        let samples = Arc::clone(&samples);
        let frames = Arc::clone(&frames);
        let quit = Arc::clone(&quit);
        tokio::spawn(async move {
            while !quit.load(Ordering::Acquire) {
                if let Ok(chunk) = cs.recv_samples().await {
                    frames.fetch_add(1, Ordering::Relaxed);
                    samples.lock().await.extend_from_slice(&chunk);
                } else {
                    break;
                }
            }
        });
    }

    tokio::time::sleep(hold).await;
    quit.store(true, Ordering::Release);
    let buf = samples.lock().await.clone();
    Ok((
        frames.load(Ordering::Relaxed),
        AudioStats::analyze(&buf, SAMPLE_RATE, CHANNELS as u16),
    ))
}

/// Puerto que escucha el host del spike.
const HOST_PORT: u16 = 34_900;

/// Dirección del host, construida en runtime (no es `const` estable).
fn host_addr() -> SocketAddr {
    SocketAddr::from(([127, 0, 0, 1], HOST_PORT))
}

/// Medición: un host y dos emisores con frecuencias distintas.
///
/// Si el host oye ambos tonos a la vez (frecuencia media medida), el audio
/// entrante no se restringe al peer del handshake. Si mide 440 Hz exacto, cada
/// sesión está encerrada en su propio par.
#[tokio::test(flavor = "multi_thread", worker_threads = 6)]
async fn spike_one_host_two_speakers() -> anyhow::Result<()> {
    // El host escucha en HOST_ADDR durante toda la medición.
    let host = tokio::spawn(async {
        let transport = UdpTransport::bind(UdpConfig {
            bind_addr: host_addr(),
            ..Default::default()
        })
        .await?;
        let transport: Arc<dyn Transport> = Arc::new(transport);
        let cs = Arc::new(CodecSession::new(transport, config(), CodecId::Pcm)?);
        let session = cs.session();
        session.handshake_open().await?;

        let samples: Arc<Mutex<Vec<i16>>> = Arc::new(Mutex::new(Vec::new()));
        let frames = Arc::new(AtomicU64::new(0));
        let quit = Arc::new(AtomicBool::new(false));
        {
            let cs = Arc::clone(&cs);
            let samples = Arc::clone(&samples);
            let frames = Arc::clone(&frames);
            let quit = Arc::clone(&quit);
            tokio::spawn(async move {
                while !quit.load(Ordering::Acquire) {
                    if let Ok(chunk) = cs.recv_samples().await {
                        frames.fetch_add(1, Ordering::Relaxed);
                        samples.lock().await.extend_from_slice(&chunk);
                    } else {
                        break;
                    }
                }
            });
        }

        tokio::time::sleep(Duration::from_millis(4000)).await;
        quit.store(true, Ordering::Release);
        let buf = samples.lock().await.clone();
        let (frames, stats) = (
            frames.load(Ordering::Relaxed),
            AudioStats::analyze(&buf, SAMPLE_RATE, CHANNELS as u16),
        );
        Ok::<(u64, AudioStats), anyhow::Error>((frames, stats))
    });

    tokio::time::sleep(Duration::from_millis(250)).await;
    let hold = Duration::from_millis(3000);
    let a = tokio::spawn(speaker(440.0, hold));
    let b = tokio::spawn(speaker(660.0, hold));

    let sent_a = a.await.expect("sonda A panic")?;
    let sent_b = b.await.expect("sonda B panic")?;
    let host_res = host.await.expect("host panic")?;
    let (frames, stats) = host_res;

    println!("--- SPIKE S0: un host, dos emisores ---");
    println!("emisor 440 Hz: {sent_a} frames enviados");
    println!("emisor 660 Hz: {sent_b} frames enviados");
    println!("host: {frames} frames recibidos");
    println!("host midio: {:.1} Hz (rms {:.4})", stats.freq_hz, stats.rms);

    if frames == 0 {
        println!("HALLAZGO: el host NO recibio audio de ningun emisor.");
    } else if (stats.freq_hz - 440.0).abs() < 20.0 {
        println!("HALLAZGO: el host midio ~440 Hz -> cada sesion esta encerrada en su par.");
    } else if (stats.freq_hz - 660.0).abs() < 20.0 {
        println!("HALLAZGO: el host midio ~660 Hz -> cada sesion esta encerrada en su par.");
    } else {
        println!(
            "HALLAZGO: el host midio una MEZCLA ({:.1} Hz) -> el audio entrante NO se \
             restringe al peer del handshake.",
            stats.freq_hz
        );
    }
    println!("--- fin S0 ---");
    Ok(())
}
