# Gravital Talk

[![CI](https://github.com/angelnereira/gravital-talk/actions/workflows/ci.yml/badge.svg)](https://github.com/angelnereira/gravital-talk/actions/workflows/ci.yml)
[![Build outputs](https://github.com/angelnereira/gravital-talk/actions/workflows/build-outputs.yml/badge.svg)](https://github.com/angelnereira/gravital-talk/actions/workflows/build-outputs.yml)
[![Android APK](https://github.com/angelnereira/gravital-talk/actions/workflows/android.yml/badge.svg)](https://github.com/angelnereira/gravital-talk/actions/workflows/android.yml)
[![Docs](https://github.com/angelnereira/gravital-talk/actions/workflows/docs.yml/badge.svg)](https://github.com/angelnereira/gravital-talk/actions/workflows/docs.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#licencia)

Gravital Talk es un protocolo de comunicación de audio Push-To-Talk en tiempo real escrito en Rust. Transporta voz de baja latencia sobre UDP con cifrado de extremo a extremo, emparejamiento P2P por código QR sin servidor intermediario obligatorio, control de congestión, corrección de errores (FEC) y métricas de calidad en tiempo real. El núcleo es `no_std` compatible y se expone a cualquier lenguaje a través de una FFI estable en C.

**Estado actual: alpha.** La API pública y el protocolo wire son estables dentro de la serie `0.1.x`.

---

## Índice

- [Inicio rápido — Android](#inicio-rápido--android)
- [Inicio rápido — CLI](#inicio-rápido--cli)
- [Arquitectura](#arquitectura)
- [Estado del proyecto](#estado-del-proyecto)
- [Emparejamiento P2P por QR](#emparejamiento-p2p-por-qr)
- [Requisitos](#requisitos)
- [Compilar desde fuente](#compilar-desde-fuente)
- [Binarios pre-compilados](#binarios-pre-compilados)
- [Uso en Rust](#uso-en-rust)
- [Relay de producción](#relay-de-producción)
- [C FFI](#c-ffi)
- [Python SDK](#python-sdk)
- [Web / WASM SDK](#web--wasm-sdk)
- [CLI](#cli)
- [Protocolo](#protocolo)
- [Observabilidad](#observabilidad)
- [Tests](#tests)
- [Infraestructura](#infraestructura)
- [Hoja de ruta](#hoja-de-ruta)
- [Contribuciones](#contribuciones)
- [Licencia](#licencia)

---

## Inicio rápido — Android

### Instalar el APK (última build)

```bash
# El CI genera una APK por cada push; están en outputs/android/debug/
LATEST=$(cat outputs/android/debug/LATEST.txt)
adb install -r "outputs/android/debug/$LATEST"

# Abrir directamente en la pantalla de emparejamiento
adb shell am start -n com.gravitaltalk/.PairingActivity
```

### Usar la app

1. **Persona A → "Crear llamada"** — la app muestra un código QR y espera.
2. **Persona B → "Unirse a llamada" → escanear QR** — la app se conecta automáticamente.
3. Ambos están conectados: mantener pulsado el botón PTT para hablar.
4. Cualquiera puede pulsar **"Colgar"** para desconectarse.

No se requiere ningún servidor intermediario. La app intenta conexión directa LAN → IP pública (STUN) → relay (si se configuró uno).

---

## Inicio rápido — CLI

```bash
# Compilar
cargo build --release -p gravital-talk-cli

# Terminal 1: receptor (abre puerto 9000)
./target/release/gs receive --bind 0.0.0.0:9000

# Terminal 2: emisor (5 segundos desde el micrófono)
./target/release/gs send --peer 127.0.0.1:9000 --duration 5

# O: levantar un relay local y conectar desde otro equipo
./target/release/gs relay --bind 0.0.0.0 --udp-port 9000
./target/release/gs ptt --relay 192.168.1.5:9000
```

---

## Arquitectura

```
┌─────────────────────────────────────────────────────────────────────┐
│  Aplicaciones                                                       │
│  App Android (PairingActivity + PttActivity)                        │
│  CLI gs  ·  Python SDK  ·  JavaScript/WASM                         │
├─────────────────────────────────────────────────────────────────────┤
│  gravital-talk          (facade: CodecSession, re-exports)          │
│  gravital-talk-ffi      (ABI C estable + JNI bridge Android)        │
│  gravital-talk-cli      (binario gs: send/receive/ptt/relay/bench)  │
│  gravital-talk-relay    (daemon: UDP + WebSocket + Prometheus)      │
├─────────────────────────────────────────────────────────────────────┤
│  gravital-talk-transport  (Session, UDP, STUN, FEC, jitter buffer)  │
│  gravital-talk-codec      (Opus, PCM, PLC, negociación)             │
│  gravital-talk-io         (captura/playback: ALSA/CoreAudio/WASAPI) │
│  gravital-talk-metrics    (RTT, jitter, pérdida, MOS)               │
├─────────────────────────────────────────────────────────────────────┤
│  gravital-talk-core   (no_std: header, crypto X25519+ChaCha20, FSM) │
└─────────────────────────────────────────────────────────────────────┘
```

El transporte primario es UDP con DSCP EF. El handshake establece claves con **X25519 ECDH**, las deriva con **HKDF-SHA256** y cifra cada paquete con **ChaCha20-Poly1305 AEAD**.

---

## Estado del proyecto

| Componente | Estado | Notas |
|---|---|---|
| Protocolo wire v1 | ✅ funcional | handshake 4-way, cifrado por paquete, negociación de codec |
| `gravital-talk-core` | ✅ funcional | `no_std`, 54 tests unitarios, proptest |
| `gravital-talk-transport` | ✅ funcional | UDP, STUN, FEC XOR, jitter buffer, congestion control |
| `gravital-talk-codec` | ✅ funcional | PCM pass-through, Opus 64 kbps con PLC |
| `gravital-talk-metrics` | ✅ funcional | RTT EWMA, jitter RFC 3550, pérdida bitmap, MOS estimado |
| `gravital-talk-ffi` | ✅ funcional | ABI C estable, cbindgen, JNI bridge Android |
| `gravital-talk-relay` | ✅ funcional | UDP + WebSocket, /metrics Prometheus, /healthz |
| `gravital-talk-io` | ✅ funcional | cpal (ALSA/CoreAudio/WASAPI/AAudio) |
| `gravital-talk-cli` | ✅ funcional | send, receive, ptt, relay, devices, bench, info, doctor |
| App Android | ✅ funcional | emparejamiento QR, PTT, wake lock, reconexión automática |
| STUN / NAT traversal | ✅ funcional | RFC 5389, stun.l.google.com, fallback P2P → relay |
| PLC (Packet Loss Concealment) | ✅ funcional | CodecSession: hasta 4 frames de silencio por hueco |
| Auto-reconexión CLI | ✅ funcional | gs ptt: backoff 2 s→30 s, reconexión por cambio de red |
| Tonos PTT (CLI + Android) | ✅ funcional | beep 880 Hz al presionar, 440 Hz al soltar |
| Build outputs automático (CI) | ✅ funcional | APK + binarios en outputs/ por cada push |
| Python SDK | ✅ funcional | PyO3 + maturin |
| Web/WASM SDK | ✅ funcional | wasm-bindgen + WebSocket transport |
| Publicación crates.io / PyPI / npm | 🔲 pendiente | roadmap 0.4 |
| Noise Protocol (forward secrecy) | 🔲 pendiente | roadmap 0.3 |
| Swift SDK | 🔲 pendiente | roadmap 0.4 |
| Node.js SDK | 🔲 pendiente | roadmap 0.4 |

---

## Emparejamiento P2P por QR

Gravital Talk conecta dos dispositivos **sin necesidad de un servidor intermediario**. El relay es siempre opcional (fallback para redes con NAT simétrico).

### Cómo funciona

```
Persona A — "Crear llamada"
  1. Crea sesión UDP en puerto efímero
  2. Obtiene IP LAN vía ConnectivityManager
  3. Consulta IP pública vía STUN (stun.l.google.com:19302)
  4. Genera URI:  gravital-talk://pair?v=1&lan=192.168.1.5:48271&pub=203.0.113.45:48271
  5. Muestra QR + código de texto  GRVT-A3F2
  6. Llama handshake_open() — acepta el primer cliente de cualquier IP

Persona B — "Unirse a llamada" → escanea QR
  1. Parsea URI → obtiene lan, pub, relay
  2. Intenta LAN directa           (timeout 2 s)  — funciona en misma WiFi
  3. Intenta IP pública vía STUN   (timeout 5 s)  — funciona en ~85 % de redes
  4. Intenta relay como fallback   (timeout 10 s) — funciona siempre si hay relay
  5. Primer éxito → handshake completo → PTT activo
```

### Formato del URI QR

```
gravital-talk://pair?v=1&lan=<ip_lan>:<puerto>&pub=<ip_publica>:<puerto>&relay=<host:port>
```

Los campos `pub` y `relay` son opcionales. Si STUN falla (ej. sin internet) sólo aparece `lan`. Si no se configura relay, no aparece `relay`.

### Integración STUN (RFC 5389)

```rust
// Rust
use gravital_talk::discover_public_addr;
let addr = discover_public_addr(0).await?;  // 0 = puerto efímero
println!("IP pública: {addr}");

// C FFI
char buf[64];
gs_discover_public_addr(0, buf, sizeof(buf));  // escribe "ip:port"

// Android / Kotlin
val pubAddr: String? = GravitalTalkJni.nativeDiscoverPublicAddr(localPort)
```

### Handshake servidor abierto

```rust
// Acepta el primer cliente de cualquier dirección (modo pairing).
// El peer queda fijado automáticamente tras el primer ClientHello válido.
session.handshake_open().await?;

// C FFI
gs_session_accept_any(handle);

// Android / Kotlin
GravitalTalkJni.nativeAcceptAny(handle)
```

Documentación detallada: [`docs/pairing.md`](docs/pairing.md).

---

## Requisitos

**Toolchain:**

```
Rust >= 1.78 (stable)
Java 17+ (sólo para compilar el APK Android)
```

**Dependencias del sistema (Ubuntu/Debian):**

```bash
sudo apt-get install -y libopus-dev libasound2-dev pkg-config
```

**macOS (Homebrew):**

```bash
brew install opus
```

**Android SDK** (sólo para compilar el APK manualmente):

```bash
# Usar el script incluido (detecta automáticamente Android SDK y NDK)
./scripts/build-android.sh              # sólo arm64-v8a (más rápido)
./scripts/build-android.sh --all-abis  # arm64 + armv7 + x86_64
```

---

## Compilar desde fuente

```bash
git clone https://github.com/angelnereira/Gravital-Talk.git
cd Gravital-Talk

# Verificar el workspace (no requiere ALSA)
cargo check -p gravital-talk-core \
            -p gravital-talk-metrics \
            -p gravital-talk-transport \
            -p gravital-talk \
            -p gravital-talk-ffi

# Build completo (requiere libopus-dev y libasound2-dev)
cargo build --release

# Tests
cargo test --lib --tests \
  -p gravital-talk-core \
  -p gravital-talk-metrics \
  -p gravital-talk-transport \
  -p gravital-talk \
  -p gravital-talk-ffi
```

Targets de Makefile disponibles:

```bash
make check-all     # fmt + clippy + tests
make bench         # benchmarks con criterion
make ffi-smoke     # genera cabecera C y compila smoke test
make python-test   # compila SDK Python y ejecuta pytest
make web-sdk       # compila SDK WASM
```

---

## Binarios pre-compilados

El CI construye y versiona automáticamente los binarios en `outputs/`:

```
outputs/
├── android/
│   ├── debug/    gravital-talk-v<ver>-<sha>-debug.apk
│   └── release/  gravital-talk-v<ver>-<sha>-release-unsigned.apk
├── linux/
│   ├── x86_64/  gs-v<ver>-<sha>-linux-x86_64
│   └── aarch64/ gs-v<ver>-<sha>-linux-aarch64
├── macos/
│   ├── x86_64/  gs-v<ver>-<sha>-macos-x86_64
│   └── aarch64/ gs-v<ver>-<sha>-macos-aarch64
└── windows/
    └── x86_64/  gs-v<ver>-<sha>-windows-x86_64.exe
```

Cada directorio tiene un `LATEST.txt` con el nombre del último build:

```bash
# Instalar última APK
LATEST=$(cat outputs/android/debug/LATEST.txt)
adb install -r "outputs/android/debug/$LATEST"

# Ejecutar CLI en Linux
LATEST=$(cat outputs/linux/x86_64/LATEST.txt)
chmod +x "outputs/linux/x86_64/$LATEST"
./outputs/linux/x86_64/$LATEST --help
```

Para versiones con tag, los assets también se publican como **GitHub Releases**.

---

## Uso en Rust

Añadir al `Cargo.toml`:

```toml
[dependencies]
gravital-talk = { git = "https://github.com/angelnereira/Gravital-Talk" }
tokio = { version = "1", features = ["full"] }
```

### Sesión básica (frames de audio raw)

```rust
use std::sync::Arc;
use gravital_talk::{Config, Session, SessionRole, UdpConfig, UdpTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server_t = Arc::new(UdpTransport::bind(UdpConfig {
        bind_addr: "0.0.0.0:9000".parse()?,
        ..Default::default()
    }).await?);
    let client_t = Arc::new(UdpTransport::bind(UdpConfig {
        bind_addr: "0.0.0.0:0".parse()?,
        ..Default::default()
    }).await?);

    let server = Arc::new(Session::new(server_t, Config::default()));
    let client = Arc::new(Session::new(client_t, Config::default()));

    // Handshake concurrente
    let srv = server.clone();
    let t = tokio::spawn(async move {
        srv.handshake(SessionRole::Server, "127.0.0.1:0".parse().unwrap()).await
    });
    client.handshake(SessionRole::Client, "127.0.0.1:9000".parse()?).await?;
    t.await??;

    // Audio — 20 ms a 48 kHz mono = 1920 bytes PCM 16-bit
    client.ptt_press().await?;
    client.send_audio(&vec![0u8; 1920]).await?;
    let frame = server.recv_audio().await?;
    println!("frame: {} bytes, seq={}", frame.payload.len(), frame.sequence);

    client.ptt_release().await?;
    client.close().await?;
    Ok(())
}
```

### Emparejamiento P2P sin relay

```rust
use gravital_talk::{discover_public_addr, Session, UdpConfig, UdpTransport};
use std::sync::Arc;

// Lado HOST: acepta cualquier cliente
let transport = Arc::new(UdpTransport::bind(UdpConfig::default()).await?);
let local_port = transport.local_addr()?.port();
let public_addr = discover_public_addr(local_port).await?;

println!("QR URI: gravital-talk://pair?v=1&pub={public_addr}");

let session = Arc::new(Session::new(transport, Default::default()));
session.handshake_open().await?;   // acepta el primer peer de cualquier IP
println!("¡Conectado!");

// Lado CLIENTE: conectar parseando el URI QR
let session = Arc::new(Session::new(
    Arc::new(UdpTransport::bind(UdpConfig::default()).await?),
    Default::default(),
));
session.handshake(SessionRole::Client, "203.0.113.45:48271".parse()?).await?;
```

### CodecSession (muestras PCM i16 con PLC)

```rust
use gravital_talk::{CodecId, CodecSession, Config, SessionRole, UdpConfig, UdpTransport};

let transport = Arc::new(UdpTransport::bind(UdpConfig::default()).await?);
let cs = CodecSession::new(transport, Config::default(), CodecId::Pcm)?;
cs.handshake(SessionRole::Client, "127.0.0.1:9000".parse()?).await?;

cs.send_samples(&vec![0i16; 480]).await?;
let samples: Vec<i16> = cs.recv_samples().await?;
// recv_samples() aplica PLC automáticamente si hay gaps de secuencia
```

### Descubrir IP pública vía STUN

```rust
use gravital_talk::discover_public_addr;

let public_addr = discover_public_addr(0).await?;  // 0 = puerto efímero
println!("IP pública: {public_addr}");
// → "203.0.113.45:48271"
```

### Métricas en tiempo real

```rust
let fill = session.jitter_buffer().fill_percent();
let snap = session.metrics().snapshot(fill);
println!("RTT {:.1}ms  Jitter {:.1}ms  Loss {:.1}%  MOS {:.2}",
    snap.rtt_ms, snap.jitter_ms, snap.loss_percent, snap.estimated_mos);
```

---

## Relay de producción

El relay enruta paquetes entre peers sin descifrarlos. Acepta UDP y WebSocket en el mismo proceso.

| Servicio | Puerto por defecto |
|---|---|
| UDP (protocolo nativo) | 9000 |
| WebSocket (navegadores) | 9090 |
| Observabilidad HTTP | 9100 |

```bash
# Docker Compose (relay + Prometheus)
cd crates/gravital-talk-relay
docker compose up -d

# Binario directo
cargo build --release -p gravital-talk-relay
./target/release/gs-relay --config relay.example.toml

# Levantarlo desde el CLI
./target/release/gs relay --bind 0.0.0.0 --udp-port 9000
```

---

## C FFI

```bash
cargo build --release -p gravital-talk-ffi
# → target/release/libgravital_talk_ffi.so  (Linux)
# → target/release/libgravital_talk_ffi.dylib (macOS)
# → target/release/gravital_talk_ffi.dll    (Windows)
```

```c
#include "gravital_talk.h"

GsConfig cfg;
gs_config_default(&cfg);

GsSessionHandle *h = NULL;
gs_session_create(&cfg, "0.0.0.0", 0, &h);

// Descubrir IP pública via STUN
char pub_addr[64];
gs_discover_public_addr(0, pub_addr, sizeof(pub_addr));

// Aceptar cualquier cliente (modo QR pairing)
gs_session_accept_any(h);

// Audio
uint8_t audio[1920] = {0};
gs_session_send_audio(h, audio, sizeof(audio));

gs_session_close(h);
gs_session_destroy(h);
```

### API C — funciones de pairing

| Función | Descripción |
|---|---|
| `gs_discover_public_addr(port, buf, len)` | Descubre IP pública via STUN; escribe `"ip:port"` |
| `gs_session_accept_any(handle)` | Handshake servidor sin conocer la IP del cliente |
| `gs_session_local_port(handle, out_port)` | Puerto UDP local del socket |

### API C — funciones de sesión

| Función | Descripción |
|---|---|
| `gs_config_default(out)` | Rellena configuración por defecto |
| `gs_session_create(cfg, addr, port, out)` | Crea sesión y vincula socket UDP |
| `gs_session_destroy(handle)` | Libera sesión (NULL es no-op) |
| `gs_session_connect(handle, addr, port)` | Handshake como cliente |
| `gs_session_accept(handle, addr, port)` | Handshake como servidor (IP conocida) |
| `gs_session_send_audio(handle, data, len)` | Envía frame de audio |
| `gs_session_recv_audio(handle, buf, len_inout)` | Recibe siguiente frame |
| `gs_session_ptt_press(handle)` | Activa PTT: FloorRequest + ControlResume |
| `gs_session_ptt_release(handle)` | Desactiva PTT: FloorRelease + ControlPause |
| `gs_session_close(handle)` | Cierra sesión enviando CLOSE |
| `gs_session_state(handle, out)` | Estado actual |
| `gs_session_id(handle, out)` | Session ID negociado |
| `gs_session_metrics(handle, out)` | Snapshot de métricas |
| `gs_error_last()` | Último error del hilo (C-string, no liberar) |

---

## Python SDK

```bash
pip install maturin
cd sdks/python
maturin develop --release
```

```python
import gravital_talk as gt

server = gt.Session(gt.Config(sample_rate=48000), bind_addr="0.0.0.0", bind_port=9000)
client = gt.Session(gt.Config(sample_rate=48000), bind_addr="0.0.0.0", bind_port=0)

import threading
t = threading.Thread(target=server.accept, args=("127.0.0.1", client.local_port))
t.start()
client.connect("127.0.0.1", 9000)
t.join()

client.send_audio(bytes(1920))
data = server.recv_audio()

m = client.metrics()
print(f"RTT: {m.rtt_ms:.1f} ms  MOS: {m.estimated_mos:.2f}")
client.close(); server.close()
```

---

## Web / WASM SDK

```bash
npm install -g wasm-pack
cd sdks/web
wasm-pack build --target web --out-dir pkg --release
```

```typescript
import init, { GravitalTalkSession } from "./pkg/gravital_talk_web.js";

await init();
const session = new GravitalTalkSession();
await session.connect("wss://relay.host:9090/session/abc123");
await session.sendAudio(new Float32Array(480));
const pcm = await session.recvAudio();
session.close();
```

---

## CLI

```bash
cargo install --path crates/gravital-talk-cli
```

```
gs send     --peer <addr:port> [--codec pcm|opus] [--duration 5]
gs receive  --bind <addr:port> [--output audio.wav]
gs ptt      --relay <host:port>          # PTT interactivo con reconexión automática
gs relay    --bind 0.0.0.0               # Levantar relay local
gs devices                               # Listar dispositivos de audio
gs bench    --peer <addr:port>           # Benchmark p50/p95/p99
gs info     --peer <addr:port>
gs doctor                                # Diagnóstico: audio, red, dependencias
```

Ejemplo PTT con reconexión automática:

```bash
# Lado servidor (relay local)
gs relay --bind 0.0.0.0 --udp-port 9000

# Lado cliente — Ctrl+Space para hablar, Ctrl+C para salir
# Se reconecta solo si la red cae (backoff 2 s→30 s)
gs ptt --relay 127.0.0.1:9000
```

---

## Protocolo

Handshake 4-way con X25519 ECDH + HKDF-SHA256:

```
Cliente                         Servidor
   |── ClientHello (X25519 pub) ──▶|
   |◀── ServerHello (X25519 pub) ──|
   |── KeyExchange (auth_tag) ────▶|
   |◀── SessionConfirm (sess_id) ──|
   |◀══════ audio ChaCha20 ═══════▶|
```

Cabecera de 24 bytes por paquete. Payload cifrado con ChaCha20-Poly1305 AEAD; el nonce de 96 bits deriva de `sequence` + `session_id`. La cabecera es AAD, garantizando su integridad sin firma extra.

Especificación completa: [`docs/protocol-spec.md`](docs/protocol-spec.md).

---

## Observabilidad

Métricas Prometheus expuestas en `http://<relay>:9100/metrics`:

```
gs_relay_packets_in_total       gs_relay_packets_out_total
gs_relay_bytes_in_total         gs_relay_bytes_out_total
gs_relay_active_sessions        gs_relay_ws_connections
gs_relay_dropped_total{reason}
```

Dashboard Grafana: `infra/grafana/dashboards/gravital-fleet-overview.json`.

---

## Tests

```bash
cargo test -p gravital-talk-core       # 54 tests: FSM, crypto, codec, fragmentación
cargo test -p gravital-talk-transport  # 35 tests: STUN, jitter, FEC, congestión
cargo test -p gravital-talk-metrics    # 22 tests: RTT, jitter, pérdida, MOS
cargo test -p gravital-talk-ffi        # 5 tests: ABI C, null safety

# Integración — loopback UDP real
cargo test --test ptt_floor_control -p gravital-talk
cargo test --test handshake_flow    -p gravital-talk
cargo test --test net_sim           -p gravital-talk
cargo test --test opus_roundtrip    -p gravital-talk

# Benchmarks
cargo bench -p gravital-talk
```

---

## Infraestructura

```
infra/
├── terraform/modules/
│   ├── relay-aws/         EC2 t4g.small ARM64 (~$12/mes)
│   ├── relay-hetzner/     CX22 (~€4/mes)
│   └── relay-digitalocean/ Droplet (~$6/mes)
├── helm/gravital-talk-relay/   Helm chart Kubernetes
├── grafana/dashboards/         Dashboard Grafana
└── cloud-init/raspberry-pi.yml  Pi 4/5 con Pi OS Lite ARM64
```

```bash
# Docker
docker run -p 9000:9000/udp -p 9090:9090 -p 9100:9100 \
  ghcr.io/angelnereira/gravital-talk-relay:latest

# Kubernetes
helm install gravital-talk-relay ./infra/helm/gravital-talk-relay \
  --set image.tag=0.1.0-alpha.1
```

---

## Hoja de ruta

| Versión | Estado | Contenido |
|---|---|---|
| **0.1.0-alpha.1** | ✅ | Protocolo core, UDP, FFI, CLI MVP, SDKs Python/WASM |
| **0.2.0-alpha.1** | ✅ | Codec Opus, audio I/O cpal, CLI con `--device` |
| **0.2.0-alpha.2** | ✅ | Negociación codec, resampler, relay productivo, Terraform/Helm |
| **0.2.0-alpha.3** | ✅ | **App Android** (PairingActivity, QR, CameraX), **STUN** RFC 5389, **PLC**, auto-reconexión CLI, tonos PTT, CI auto-build `outputs/` |
| **0.3** | 🔲 | Noise Protocol (forward secrecy), rate limiting, relay cluster Redis |
| **0.4** | 🔲 | SDKs Swift + Node.js, publicación crates.io / PyPI / npm |
| **1.0** | 🔲 | Protocolo estable, auditoría de seguridad, SemVer |

---

## Contribuciones

Ver `CONTRIBUTING.md`. Resumen:

1. Abrir issue antes de implementar cambios grandes.
2. Ramas con prefijo `feat/`, `fix/` o `refactor/`.
3. Pasar `cargo fmt --check`, `cargo clippy -- -D warnings` y todos los tests.
4. El `unsafe` requiere comentario `// SAFETY:` justificando las invariantes.
5. Commits en Conventional Commits (`feat:`, `fix:`, `docs:`, etc.).

Para vulnerabilidades de seguridad: ver `SECURITY.md`.

---

## Licencia

Dual licencia MIT y Apache 2.0.

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

Copyright 2024–2026 Angel Nereira / Nereira Technology and Business Solutions.
