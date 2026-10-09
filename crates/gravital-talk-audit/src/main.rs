//! `gs-audit` — ejecutor del harness de auditoría.
//!
//! Salida: un resumen legible por consola, un JSON opcional para CI y un
//! código de salida distinto de cero si algún criterio se incumple.
//!
//! Ejemplos:
//!
//! ```text
//! # Par P2P 1:1 con turnos partidos y verificación de audio en ambos lados
//! gs-audit --mode p2p --duration-ms 4000 --split-turns --verify-audio
//!
//! # N sesiones P2P simultáneas, cada una con su frecuencia (detecta cruce)
//! gs-audit --mode p2p --sessions 3 --duration-ms 4000 --verify-audio
//!
//! # Sala vía relay: el host y el join se conectan al mismo código
//! gs-audit --mode room --relay relay --room GRVT-A3F2 --duration-ms 4000
//!
//! # Un solo lado (para escenarios multi-contenedor)
//! gs-audit --role host --listen --port 9000 --mode room --relay relay --room CODE
//! ```

use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Parser;
use gravital_talk::CodecId;
use gravital_talk_audit::harness::{
    run_multi_pair, run_pair, PairOptions, PeerOptions, PeerRole, TransportMode, TurnPlan,
};
use gravital_talk_audit::{report, PeerOutcome};

/// Harness de auditoría end-to-end de Gravital Talk.
#[derive(Debug, Parser)]
#[command(name = "gs-audit", version, about, long_about = None)]
struct Cli {
    /// Rol de este proceso.
    #[arg(long, value_enum, default_value = "pair")]
    role: RoleArg,

    /// Cómo se alcanza el peer.
    #[arg(long, value_enum, default_value = "p2p")]
    mode: ModeArg,

    /// Host del relay (obligatorio en modo room).
    #[arg(long)]
    relay: Option<String>,

    /// Puerto UDP del relay.
    #[arg(long, default_value_t = 9000)]
    relay_udp_port: u16,

    /// Puerto HTTP de observabilidad del relay (para resolver salas).
    #[arg(long, default_value_t = 9100)]
    relay_obs_port: u16,

    /// Código de sala (obligatorio en modo room).
    #[arg(long)]
    room: Option<String>,

    /// Token de sala (PSK de Noise).
    #[arg(long)]
    room_token: Option<String>,

    /// Dirección del peer en modo P2P de un solo lado.
    #[arg(long)]
    peer: Option<String>,

    /// Puerto donde escucha el host.
    #[arg(long, default_value_t = 34_100)]
    port: u16,

    /// Actúa como host (espera al peer). Implica `handshake_open`.
    #[arg(long)]
    listen: bool,

    /// Códec de audio.
    #[arg(long, value_enum, default_value = "pcm")]
    codec: CodecArg,

    /// Pares host/join simultáneos en este proceso.
    #[arg(long, default_value_t = 1)]
    sessions: usize,

    /// Duración de la ejecución, en milisegundos.
    #[arg(long, default_value_t = 4_000)]
    duration_ms: u64,

    /// Frecuencia de la senoidal de prueba, en Hz.
    #[arg(long, default_value_t = gravital_talk_audit::DEFAULT_FREQ_HZ)]
    freq_hz: f64,

    /// MTU por paquete en bytes.
    #[arg(long, default_value_t = 1200)]
    mtu: usize,

    /// Duración del frame de audio en milisegundos. OJO: con PCM el tope duro
    /// del core es 1176 B de payload, asi que 20 ms no cabe (usa 10).
    #[arg(long, default_value_t = gravital_talk_audit::harness::FRAME_DURATION_MS)]
    frame_ms: u64,

    /// Plan de turnos del host: `all`, `never` o `INICIO-FIN` en ms.
    #[arg(long, default_value = "all")]
    host_turn: String,

    /// Plan de turnos del join: `all`, `never` o `INICIO-FIN` en ms.
    #[arg(long, default_value = "all")]
    join_turn: String,

