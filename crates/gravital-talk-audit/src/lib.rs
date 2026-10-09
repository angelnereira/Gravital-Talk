//! Harness de auditoría end-to-end de Gravital Talk.
//!
//! Existe para responder a una pregunta que la suite actual no responde:
//! ¿el audio **realmente** cruza el protocolo en ambos sentidos, o sólo
//! llegan datagramas?
//!
//! Los tests de integración existentes (`crates/gravital-talk/tests/`) usan
//! frames PCM crudos y verifican conteos y estados. `gs ptt --headless` va un
//! paso más allá y reporta `recibidos=N`, pero:
//!
//! - `docker/scripts/room-host.sh` no parsea su salida, así que la dirección
//!   *join -> host* nunca se verifica;
//! - un conteo de frames no distingue audio válido de bytes corruptos;
//! - en modo headless el PTT se mantiene activo toda la ejecución, así que
//!   ambos lados hablan a la vez y el floor control nunca se ejercita.
//!
//! Este crate resuelve las tres carencias: planes de turno disjuntos, análisis
//! del PCM recibido y criterios evaluados dentro del proceso que producen un
//! código de salida útil para CI.

pub mod analyze;
pub mod harness;
pub mod http;
pub mod report;
pub mod sine;

pub use analyze::AudioStats;
pub use harness::{
    render_summary, run_multi_pair, run_pair, run_peer, PairOptions, PeerOptions, PeerOutcome,
    PeerRole, TransportMode, TurnPlan, DEFAULT_FREQ_HZ,
};
