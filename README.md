# Gravital Sound

[![CI](https://github.com/angelnereira/gravital-sound/actions/workflows/ci.yml/badge.svg)](https://github.com/angelnereira/gravital-sound/actions/workflows/ci.yml)
[![Docs](https://github.com/angelnereira/gravital-sound/actions/workflows/docs.yml/badge.svg)](https://github.com/angelnereira/gravital-sound/actions/workflows/docs.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#licencia)

**Protocolo de comunicación de audio en tiempo real, de grado militar, escrito en Rust.**

Gravital Sound es una biblioteca de infraestructura diseñada para transportar audio de baja latencia por internet con integridad criptográfica, control de jitter y observabilidad completa. El núcleo del protocolo está implementado en Rust puro (`no_std` compatible) y se expone a cualquier lenguaje a través de una capa FFI estable en C.

Este repositorio es la implementación de referencia. Contiene el protocolo, el transporte UDP/WebSocket, codec Opus + audio I/O hardware, métricas Prometheus, CLI productivo, relay server stand-alone, módulos Terraform multi-cloud, Helm chart para Kubernetes, y SDKs de Python y Web/WASM.

---

## Filosofía

1. **Portabilidad universal.** Un único core Rust, una capa FFI C estable, SDKs idiomáticos por plataforma y módulos Terraform/Helm para desplegar en cualquier server o dispositivo. El mismo protocolo corre en servidores Linux, escritorio, móvil, navegador, edge (Raspberry Pi).
2. **Rendimiento sin compromisos.** Zero-copy en el decode, zero-allocation en el hot path, SIMD para CRC-16, `mimalloc`, LTO fat, `codegen-units = 1`, `panic = "abort"`. Targets de latencia medidos con histogramas HDR.
3. **Correctitud por construcción.** Máquina de estados de sesión con transiciones type-safe (una transición inválida es un error de compilación), property testing con `proptest`, verificación formal opcional con `kani`.
4. **Observabilidad desde el día 0.** RTT, jitter, pérdida, reordenamiento y MOS estimado expuestos como contadores atómicos lock-free. Relay productivo con endpoints `/metrics` (Prometheus) y `/healthz`. Dashboards Grafana prediseñados.

---

## Estado del proyecto

| Componente                                | Estado          |
|-------------------------------------------|-----------------|
| Especificación del protocolo              | `0.1-draft`     |
| Core (`gravital-sound-core`)              | `alpha`         |
| Transporte UDP                            | `alpha`         |
| Transporte WebSocket                      | `alpha`         |
| Métricas (RTT, jitter, loss, MOS)         | `alpha`         |
| Capa FFI (C ABI)                          | `alpha`         |
| CLI (`gs`)                                | `alpha`         |
| Codec Opus + PCM (`gravital-sound-codec`) | `alpha`         |
| Audio I/O hardware (`gravital-sound-io`)  | `alpha`         |
| Negociación de codec en handshake         | `alpha`         |
| Resampler (`rubato`)                      | `alpha`         |
| Relay productivo (`gravital-sound-relay`) | `alpha`         |
| Dockerfile + docker-compose               | `alpha`         |
| Terraform: AWS / Hetzner / DigitalOcean   | `alpha`         |
| Cloud-init Raspberry Pi                   | `alpha`         |
| Helm chart Kubernetes                     | `alpha`         |
| Dashboard Grafana                         | `alpha`         |
| SDK Python                                | `alpha`         |
| SDK Web / WASM                            | `alpha`         |
| SDK Swift / Kotlin / Node                 | *roadmap*       |
| Cifrado Noise + rate limiting             | *roadmap*       |
| Publicación crates.io / PyPI / npm        | *roadmap*       |

La versión actual es `0.2.0-alpha.1`. El protocolo aún no es estable; pueden introducirse cambios incompatibles hasta la `0.1.0` final.

---

## Quickstart — Rust

```rust
use std::sync::Arc;
use gravital_sound::{CodecId, CodecSession, Config, SessionRole, UdpConfig, UdpTransport};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let transport = Arc::new(UdpTransport::bind(UdpConfig::default()).await?);
    let session = CodecSession::new(transport, Config::default(), CodecId::Opus)?;
    session.handshake(SessionRole::Client, "127.0.0.1:9000".parse()?).await?;

    let frame_samples = vec![0i16; 480]; // 10 ms @ 48 kHz mono
    session.send_samples(&frame_samples).await?;

    let received = session.recv_samples().await?;
    let snap = session.session().metrics().snapshot(0.0);
    println!("MOS {:.2} · jitter {:.2} ms · loss {:.2}%",
        snap.estimated_mos, snap.jitter_ms, snap.loss_percent);
    let _ = received;
    Ok(())
}
```

## Quickstart — Python

```python
import gravital_sound as gs

session = gs.Session(sample_rate=48_000, channels=1)
session.connect("127.0.0.1", 9000)

while True:
    pcm = session.recv_audio()
    process(pcm)
```

## Quickstart — Browser (WASM)

```typescript
import { GravitalSound } from "@gravital/sound-web";

const session = await GravitalSound.connect({
  url: "wss://relay.gravitalsound.dev/session/abc",
  sampleRate: 48_000,
  channels: 1,
});

session.onAudio((pcm) => audioContext.enqueue(pcm));
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

## Quickstart — Relay productivo

```bash
# Local con Docker Compose (relay + Prometheus).
cd crates/gravital-sound-relay
docker compose up --build

# Endpoints expuestos:
#   UDP    9000 → tráfico de audio
#   WS     9090 → bridge para clientes browser
#   HTTP   9100 → /metrics (Prometheus) y /healthz
#   HTTP   9091 → Prometheus UI
```

```bash
# Standalone:
gs-relay --udp-bind 0.0.0.0:9000 --ws-bind 0.0.0.0:9090
curl http://localhost:9100/healthz   # → OK
curl http://localhost:9100/metrics   # → métricas Prometheus
```

## Quickstart — Terraform

Despliega un relay en AWS con un solo comando:

```bash
cd infra/terraform/examples/single-region-aws
terraform init && terraform apply
# → output: relay_endpoint, ws_url, udp_port
```

O ultra-barato (~€4/mes) en Hetzner:

```bash
cd infra/terraform/examples/self-hosted-hetzner
export TF_VAR_hcloud_token="..."
terraform init && terraform apply
```

## Quickstart — Raspberry Pi como edge node

```bash
# Flash Raspberry Pi OS Lite ARM64 en SD card, luego:
sudo cp infra/cloud-init/raspberry-pi.yml /boot/firmware/user-data
# Editar /boot/firmware/user-data y poner GS_RELAY_HOST.
# Boot. El Pi captura del mic local y envía al relay.
```

---

## Arquitectura

```
┌──────────────────────────────────────────────────────────┐
│  Apps & SDKs (Python · Web/WASM · CLI · Swift roadmap)   │
└───────────────────────────┬──────────────────────────────┘
                            │
┌───────────────────────────┴──────────────────────────────┐
│  gravital-sound-ffi  (C ABI estable, generado cbindgen)  │
└───────────────────────────┬──────────────────────────────┘
                            │
┌───────────────────────────┴──────────────────────────────┐
│  gravital-sound (facade)  ·  CodecSession                │
│  gravital-sound-codec  ·  gravital-sound-io  ·  cli      │
└───────────────────────────┬──────────────────────────────┘
                            │
┌───────────────────────────┴──────────────────────────────┐
│  gravital-sound-transport  ·  gravital-sound-metrics     │
│  Session · UdpTransport · WebSocketTransport · jitter    │
└───────────────────────────┬──────────────────────────────┘
                            │
┌───────────────────────────┴──────────────────────────────┐
│  gravital-sound-core  —  no_std · zero-copy · type-safe  │
└──────────────────────────────────────────────────────────┘

Servicios y deploy:
┌──────────────────────────────────────────────────────────┐
│  gravital-sound-relay  (UDP+WS routing, Prometheus)      │
│        ↓                                                 │
│  Docker · Helm chart · Terraform (AWS/Hetzner/DO)        │
│  Cloud-init (Pi · VPS) · Grafana dashboards              │
└──────────────────────────────────────────────────────────┘
```

La especificación formal vive en [`docs/protocol-spec.md`](docs/protocol-spec.md). El formato binario del paquete, con diagramas de bits y ejemplos hex, está en [`docs/packet-format.md`](docs/packet-format.md).

---

## Compilación

Requisitos:
- Rust estable 1.78 o superior (instalable vía [rustup](https://rustup.rs/)).
- `make`, `gcc` (para el smoke test de FFI).
- Sistema: `libopus-dev` y `libasound2-dev` (Linux) o `brew install opus` (macOS) — necesarios para los crates `gravital-sound-codec` (feature `opus`) y `gravital-sound-io`.
- Para cross-compilation: [`cross-rs`](https://github.com/cross-rs/cross).
- Para el SDK Python: [`maturin`](https://www.maturin.rs/) y Python ≥ 3.9.
- Para el SDK Web: [`wasm-pack`](https://rustwasm.github.io/wasm-pack/).
- Para Terraform: Terraform ≥ 1.5.
- Para Kubernetes: Helm ≥ 3.10.

```bash
make build           # release de todo el workspace
make check-all       # fmt + clippy + test (gate mínimo)
make bench           # criterion benchmarks
make cross-wasm      # valida que el core sigue siendo no_std compatible
make ffi-smoke       # genera header C y corre el smoke test
make python-test     # compila SDK Python y corre pytest
make web-sdk         # compila SDK Web/WASM
```

Sin libopus / libasound (entornos minimalistas), compilar con `--no-default-features`:

```bash
cargo build --workspace --no-default-features
```

---

## Estructura del workspace

```
crates/
├── gravital-sound-core         # no_std: header, packet, fragment, sesión, CRC
├── gravital-sound-metrics      # RTT/jitter/loss/MOS, contadores lock-free
├── gravital-sound-transport    # UDP + WebSocket + jitter buffer + Session
├── gravital-sound-codec        # Encoder/Decoder, PCM, Opus (libopus)
├── gravital-sound-io           # AudioCapture/Playback (cpal), Resampler (rubato)
├── gravital-sound              # Facade + CodecSession + ejemplos + benches
├── gravital-sound-ffi          # C ABI estable, header generado por cbindgen
├── gravital-sound-cli          # Binario `gs` (send/receive/devices/bench/relay)
└── gravital-sound-relay        # Binario `gs-relay` productivo

sdks/
├── python                      # PyO3 + maturin
└── web                         # wasm-bindgen + wasm-pack

infra/
├── terraform/
│   ├── modules/relay-aws       # EC2 + Security Group + Route53
│   ├── modules/relay-hetzner   # CX22, ~€4/mes (opción barata)
│   ├── modules/relay-digitalocean
│   └── modules/edge-node       # user_data agnóstico para edge clients
├── helm/gravital-sound-relay   # Chart Helm v3 con HPA, ServiceMonitor
├── grafana/dashboards          # Fleet overview JSON
└── cloud-init/raspberry-pi.yml # Bootstrap directo de SD card

docs/
├── protocol-spec.md            # especificación formal del protocolo
├── packet-format.md            # formato binario con diagramas de bits
├── session-model.md            # máquina de estados
├── transport.md                # diseño del transporte
├── codecs.md                   # arquitectura codec, rangos Opus
├── audio-io.md                 # captura/playback con cpal
├── security.md                 # modelo de amenazas
├── portability.md              # guía de portabilidad
├── benchmarks.md               # baseline criterion
└── adr/                        # decisiones arquitectónicas (001-007)
```

---

## CI/CD

| Workflow                          | Trigger                                   | Función                                                  |
|-----------------------------------|-------------------------------------------|----------------------------------------------------------|
| `.github/workflows/ci.yml`        | push (main/feat/verify/claude) + PR       | fmt, clippy, test (Linux+macOS), no-default-features, FFI smoke, cross-wasm, cross-aarch64, python-sdk |
| `.github/workflows/release.yml`   | tag `v*`                                  | Matriz binaria 5 plataformas + wheels Python + bundle WASM, draft release a publicación |
| `.github/workflows/docs.yml`      | push main + tag `v*`                      | `cargo doc --workspace --all-features` → GitHub Pages    |
| `.github/workflows/terraform.yml` | cambios en `infra/terraform/**`           | fmt, validate por módulo, tflint, checkov security scan  |

---

## Documentación

- [`docs/overview.md`](docs/overview.md) — visión y posicionamiento.
- [`docs/protocol-spec.md`](docs/protocol-spec.md) — especificación completa.
- [`docs/packet-format.md`](docs/packet-format.md) — formato binario.
- [`docs/session-model.md`](docs/session-model.md) — máquina de estados.
- [`docs/transport.md`](docs/transport.md) — diseño del transporte.
- [`docs/codecs.md`](docs/codecs.md) — codec Opus, rangos de bitrate, negociación.
- [`docs/audio-io.md`](docs/audio-io.md) — captura/playback con cpal, backpressure.
- [`docs/security.md`](docs/security.md) — modelo de amenazas.
- [`docs/portability.md`](docs/portability.md) — guía de portabilidad.
- [`docs/benchmarks.md`](docs/benchmarks.md) — baselines criterion.
- [`docs/adr/`](docs/adr) — decisiones arquitectónicas (001-007).
- [`infra/README.md`](infra/README.md) — runbook de Terraform/Helm/cloud-init.
- [`infra/terraform/modules/relay-aws/README.md`](infra/terraform/modules/relay-aws/README.md) — guía del módulo AWS.
- [`infra/helm/gravital-sound-relay/README.md`](infra/helm/gravital-sound-relay/README.md) — guía del chart Helm.
- [`infra/grafana/README.md`](infra/grafana/README.md) — dashboards.
- [`crates/gravital-sound-relay/`](crates/gravital-sound-relay/) — código del relay y Dockerfile.

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
