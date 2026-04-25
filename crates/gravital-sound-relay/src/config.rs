//! Configuración cargada desde TOML o flags CLI.

use std::net::SocketAddr;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct RelayConfig {
    /// Bind para tráfico UDP (datagramas Gravital). Default: 0.0.0.0:9000.
    #[serde(default = "default_udp")]
    pub udp_bind: SocketAddr,
    /// Bind para WebSocket. Default: 0.0.0.0:9090.
    #[serde(default = "default_ws")]
    pub ws_bind: SocketAddr,
    /// Bind para el endpoint HTTP de observabilidad (/metrics, /healthz).
    #[serde(default = "default_obs")]
    pub observability_bind: SocketAddr,
    /// TTL para entradas inactivas del routing table (segundos).
    #[serde(default = "default_ttl")]
    pub session_ttl_secs: u64,
    /// Máximo número de session_id activos simultáneos. Protege memoria.
    #[serde(default = "default_max_sessions")]
    pub max_sessions: usize,
}

fn default_udp() -> SocketAddr {
    "0.0.0.0:9000".parse().unwrap()
}
fn default_ws() -> SocketAddr {
    "0.0.0.0:9090".parse().unwrap()
}
fn default_obs() -> SocketAddr {
    "0.0.0.0:9100".parse().unwrap()
}
fn default_ttl() -> u64 {
    300
}
fn default_max_sessions() -> usize {
    10_000
}

impl Default for RelayConfig {
    fn default() -> Self {
        Self {
            udp_bind: default_udp(),
            ws_bind: default_ws(),
            observability_bind: default_obs(),
            session_ttl_secs: default_ttl(),
            max_sessions: default_max_sessions(),
        }
    }
}

impl RelayConfig {
    pub fn from_file(path: &Path) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        let cfg: Self = toml::from_str(&contents)?;
        Ok(cfg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_sensible() {
        let cfg = RelayConfig::default();
        assert_eq!(cfg.udp_bind.port(), 9000);
        assert_eq!(cfg.ws_bind.port(), 9090);
        assert_eq!(cfg.observability_bind.port(), 9100);
        assert_eq!(cfg.session_ttl_secs, 300);
        assert_eq!(cfg.max_sessions, 10_000);
    }

    #[test]
    fn parses_toml() {
        let toml_str = r#"
            udp_bind = "127.0.0.1:7000"
            ws_bind = "127.0.0.1:7001"
            observability_bind = "127.0.0.1:7002"
            session_ttl_secs = 60
            max_sessions = 100
        "#;
        let cfg: RelayConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.udp_bind.port(), 7000);
        assert_eq!(cfg.session_ttl_secs, 60);
        assert_eq!(cfg.max_sessions, 100);
    }

    #[test]
    fn partial_toml_uses_defaults() {
        let toml_str = r#"udp_bind = "127.0.0.1:5000""#;
        let cfg: RelayConfig = toml::from_str(toml_str).unwrap();
        assert_eq!(cfg.udp_bind.port(), 5000);
        assert_eq!(cfg.ws_bind.port(), 9090); // default
    }
}