    /// Divide la duración en dos turnos disjuntos (host luego join).
    #[arg(long, conflicts_with_all = ["host_turn", "join_turn"])]
    split_turns: bool,

    /// Exige como mínimo N frames recibidos.
    #[arg(long)]
    expect_rx_frames: Option<u64>,

    /// Exige como mínimo N frames enviados.
    #[arg(long)]
    expect_tx_frames: Option<u64>,

    /// Exige como mínimo N activaciones observadas del PTT remoto.
    #[arg(long)]
    expect_peer_ptt: Option<u64>,

    /// Verifica que el audio recibido sea el tono esperado (frecuencia, RMS).
    #[arg(long)]
    verify_audio: bool,

    /// Tolerancia de frecuencia en Hz.
    #[arg(long, default_value_t = 8.0)]
    freq_tolerance_hz: f64,

    /// RMS mínimo para considerar que el audio no es silencio.
    #[arg(long, default_value_t = 0.01)]
    min_rms: f64,

    /// Escribe el PCM recibido en un WAV (o en un directorio si es par).
    #[arg(long)]
    dump_rx_wav: Option<PathBuf>,

    /// Nombre del escenario en el informe.
    #[arg(long, default_value = "audit")]
    scenario: String,

    /// Escribe el informe JSON en esta ruta.
    #[arg(long)]
    json: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum RoleArg {
    /// Ejecuta host y join juntos en este proceso.
    Pair,
    Host,
    Join,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum ModeArg {
    P2p,
    Room,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
enum CodecArg {
    Pcm,
    Opus,
}

impl From<CodecArg> for CodecId {
    fn from(c: CodecArg) -> Self {
        match c {
            CodecArg::Pcm => Self::Pcm,
            CodecArg::Opus => Self::Opus,
        }
    }
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match run(cli).await {
        Ok(true) => std::process::exit(0),
        Ok(false) => std::process::exit(1),
        Err(e) => {
            eprintln!("gs-audit: {e:#}");
            std::process::exit(2);
        }
    }
}

/// Ejecuta la auditoría. `Ok(false)` significa "corrió pero falló un criterio".
async fn run(cli: Cli) -> Result<bool> {
    let mode = match cli.mode {
        ModeArg::P2p => TransportMode::P2p,
        ModeArg::Room => TransportMode::Room,
    };

    let outcomes = match cli.role {
        RoleArg::Pair => run_pairs(&cli, mode).await?,
        RoleArg::Host | RoleArg::Join => run_single(&cli, mode).await?,
    };

    let passed = outcomes.iter().all(|o| o.passed());
    print!("{}", gravital_talk_audit::render_summary(&outcomes));

    let json = report::to_json(&cli.scenario, &outcomes);
    if let Some(path) = cli.json.as_ref() {
        report::write(path, &json)?;
        println!("informe JSON: {}", path.display());
    }
    if !passed {
        println!("\nAUDITORIA FALLIDA — ver criterios incumplidos arriba");
        for o in &outcomes {
            if !o.passed() {
                println!("  - {}: {}", o.label, o.failures.join("; "));
            }
        }
    }
    Ok(passed)
}

/// Escenarios de par (uno o varios) ejecutados en este proceso.
async fn run_pairs(cli: &Cli, mode: TransportMode) -> Result<Vec<PeerOutcome>> {
    let mut opts = PairOptions {
        scenario: cli.scenario.clone(),
        mode,
        host_bind: SocketAddr::from(([127, 0, 0, 1], cli.port)),
        freq_hz: cli.freq_hz,
        mtu: cli.mtu,
        frame_duration_ms: cli.frame_ms,
        relay: cli.relay.clone(),
        relay_udp_port: cli.relay_udp_port,
        relay_obs_port: cli.relay_obs_port,
        room: cli.room.clone(),
        room_token: cli.room_token.clone(),
        codec: cli.codec.into(),
        duration_ms: cli.duration_ms,
        freq_tolerance_hz: cli.freq_tolerance_hz,
        min_rms: cli.min_rms,
        verify_audio: cli.verify_audio,
        expect_rx_frames_each: cli.expect_rx_frames,
        expect_tx_frames_each: cli.expect_tx_frames,
        expect_peer_ptt_activations: cli.expect_peer_ptt,
        dump_dir: cli.dump_rx_wav.clone(),
        ..PairOptions::default()
    };

    if cli.split_turns {
        opts.with_split_turns();
    } else {
        opts.host_turn = TurnPlan::parse(&cli.host_turn)?;
        opts.join_turn = TurnPlan::parse(&cli.join_turn)?;
    }

    if mode == TransportMode::Room {
        if opts.relay.is_none() || opts.room.is_none() {
            bail!("--mode room requiere --relay y --room");
        }
        if opts.host_bind.port() == 0 {
            bail!("--port 0 no sirve en modo room: el relay enruta por session_id");
        }
    }

    if cli.sessions > 1 {
        run_multi_pair(&opts, cli.sessions).await
    } else {
        run_pair(&opts).await
    }
}

/// Escenario de un solo lado, para despliegues multi-contenedor.
async fn run_single(cli: &Cli, mode: TransportMode) -> Result<Vec<PeerOutcome>> {
    let role = if cli.role == RoleArg::Host {
        PeerRole::Host
    } else {
        PeerRole::Join
    };
    let listen = cli.listen || role == PeerRole::Host;

    // En modo room el destino es el relay y el session_id viene del código.
    let (peer_addr, preset_session_id) = match mode {
        TransportMode::Room => {
            let relay = cli
                .relay
                .as_ref()
                .context("--relay es obligatorio en modo room")?;
            let room = cli
                .room
                .as_ref()
                .context("--room es obligatorio en modo room")?;
            let sid = gravital_talk_audit::http::resolve_room(
                relay,
                cli.relay_obs_port,
                room,
                cli.room_token.as_deref(),
            )
            .await?;
            let addr =
                gravital_talk_audit::http::lookup_host_any(relay, cli.relay_udp_port).await?;
            (addr, Some(sid))
        }
        TransportMode::P2p => {
            let peer = cli
                .peer
                .as_ref()
                .context("--peer es obligatorio en modo p2p de un solo lado")?;
            let addr = gravital_talk_audit::http::lookup_host_any(peer, cli.port).await?;
            (addr, None)
        }
    };

    let opts = PeerOptions {
        label: format!("{}/{}", cli.scenario, role.as_str()),
        role,
        mode,
        bind_addr: SocketAddr::from(([0, 0, 0, 0], if listen { cli.port } else { 0 })),
        // En sala el tráfico llega reenviado por el relay: hay que esperarlo
        // a él, igual que hace `gs ptt --listen --relay`.
        expect_known_peer: listen && mode == TransportMode::Room,
        peer_addr,
        preset_session_id,
        room_token: cli.room_token.clone(),
        codec: cli.codec.into(),
        freq_hz: cli.freq_hz,
        duration_ms: cli.duration_ms,
        mtu: cli.mtu,
        frame_duration_ms: cli.frame_ms,
        turn: if role == PeerRole::Host {
            TurnPlan::parse(&cli.host_turn)?
        } else {
            TurnPlan::parse(&cli.join_turn)?
        },
        dump_rx_wav: cli.dump_rx_wav.clone(),
        verify_audio: cli.verify_audio,
        freq_tolerance_hz: cli.freq_tolerance_hz,
        min_rms: cli.min_rms,
        expect_rx_frames: cli.expect_rx_frames,
        expect_tx_frames: cli.expect_tx_frames,
        expect_peer_ptt_activations: cli.expect_peer_ptt,
    };

    Ok(vec![gravital_talk_audit::run_peer(&opts).await?])
}
