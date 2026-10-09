//! Informe JSON de la auditoría.
//!
//! Se genera a mano, sin dependencias de serialización, para que el harness no
//! añada librerías al árbol de dependencias del proyecto. El formato permite
//! que CI distinga "pasó" de "no se ejecutó": un escenario ausente en el JSON
//! es un escenario que no corrió, nunca un aprobado.

use std::fmt::Write as _;
use std::path::Path;

use crate::harness::PeerOutcome;

/// Escapa una cadena para JSON (ASCII, sin caracteres de control).
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn jstr(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

/// Serializa los resultados como objeto JSON de un escenario.
pub fn to_json(scenario: &str, outcomes: &[PeerOutcome]) -> String {
    let passed = outcomes.iter().filter(|o| o.passed()).count();
    let mut out = String::new();
    let _ = writeln!(out, "{{");
    let _ = writeln!(out, "  \"scenario\": {},", jstr(scenario));
    let _ = writeln!(out, "  \"peers_total\": {},", outcomes.len());
    let _ = writeln!(out, "  \"peers_passed\": {},", passed);
    let _ = writeln!(out, "  \"pass\": {},", passed == outcomes.len());
    out.push_str("  \"peers\": [\n");

    for (i, o) in outcomes.iter().enumerate() {
        let sep = if i + 1 == outcomes.len() { "" } else { "," };
        let _ = writeln!(out, "    {{");
        let _ = writeln!(out, "      \"label\": {},", jstr(&o.label));
        let _ = writeln!(out, "      \"role\": {},", jstr(o.role.as_str()));
        let _ = writeln!(out, "      \"mode\": {},", jstr(o.mode.as_str()));
        let _ = writeln!(out, "      \"codec\": {},", jstr(&o.codec));
        let _ = writeln!(out, "      \"freq_hz\": {:.2},", o.freq_hz);
        let _ = writeln!(out, "      \"local_addr\": {},", jstr(&o.local_addr));
        let _ = writeln!(out, "      \"peer_addr\": {},", jstr(&o.peer_addr));
        let _ = writeln!(out, "      \"tx_frames\": {},", o.stats.tx_frames);
        let _ = writeln!(out, "      \"rx_frames\": {},", o.stats.rx_frames);
        let _ = writeln!(
            out,
            "      \"peer_ptt_activations\": {},",
            o.stats.peer_ptt_activations
        );
        let _ = writeln!(out, "      \"send_errors\": {},", o.stats.send_errors);
        let _ = writeln!(out, "      \"recv_errors\": {},", o.stats.recv_errors);
        let _ = writeln!(out, "      \"audio\": {{");
        let _ = writeln!(out, "        \"samples\": {},", o.audio.samples);
        let _ = writeln!(out, "        \"duration_ms\": {:.2},", o.audio.duration_ms);
        let _ = writeln!(out, "        \"rms\": {:.6},", o.audio.rms);
        let _ = writeln!(out, "        \"peak\": {:.6},", o.audio.peak);
        let _ = writeln!(out, "        \"freq_hz\": {:.2},", o.audio.freq_hz);
        let _ = writeln!(
            out,
            "        \"zero_crossings\": {},",
            o.audio.zero_crossings
        );
        let _ = writeln!(out, "        \"silent\": {}", o.audio.silent);
        let _ = writeln!(out, "      }},");
        let _ = writeln!(out, "      \"metrics\": {{");
        let _ = writeln!(out, "        \"rtt_ms\": {:.2},", o.rtt_ms);
        let _ = writeln!(out, "        \"jitter_ms\": {:.2},", o.jitter_ms);
        let _ = writeln!(out, "        \"loss_percent\": {:.2},", o.loss_percent);
        let _ = writeln!(out, "        \"mos\": {:.2},", o.mos);
        let _ = writeln!(out, "        \"packets_received\": {},", o.packets_received);
        let _ = writeln!(out, "        \"bytes_received\": {}", o.bytes_received);
        let _ = writeln!(out, "      }},");
        let wav = o
            .rx_wav
            .as_ref()
            .map_or_else(|| "null".to_string(), |p| jstr(&p.display().to_string()));
        let _ = writeln!(out, "      \"rx_wav\": {wav},");
        let failures: Vec<String> = o.failures.iter().map(|f| jstr(f)).collect();
        let _ = writeln!(out, "      \"failures\": [{}]", failures.join(", "));
        let _ = writeln!(out, "    }}{sep}");
    }

    out.push_str("  ]\n}\n");
    out
}

/// Escribe el informe en disco, creando los directorios necesarios.
pub fn write(path: &Path, contents: &str) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    std::fs::write(path, contents)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analyze::AudioStats;
    use crate::harness::{PeerRole, RunStats, TransportMode};

    fn sample(label: &str, failures: Vec<&str>) -> PeerOutcome {
        PeerOutcome {
            label: label.to_string(),
            role: PeerRole::Host,
            mode: TransportMode::P2p,
            codec: "Pcm".to_string(),
            freq_hz: 440.0,
            local_addr: "127.0.0.1:34100".to_string(),
            peer_addr: "127.0.0.1:34101".to_string(),
            stats: RunStats {
                tx_frames: 50,
                rx_frames: 50,
                peer_ptt_activations: 1,
                send_errors: 0,
                recv_errors: 0,
            },
            audio: AudioStats::analyze(&vec![1000i16; 960], 48_000, 1),
            rx_wav: None,
            rtt_ms: 1.5,
            jitter_ms: 0.2,
            loss_percent: 0.0,
            mos: 4.4,
            packets_received: 50,
            bytes_received: 48_000,
            failures: failures.into_iter().map(String::from).collect(),
        }
    }

    #[test]
    fn emits_valid_json_shell() {
        let json = to_json(
            "unit",
            &[sample("a", vec![]), sample("b", vec!["rx frames 0 < 1"])],
        );
        assert!(json.starts_with('{'));
        assert!(json.trim_end().ends_with('}'));
        assert!(json.contains("\"peers_total\": 2"));
        assert!(json.contains("\"peers_passed\": 1"));
        assert!(json.contains("\"pass\": false"));
        assert!(json.contains("\"failures\": [\"rx frames 0 < 1\"]"));
        // Comas balanceadas fuera de cadenas: conteo bruto como smoke test.
        let opens = json.matches('{').count();
        let closes = json.matches('}').count();
        assert_eq!(opens, closes);
    }

    #[test]
    fn escaping_neutralizes_quotes_and_backslashes() {
        assert_eq!(json_escape(r#"a"b"#), r#"a\"b"#);
        assert_eq!(json_escape(r"a\b"), r"a\\b");
        assert_eq!(json_escape("line\nbreak"), r"line\nbreak");
        let ctrl = char::from_u32(1).expect("valid code point");
        assert_eq!(json_escape(&ctrl.to_string()), "\\u0001");
        // Si no se escapan las comillas, el JSON resultante queda roto.
        let json = to_json("x", &[sample("has \" quote", vec![])]);
        assert!(json.contains(r#""has \" quote""#), "json was:\n{json}");
    }

    #[test]
    fn all_passed_marks_true() {
        let json = to_json("ok", &[sample("a", vec![])]);
        assert!(json.contains("\"pass\": true"));
    }
}
