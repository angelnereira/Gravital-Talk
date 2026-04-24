# Gravital Sound

**Protocolo de comunicación de audio en tiempo real, de grado militar, escrito en Rust.**

Gravital Sound es una biblioteca de infraestructura diseñada para transportar audio de baja latencia por internet con integridad criptográfica, control de jitter y observabilidad completa. El núcleo del protocolo está implementado en Rust puro (`no_std` compatible) y se expone a cualquier lenguaje a través de una capa FFI estable en C.

Este repositorio es la implementación de referencia. Contiene el protocolo, el transporte, las métricas, el CLI, y los SDKs de Python y Web/WASM.

---

## Filosofía

1. **Portabilidad universal.** Un único core Rust, una capa FFI C estable, y SDKs idiomáticos por plataforma. El mismo protocolo corre en servidores Linux, escritorio, móvil y navegador.
2. **Rendimiento sin compromisos.** Zero-copy en el decode, zero-allocation en el hot path, SIMD para CRC-16, `mimalloc`, LTO fat, `codegen-units = 1`, `panic = "abort"`. Targets de latencia medidos con histogramas HDR.
3. **Correctitud por construcción.** Máquina de estados de sesión con transiciones type-safe (una transición inválida es un error de compilación), property testing con `proptest`, verificación formal opcional con `kani`.
4. **Observabilidad desde el día 0.** RTT, jitter, pérdida, reordenamiento y MOS estimado expuestos como contadores atómicos lock-free.

---

## Estado del proyecto

| Componente                       | Estado       |
|----------------------------------|--------------|
| Especificación del protocolo     | `0.1-draft`  |
| Core (`gravital-sound-core`)     | `alpha`      |
| Transporte UDP                   | `alpha`      |
| Transporte WebSocket             | `alpha`      |
| Métricas                         | `alpha`      |
| Capa FFI (C ABI)                 | `alpha`      |
| CLI (`gs`)                       | `alpha`      |
| SDK Python                       | `alpha`      |
| SDK Web / WASM                   | `alpha`      |
| SDK Swift / Kotlin / Node        | *roadmap*    |
| Codec Opus + audio I/O (`cpal`)  | *roadmap*    |
| Relay productivo + Docker        | *roadmap*    |

La versión actual es `0.1.0-alpha.1`. El protocolo aún no es estable; pueden introducirse cambios incompatibles hasta la `0.1.0` final.

---

## Quickstart — Rust

```rust
use gravital_sound::{Config, Session};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let config = Config::default()
        .sample_rate(48_000)
        .channels(2)
        .frame_duration_ms(20)
        .jitter_buffer_ms(40);

    let mut session = Session::connect(&config, "127.0.0.1:9000").await?;

    let sine = gravital_sound::test_signals::sine(48_000, 2, 440.0, /* ms = */ 1000);
    for frame in sine.chunks(960 * 2) {
        session.send_audio(frame).await?;
    }

    let metrics = session.metrics();
    println!("RTT {:.2} ms · jitter {:.2} ms · loss {:.2}%",
        metrics.rtt_ms, metrics.jitter_ms, metrics.loss_percent);
    Ok(())
}
```

## Quickstart — Python

```python
import gravital_sound as gs

session = gs.Session(sample_rate=48_000, channels=2)
session.connect("127.0.0.1", 9000)

while True:
    pcm = session.recv_audio(frame_size=960)
    process(pcm)
```

## Quickstart — Browser (WASM)

```typescript
import { GravitalSound } from "@gravital/sound-web";

const session = await GravitalSound.connect({
  url: "wss://relay.gravitalsound.dev/session/abc",
  sampleRate: 48_000,
  channels: 2,
});

session.onAudio((pcm) => {
  audioContext.enqueue(pcm);
});
```

## Quickstart — CLI

