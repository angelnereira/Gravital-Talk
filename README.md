# Gravital Talk

[![CI](https://github.com/angelnereira/gravital-talk/actions/workflows/ci.yml/badge.svg)](https://github.com/angelnereira/gravital-talk/actions/workflows/ci.yml)
[![Docs](https://github.com/angelnereira/gravital-talk/actions/workflows/docs.yml/badge.svg)](https://github.com/angelnereira/gravital-talk/actions/workflows/docs.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#licencia)

Gravital Talk es un protocolo de comunicación de audio en tiempo real escrito en Rust. Transporta audio de baja latencia sobre UDP con cifrado de extremo a extremo, control de congestión, corrección de errores por reenvío (FEC) y métricas de calidad en tiempo real. El núcleo es `no_std` compatible y se expone a cualquier lenguaje a través de una capa FFI estable en C.

**Estado actual: alpha.** La API pública y el protocolo wire son estables dentro de la serie `0.1.x`. Los crates no están publicados en registros públicos todavía; la distribución es por fuente o por artefactos de GitHub Release.

---

## Indice

- [Arquitectura](#arquitectura)
- [Estado del proyecto](#estado-del-proyecto)
- [Requisitos](#requisitos)
- [Compilar desde fuente](#compilar-desde-fuente)
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

## Arquitectura

El proyecto se organiza en tres capas:

```
┌─────────────────────────────────────────────────────────────┐
│  Aplicaciones y SDKs                                        │
│  Rust  ·  C/C++  ·  Python  ·  JavaScript/WASM             │
├─────────────────────────────────────────────────────────────┤
│  gravital-talk          (facade: re-exports, CodecSession)  │
│  gravital-talk-ffi      (ABI C estable, cbindgen)           │
│  gravital-talk-cli      (binario gs)                        │
│  gravital-talk-relay    (daemon de relay)                   │
├─────────────────────────────────────────────────────────────┤
│  gravital-talk-transport  (Session, UDP, WebSocket, FEC)    │
│  gravital-talk-codec      (Opus, PCM, negociación)          │
│  gravital-talk-io         (captura/reproducción hardware)   │
│  gravital-talk-metrics    (RTT, jitter, pérdida, MOS)       │
├─────────────────────────────────────────────────────────────┤
│  gravital-talk-core   (no_std: header, crypto, FSM, FEC)   │
└─────────────────────────────────────────────────────────────┘
```

El transporte primario es UDP con DSCP EF (Expedited Forwarding). Las conexiones desde navegador usan WebSocket como transporte alternativo. El handshake establece claves con X25519 ECDH, las deriva con HKDF-SHA256 y cifra cada paquete con ChaCha20-Poly1305 AEAD.

---

## Estado del proyecto

| Componente | Estado | Notas |
|---|---|---|
| Protocolo wire v1 | funcional | handshake 4-way, cifrado por paquete, negociación de codec |
| `gravital-talk-core` | funcional | `no_std`, 54 tests unitarios, proptest |
| `gravital-talk-transport` | funcional | UDP, WebSocket, FEC XOR, jitter buffer, congestion control |
| `gravital-talk-codec` | funcional | PCM pass-through, Opus 64 kbps con PLC y FEC |
| `gravital-talk-metrics` | funcional | RTT EWMA, jitter RFC 3550, pérdida bitmap, MOS estimado |
| `gravital-talk-ffi` | funcional | ABI C estable, header auto-generado con cbindgen |
| `gravital-talk-relay` | funcional | UDP + WebSocket, /metrics Prometheus, /healthz |
| `gravital-talk-io` | funcional | cpal (ALSA/CoreAudio/WASAPI/AAudio), requiere libopus |
| `gravital-talk-cli` | funcional | send, receive, devices, bench, info, doctor |
| Python SDK | funcional | PyO3 + maturin, requiere Rust toolchain para compilar |
| Web/WASM SDK | funcional | wasm-bindgen + wasm-pack, WebSocket transport |
| Swift SDK | pendiente | roadmap 0.4 |
| Kotlin/Android SDK | pendiente | roadmap 0.4 |
| Node.js SDK | pendiente | roadmap 0.4 |
| Publicación en crates.io / PyPI / npm | pendiente | roadmap 0.2 |
| Noise Protocol (forward secrecy) | pendiente | roadmap 0.3 |
| STUN/NAT traversal | pendiente | roadmap 0.3 |

---

## Requisitos

**Toolchain:**

```
Rust >= 1.78 (stable)
```

**Dependencias del sistema (Ubuntu/Debian):**

```bash
sudo apt-get install -y libopus-dev libasound2-dev pkg-config
```

**macOS (Homebrew):**

```bash
brew install opus
```

`libasound2-dev` es específico de Linux (ALSA). En macOS y Windows el audio usa CoreAudio y WASAPI respectivamente — no requieren dependencias adicionales.

Para la compilación cruzada a ARM64:

```bash
cargo install cross
```

---

## Compilar desde fuente

```bash
git clone https://github.com/angelnereira/Gravital-Talk.git
cd Gravital-Talk

# Verificar todo el workspace (excluye crates que requieren ALSA si no está instalado)
cargo check -p gravital-talk-core \
            -p gravital-talk-metrics \
            -p gravital-talk-transport \
            -p gravital-talk \
            -p gravital-talk-ffi

# Build completo (requiere libopus-dev y libasound2-dev)
cargo build --release

# Ejecutar todos los tests
cargo test --lib --tests \
  -p gravital-talk-core \
  -p gravital-talk-metrics \
  -p gravital-talk-transport \
  -p gravital-talk \
  -p gravital-talk-ffi
```

Los targets de Makefile disponibles:

```bash
make check-all     # fmt + clippy + tests
make bench         # benchmarks con criterion
make ffi-smoke     # genera cabecera C y compila el smoke test
make python-test   # compila el SDK Python y ejecuta pytest
make web-sdk       # compila el SDK WASM
```

---

## Uso en Rust

Añadir al `Cargo.toml`:

```toml
[dependencies]
gravital-talk = { git = "https://github.com/angelnereira/Gravital-Talk" }
tokio = { version = "1", features = ["full"] }
```

### Sesión básica (bytes de audio raw)

```rust
use std::sync::Arc;
use gravital_talk::{Config, Session, SessionRole, UdpConfig, UdpTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // --- lado servidor ---
    let server_transport = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: "0.0.0.0:9000".parse()?,
            ..Default::default()
        })
        .await?,
    );
    let server = Arc::new(Session::new(server_transport, Config::default()));

    // --- lado cliente ---
    let client_transport = Arc::new(
        UdpTransport::bind(UdpConfig {
            bind_addr: "0.0.0.0:0".parse()?,
            ..Default::default()
        })
        .await?,
    );
    let client = Arc::new(Session::new(client_transport, Config::default()));

    // Handshake concurrente
    let srv = server.clone();
    let server_task = tokio::spawn(async move {
        srv.handshake(SessionRole::Server, "127.0.0.1:0".parse().unwrap()).await
    });
    client.handshake(SessionRole::Client, "127.0.0.1:9000".parse()?).await?;
    server_task.await??;

    // Enviar y recibir un frame de audio (PCM raw, 20 ms a 48 kHz mono = 960 bytes)
    let payload = vec![0u8; 960];
    client.send_audio(&payload).await?;
    let frame = server.recv_audio().await?;
    println!("frame recibido: {} bytes, seq={}", frame.payload.len(), frame.sequence);

    client.close().await?;
    Ok(())
}
```

### Sesión con codec (muestras PCM i16)

`CodecSession` añade la capa de codec por encima de `Session`:

```rust
use std::sync::Arc;
use gravital_talk::{CodecId, CodecSession, Config, SessionRole, UdpConfig, UdpTransport};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let transport = Arc::new(
        UdpTransport::bind(UdpConfig::default()).await?,
    );
    let session = CodecSession::new(transport, Config::default(), CodecId::Pcm)?;
    session.handshake(SessionRole::Client, "127.0.0.1:9000".parse()?).await?;

    // Enviar 480 muestras (20 ms a 24 kHz mono)
    let samples = vec![0i16; 480];
    session.send_samples(&samples).await?;

    // Recibir muestras decodificadas
    let received: Vec<i16> = session.recv_samples().await?;
    println!("muestras recibidas: {}", received.len());

    Ok(())
}
```

### Métricas en tiempo real

```rust
let fill = session.jitter_buffer().fill_percent();
let snap = session.metrics().snapshot(fill);

println!("RTT:      {:.1} ms", snap.rtt_ms);
println!("Jitter:   {:.1} ms", snap.jitter_ms);
println!("Perdida:  {:.1}%",   snap.loss_percent);
println!("MOS est.: {:.2}",    snap.estimated_mos);
println!("Enviados: {} paquetes / {} bytes", snap.packets_sent, snap.bytes_sent);
```

El MOS estimado sigue el modelo E-Model (ITU-T G.107) simplificado con rango 1.0–5.0. Un valor superior a 4.0 corresponde a calidad de voz excelente.

---

## Relay de producción

El relay enruta paquetes entre peers sin descifrarlos. Acepta UDP y WebSocket en el mismo proceso.

**Puertos por defecto:**

| Servicio | Puerto |
|---|---|
| UDP (protocolo) | 9000 |
| WebSocket | 9090 |
| Observabilidad HTTP | 9100 |

### Docker Compose

```bash
cd crates/gravital-talk-relay
docker compose up -d
```

El `docker-compose.yml` incluido levanta el relay y un Prometheus que lo raspa cada 5 segundos.

### Binario directo

```bash
cargo build --release -p gravital-talk-relay
./target/release/gs-relay --config crates/gravital-talk-relay/relay.example.toml
```

Configuración mínima (`relay.toml`):

```toml
udp_bind          = "0.0.0.0:9000"
ws_bind           = "0.0.0.0:9090"
observability_bind = "0.0.0.0:9100"
```

El relay no guarda estado de sesión en disco. Admite hasta dos peers por sesión; el tercero es rechazado. Las sesiones sin actividad durante el TTL configurado se eliminan automáticamente.

---

## C FFI

La cabecera `crates/gravital-talk-ffi/include/gravital_talk.h` se genera automáticamente con cbindgen al compilar el crate. El ABI es estable en la versión `GS_ABI_VERSION = 1`.

### Compilar la biblioteca

```bash
cargo build --release -p gravital-talk-ffi
# Produce: target/release/libgravital_talk_ffi.so  (Linux)
#          target/release/libgravital_talk_ffi.dylib (macOS)
#          target/release/gravital_talk_ffi.dll     (Windows)
```

### Ejemplo en C

```c
#include "gravital_talk.h"
#include <stdio.h>

int main(void) {
    GsConfig cfg;
    gs_config_default(&cfg);

    GsSessionHandle *session = NULL;
    GsStatus st = gs_session_create(&cfg, "0.0.0.0", 0, &session);
    if (st != GS_OK) {
        fprintf(stderr, "error: %s\n", gs_error_last());
        return 1;
    }

    st = gs_session_connect(session, "127.0.0.1", 9000);
    if (st != GS_OK) {
        fprintf(stderr, "handshake: %s\n", gs_error_last());
        gs_session_destroy(session);
        return 1;
    }

    uint8_t audio[960] = {0};
    gs_session_send_audio(session, audio, sizeof(audio));

    GsMetrics m;
    gs_session_metrics(session, &m);
    printf("MOS estimado: %.2f\n", m.estimated_mos);

    gs_session_close(session);
    gs_session_destroy(session);
    return 0;
}
```

Compilar:

```bash
cc -I crates/gravital-talk-ffi/include \
   -L target/release \
   -lgravital_talk_ffi \
   -o mi_app mi_app.c
```

### Referencia de la API C

| Función | Descripción |
|---|---|
| `gs_config_default(out)` | Rellena una configuración con valores por defecto |
| `gs_session_create(cfg, addr, port, out)` | Crea una sesión y vincula un socket UDP |
| `gs_session_destroy(handle)` | Libera la sesión (NULL es no-op seguro) |
| `gs_session_connect(handle, addr, port)` | Handshake como cliente |
| `gs_session_accept(handle, addr, port)` | Handshake como servidor |
| `gs_session_send_audio(handle, data, len)` | Envía un frame de audio |
| `gs_session_recv_audio(handle, buf, len_inout)` | Recibe el siguiente frame |
| `gs_session_close(handle)` | Cierra la sesión enviando CLOSE |
| `gs_session_state(handle, out)` | Estado actual de la sesión |
| `gs_session_id(handle, out)` | Session ID negociado |
| `gs_session_metrics(handle, out)` | Snapshot de métricas |
| `gs_error_last()` | Último error del hilo actual (C-string, no liberar) |
| `gs_error_clear()` | Limpia el buffer de error |
| `gs_version()` | Versión del crate (C-string estática) |
| `gs_protocol_version()` | Versión del protocolo wire |
| `gs_abi_version()` | Versión del ABI C |
| `gs_ping()` | Retorna 0; útil como smoke test de linkado |

---

## Python SDK

El SDK Python usa PyO3 y maturin. Requiere Rust instalado para compilar el módulo nativo.

### Instalación desde fuente

```bash
pip install maturin
cd sdks/python
maturin develop --release
```

### Uso

```python
import gravital_talk as gt

# Crear sesiones
server = gt.Session(config=gt.Config(sample_rate=48000, channels=1), bind_addr="0.0.0.0", bind_port=9000)
client = gt.Session(config=gt.Config(sample_rate=48000, channels=1), bind_addr="0.0.0.0", bind_port=0)

# Handshake (bloqueante, usar threading si es necesario)
import threading
t = threading.Thread(target=server.accept, args=("127.0.0.1", client.local_port))
t.start()
client.connect("127.0.0.1", server.local_port)
t.join()

# Enviar audio (bytes, por ejemplo PCM raw)
payload = bytes(960)
client.send_audio(payload)

# Recibir audio
data = server.recv_audio()  # devuelve bytes

# Métricas
m = client.metrics()
print(f"RTT: {m.rtt_ms:.1f} ms  MOS: {m.estimated_mos:.2f}")

client.close()
server.close()
```

### API de referencia

| Clase / Método | Tipo de retorno | Descripción |
|---|---|---|
| `Config(sample_rate, channels, frame_duration_ms, jitter_buffer_ms)` | `Config` | Configuración de sesión |
| `Session(config, bind_addr, bind_port)` | `Session` | Crea y vincula una sesión UDP |
| `session.connect(host, port)` | `None` | Handshake como cliente (bloqueante) |
| `session.accept(host, port)` | `None` | Handshake como servidor (bloqueante) |
| `session.send_audio(data: bytes)` | `None` | Envía un frame de audio |
| `session.recv_audio()` | `bytes` | Recibe el siguiente frame (bloqueante) |
| `session.close()` | `None` | Cierra la sesión |
| `session.session_id` | `int` | Session ID negociado |
| `session.local_port` | `int` | Puerto UDP local |
| `session.local_addr` | `str` | Dirección local completa |
| `session.metrics()` | `Metrics` | Snapshot de métricas |
| `Metrics.rtt_ms` | `float` | RTT estimado en milisegundos |
| `Metrics.jitter_ms` | `float` | Jitter RFC 3550 en milisegundos |
| `Metrics.loss_percent` | `float` | Porcentaje de pérdida |
| `Metrics.estimated_mos` | `float` | MOS estimado (1.0–5.0) |
| `Metrics.packets_sent` | `int` | Paquetes enviados |
| `Metrics.packets_received` | `int` | Paquetes recibidos |

---

## Web / WASM SDK

El SDK para navegador compila el protocolo a WebAssembly y usa WebSocket como transporte (el protocolo UDP no está disponible desde navegadores).

### Compilar

```bash
npm install -g wasm-pack
cd sdks/web
wasm-pack build --target web --out-dir pkg --release
```

Esto produce el directorio `pkg/` con el módulo WASM y los bindings TypeScript.

### Uso desde JavaScript/TypeScript

```typescript
import init, { GravitalTalkSession } from "./pkg/gravital_talk_web.js";

await init();

const session = new GravitalTalkSession();
await session.connect("wss://relay.gravitaltalk.dev/session/abc123");

// Enviar audio (Float32Array desde Web Audio API)
const audioData = new Float32Array(480);
await session.sendAudio(audioData);

// Recibir audio
const received = await session.recvAudio();  // Float32Array

session.close();
```

La integración con la Web Audio API se realiza mediante un `AudioWorklet`. Un ejemplo completo se encuentra en `sdks/web/examples/browser-demo/`.

---

## CLI

El binario `gs` es la herramienta de línea de comandos para probar y diagnosticar sesiones.

```bash
cargo install --path crates/gravital-talk-cli
```

Subcomandos disponibles:

```
gs send     --peer <addr:port> [--codec pcm|opus] [--duration 5]
            Captura desde el micrófono y envía al peer.

gs receive  --bind <addr:port> [--output audio.wav]
            Recibe audio y lo escribe a WAV o lo reproduce por los altavoces.

gs devices
            Lista los dispositivos de audio de entrada y salida disponibles.

gs bench    --peer <addr:port>
            Envía 1000 frames de loopback y mide latencia (p50/p95/p99).

gs info     --peer <addr:port>
            Consulta el estado de sesión de un peer.

gs doctor
            Diagnóstico del sistema: audio, red, dependencias.
```

Ejemplo de sesión loopback local:

```bash
# Terminal 1: receptor
gs receive --bind 0.0.0.0:9000 --output salida.wav

# Terminal 2: emisor
gs send --peer 127.0.0.1:9000 --duration 5
```

---

## Protocolo

Gravital Talk v1 implementa un handshake de 4 mensajes:

```
Cliente                         Servidor
   |                               |
   |-- ClientHello (X25519 pub) -->|
   |<-- ServerHello (X25519 pub) --|
   |-- KeyExchange (HKDF derive) ->|
   |<-- SessionConfirm (sess_id) --|
   |                               |
   |<====== audio cifrado ========>|
```

Cada paquete tiene una cabecera fija de 24 bytes:

```
 0       1       2       3
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |  MAGIC (2B)   |  VER  | TYPE  |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |          SESSION ID (4B)      |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |           SEQUENCE (4B)       |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |           TIMESTAMP (8B)      |
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
 |  FLAGS  | RSV |  CHECKSUM (2B)|
 +-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
```

El payload va cifrado con ChaCha20-Poly1305. El nonce de 96 bits se deriva del `sequence` y la clave de sesión. La AAD incluye la cabecera completa, lo que garantiza integridad del header sin necesidad de firma separada.

La especificación completa está en `docs/protocol-spec.md`.

---

## Observabilidad

El relay expone métricas en formato Prometheus en `http://<host>:9100/metrics`:

```
gs_relay_packets_in_total
gs_relay_packets_out_total
gs_relay_bytes_in_total
gs_relay_bytes_out_total
gs_relay_active_sessions
gs_relay_ws_connections
gs_relay_dropped_total
```

Las sesiones individuales exponen métricas mediante la API Rust/C/Python. El dashboard de Grafana está en `infra/grafana/dashboards/gravital-fleet-overview.json`.

---

## Tests

El proyecto cuenta con tres niveles de prueba:

**Tests unitarios y de propiedades:**

```bash
cargo test -p gravital-talk-core       # 54 tests: FSM, crypto, codec, fragmentación
cargo test -p gravital-talk-transport  # 24 tests: jitter buffer, FEC, congestión
cargo test -p gravital-talk-metrics    # 22 tests: RTT, jitter, pérdida, MOS
cargo test -p gravital-talk-ffi        # 5 tests: ABI C, null safety, create/destroy
```

**Tests de integración (SimTransport + UDP real):**

```bash
cargo test --test handshake_flow -p gravital-talk   # 5 tests: negociación, timeout
cargo test --test net_sim        -p gravital-talk   # 5 tests: 0%/20% pérdida, 10% pérdida
cargo test --test session_lifecycle -p gravital-talk
cargo test --test stress         -p gravital-talk   # 5 tests: 500 frames, burst, bidireccional
cargo test --test opus_roundtrip -p gravital-talk   # SNR > 60 dB, energía Opus > 10%
```

**Tests con pérdida de paquetes simulada:**

Los tests `net_sim` utilizan `SimTransport`, un transporte en memoria con inyección de pérdida configurable. El test `sim_handshake_survives_20pct_loss` verifica que el handshake completa con hasta 20% de pérdida dentro de 15 segundos.

**Benchmarks:**

```bash
cargo bench -p gravital-talk
```

Targets de rendimiento: decodificación de cabecera < 100 ns, CRC-16 < 20 ns/byte, latencia loopback < 1 ms (localhost).

---

## Infraestructura

El directorio `infra/` contiene todo lo necesario para desplegar el relay en producción:

**Terraform (módulos):**

```
infra/terraform/modules/
  relay-aws/          EC2 t4g.small ARM64, ~$12/mes, Route53
  relay-hetzner/      CX22, ~4 EUR/mes
  relay-digitalocean/ Droplet, ~$6/mes
  edge-node/          cloud-init para nodos cliente
```

**Kubernetes:**

```bash
helm install gravital-talk-relay ./infra/helm/gravital-talk-relay \
  --set image.tag=0.1.0-alpha.1
```

**Raspberry Pi:**

```bash
# cloud-init incluido en infra/cloud-init/raspberry-pi.yml
# Compatible con Raspberry Pi OS Lite ARM64 (Pi 4 y 5)
```

**Docker:**

```bash
docker pull ghcr.io/angelnereira/gravital-talk-relay:latest
docker run -p 9000:9000/udp -p 9090:9090 -p 9100:9100 \
  ghcr.io/angelnereira/gravital-talk-relay:latest
```

---

## Hoja de ruta

**0.2 — Publicación en registros:**
- crates.io (gravital-talk, gravital-talk-core, gravital-talk-transport)
- PyPI (gravital-talk)
- npm (@gravital/talk-web)
- GitHub Container Registry para la imagen del relay

**0.3 — Seguridad avanzada:**
- Noise Protocol (NK pattern) para forward secrecy y autenticación de servidor
- Token bucket para rate limiting por sesión
- STUN para NAT traversal

**0.4 — SDKs nativos:**
- Swift / XCFramework / Swift Package Manager (iOS, macOS)
- Kotlin / JNI / AAR (Android)
- Node.js / napi-rs

**0.5 — Escalabilidad del relay:**
- Modo cluster con coordinación vía Redis
- WebTransport como alternativa a WebSocket para navegadores

---

## Contribuciones

El proceso de contribución está descrito en `CONTRIBUTING.md`. En resumen:

1. Abrir un issue describiendo el cambio propuesto antes de implementarlo.
2. Crear una rama desde `main` con el prefijo `feat/`, `fix/` o `refactor/`.
3. El código debe pasar `cargo fmt --check`, `cargo clippy -- -D warnings` y todos los tests.
4. Las funciones `unsafe` deben incluir un comentario `// SAFETY:` explicando las invariantes.
5. Los commits siguen Conventional Commits (`feat:`, `fix:`, `docs:`, etc.).

Para reportar vulnerabilidades de seguridad, ver `SECURITY.md`.

---

## Licencia

Dual licencia MIT y Apache 2.0. Puede elegir cualquiera de las dos.

- [MIT License](LICENSE-MIT)
- [Apache License 2.0](LICENSE-APACHE)

Copyright 2024–2026 Angel Nereira / Nereira Technology and Business Solutions.
