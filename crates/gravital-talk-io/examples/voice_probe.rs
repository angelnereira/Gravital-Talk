//! Sonda de audio: abre micrófono y altavoz en el formato de la sesión y dice
//! qué pasó.
//!
//! Sirve para separar "el device no abre" de "el protocolo no transmite":
//! si esto falla, el problema es de la máquina, no de Gravital Talk.
//!
//! ```bash
//! cargo run -p gravital-talk-io --example voice_probe
//! ```

use std::time::{Duration, Instant};

use gravital_talk_io::{list_input_devices, list_output_devices, VoiceCapture, VoicePlayback};

const RATE: u32 = 48_000;
const FRAME: usize = 480;

fn main() {
    for (kind, list) in [
        ("entrada", list_input_devices()),
        ("salida", list_output_devices()),
    ] {
        match list {
            Ok(devs) => {
                for d in devs {
                    println!(
                        "{kind}: {}{}",
                        d.name,
                        if d.is_default { "  (por defecto)" } else { "" }
                    );
                }
            }
            Err(e) => println!("{kind}: no se pudieron listar: {e}"),
        }
    }

    let playback = match VoicePlayback::start(RATE, FRAME) {
        Ok(p) => {
            println!("altavoz: abierto");
            Some(p)
        }
        Err(e) => {
            println!("altavoz: FALLO: {e}");
            None
        }
    };

    match VoiceCapture::start(RATE, FRAME) {
        Ok((_capture, frames)) => {
            println!("micrófono: abierto; midiendo 2 s (y devolviéndolo al altavoz)…");
            let until = Instant::now() + Duration::from_secs(2);
            let (mut n, mut peak) = (0u32, 0i16);
            while Instant::now() < until {
                if let Ok(f) = frames.recv_timeout(Duration::from_millis(200)) {
                    n += 1;
                    peak = peak.max(f.iter().map(|s| s.saturating_abs()).max().unwrap_or(0));
                    if let Some(p) = &playback {
                        p.push(&f);
                    }
                }
            }
            println!("micrófono: {n} frames de {FRAME} muestras, pico {peak}");
        }
        Err(e) => println!("micrófono: FALLO: {e}"),
    }
}