```bash
# Lista input/output devices disponibles.
gs devices

# Terminal 1: receptor que escribe WAV + reproduce por altavoz.
gs receive --bind 0.0.0.0 --port 9000 \
           --peer 127.0.0.1 --peer-port 9100 \
           --device default --codec opus \
           --output out.wav

# Terminal 2: emisor desde micrófono con codec Opus.
gs send --host 127.0.0.1 --port 9000 \
        --device default --codec opus

# Emisor con onda sinusoidal sintética (sin hardware).
gs send --host 127.0.0.1 --port 9000 --input sine --codec pcm

# Benchmark de latencia loopback con histogramas p50/p99/p99.9.
gs bench --mode loopback --duration 30
```

---

## Arquitectura

```
┌──────────────────────────────────────────────────────────┐
│  SDKs idiomáticos (Python · Web/WASM · CLI · roadmap…)   │
└───────────────────────────┬──────────────────────────────┘
                            │
┌───────────────────────────┴──────────────────────────────┐
│  gravital-sound-ffi (C ABI estable, generado por cbindgen│
└───────────────────────────┬──────────────────────────────┘
                            │
┌───────────────────────────┴──────────────────────────────┐
│  gravital-sound (facade) · cli · transport · metrics     │
└───────────────────────────┬──────────────────────────────┘
                            │
┌───────────────────────────┴──────────────────────────────┐
│  gravital-sound-core  —  no_std · zero-copy · type-safe  │
└──────────────────────────────────────────────────────────┘
```

La especificación formal vive en [`docs/protocol-spec.md`](docs/protocol-spec.md). El formato binario del paquete, con diagramas de bits y ejemplos hex, está en [`docs/packet-format.md`](docs/packet-format.md).

---

## Compilación

Requisitos:
- Rust estable 1.78 o superior (instalable vía [rustup](https://rustup.rs/)).
- `make`, `gcc` (para el smoke test de FFI).
- Para cross-compilation: [`cross-rs`](https://github.com/cross-rs/cross).
- Para el SDK Python: [`maturin`](https://www.maturin.rs/) y Python ≥ 3.9.
- Para el SDK Web: [`wasm-pack`](https://rustwasm.github.io/wasm-pack/).

```bash
make build           # release de todo el workspace
make check-all       # fmt + clippy + test (gate mínimo)
make bench           # criterion benchmarks
make cross-wasm      # valida que el core sigue siendo no_std compatible
make ffi-smoke       # genera header C y corre el smoke test
make python-test     # compila SDK Python y corre pytest
make web-sdk         # compila SDK Web/WASM
```

---

## Documentación

- [`docs/overview.md`](docs/overview.md) — visión y posicionamiento.
- [`docs/protocol-spec.md`](docs/protocol-spec.md) — especificación completa.
- [`docs/packet-format.md`](docs/packet-format.md) — formato binario.
- [`docs/session-model.md`](docs/session-model.md) — máquina de estados.
- [`docs/transport.md`](docs/transport.md) — diseño del transporte.
- [`docs/security.md`](docs/security.md) — modelo de amenazas.
- [`docs/portability.md`](docs/portability.md) — guía de portabilidad.
- [`docs/adr/`](docs/adr) — decisiones arquitectónicas.

---

## Licencia

Dual-licensed bajo MIT ([`LICENSE-MIT`](LICENSE-MIT)) o Apache-2.0 ([`LICENSE-APACHE`](LICENSE-APACHE)), a elección del consumidor. Este es el esquema estándar del ecosistema Rust y es compatible con uso comercial y embebido.

```
SPDX-License-Identifier: MIT OR Apache-2.0
```

---

## Organización

Gravital Sound es una división de **Nereira Technology and Business Solutions**, bajo el paraguas de la marca **Gravital** (junto a Gravital Cloud, Gravital Security/Quimera, Gravital ID). Opera como biblioteca standalone para cualquier desarrollador; la integración con el resto del ecosistema Gravital es opcional.

— Angel Nereira
