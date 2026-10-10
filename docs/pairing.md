# Emparejamiento P2P — Guía técnica

## Problema

El flujo original de Gravital Talk requería que ambos usuarios conocieran la IP del relay y el identificador de sesión antes de conectar. Esto obliga a intercambiar información fuera de banda (email, chat, etc.) y no funciona sin un relay intermedio.

El modo de emparejamiento por QR/código resuelve esto: el host genera un código de un solo uso que el cliente escanea, y la conexión se establece automáticamente — sin relay obligatorio.

---

## Estrategia de conectividad

La invitación lleva **varias direcciones candidatas** del anfitrión y,
opcionalmente, un servidor de encuentro. El invitado no las prueba en orden:
sonda todas a la vez y conecta por la primera que contesta.

| Situación | Camino | Qué hace falta |
|---|---|---|
| Misma red | Dirección local (`host`) | Nada |
| Redes distintas, anfitrión alcanzable desde fuera (IP pública, puerto reenviado) | Dirección pública (`pub`) | Nada |
| Redes distintas, ambos tras NAT | Dirección pública, tras perforar | Servidor de encuentro (`rv`) |
| Los dos lados con NAT simétrico | Ninguno directo | Que el relay reenvíe el audio (no automático todavía) |

Entre dos caminos que contestan se prefiere el local: se le dan 150 ms de
margen frente a uno público que haya contestado antes.

---

## URI canónica del QR

```
gravital-talk://pair?v=1&host=192.168.0.13&room=GRVT-A3F2&udp=40958&token=…&pub=203.0.113.45:40958&rv=relay.example.com:9000
```

| Parámetro | Descripción | Obligatorio |
|-----------|-------------|-------------|
| `v` | Versión del formato (actualmente `1`) | Sí |
| `host` | IP del anfitrión en su red local | Sí |
| `room` | Código de sala legible (`XXXX-NNNN`) | Sí |
| `udp` | Puerto UDP real del socket de la sala | Sí |
| `token` | Secreto de la sala (PSK de Noise) | Sí en salas creadas por la app |
| `pub` | `ip:puerto` público de **ese mismo socket**, visto por STUN | No |
| `rv` | `host:puerto` del servidor de encuentro que usa el anfitrión | No |

`pub` y `rv` son aditivos: un cliente anterior los ignora y conecta por
`host:udp` como antes. `rv` viaja en la invitación para que el invitado no
tenga que configurar nada: los dos tienen que registrarse en el mismo servidor.

---

## Perforación de NAT

Implementado en `crates/gravital-talk-transport/src/rendezvous.rs` (protocolo
y registro del servidor) y `session.rs` (`select_path`, `handshake_any` y el
lazo de espera del anfitrión). El servidor corre dentro de `gs-relay`, en su
puerto UDP.

### Por qué hace falta un servidor

Un NAT sólo deja entrar lo que responde a algo que salió antes. Para que el
paquete del invitado entre, el anfitrión tiene que haber enviado uno hacia la
dirección pública del invitado, y no tiene de dónde sacarla: el QR sólo viaja
de anfitrión a invitado. El servidor de encuentro cubre ese hueco y nada más:
**no reenvía audio y no conoce el token de la sala**.

### Secuencia

```
Anfitrión (tras NAT A)        Servidor de encuentro        Invitado (tras NAT B)
   │── REGISTER(clave) ──────────►│                                │
   │◄── PEERS(tú=A, otros=[]) ────│                                │
   │        (cada 250 ms)         │◄────────── REGISTER(clave) ────│
   │                              │── PEERS(tú=B, otros=[A]) ─────►│
   │── REGISTER ─────────────────►│                                │
   │◄── PEERS(tú=A, otros=[B]) ───│                                │
   │── PUNCH ──────────────────────────────── (abre NAT A hacia B) │
   │◄─────────────────────────────────────────────────── PUNCH ────│
   │── PUNCH_ACK ─────────────────────────────────────────────────►│
   │◄══════════ handshake Noise y audio, directo ═════════════════►│
```

La clave de sala es `SHA-256("gravital-talk/rendezvous/v1\0" ‖ token)[..16]`:
empareja a quienes tienen el mismo token sin revelarlo, y el prefijo la separa
del PSK de Noise.

### Formato

Fuera del formato de paquete de Gravital Talk: son mensajes previos a la
sesión y los contesta un servidor que no participa en ella.

```
"GTRV" (4) | versión (1) | tipo (1) | clave de sala (16) | cuerpo
```

| Tipo | Código | Sentido | Cuerpo |
|---|---|---|---|
| `REGISTER` | 1 | extremo → servidor | relleno hasta 128 B |
| `PEERS` | 2 | servidor → extremo | dirección de quien pregunta, nº de peers (1), sus direcciones |
| `PUNCH` | 3 | extremo → extremo | nonce (8) |
| `PUNCH_ACK` | 4 | extremo → extremo | el mismo nonce |

Una dirección es `familia (1: 4 ó 6) | IP (4 ó 16) | puerto (2, big-endian)`.

### Defensas

- **Sin amplificación.** `REGISTER` va rellenado a 128 B y `PEERS` nunca los
  supera (4 peers IPv6 como mucho). Un `REGISTER` más corto no se contesta.
- **Sólo se responde a quien pregunta.** El servidor no envía nada a los demás
  registrados; se enteran en su siguiente `REGISTER`.
- **Las sondas llevan la clave de la sala.** Un extremo no contesta `PUNCH` de
  quien no la conoce.
- **Acotado.** 4 peers por sala, registros de 30 s, tope de salas igual a
  `max_sessions` del relay, y el limitador por IP del relay aplica antes.
- Nada de esto autentica: quien conecta lo demuestra después, en el handshake
  Noise con el token.

### Límites conocidos

- **NAT simétrico en los dos lados**: no hay perforación posible. Con uno
  solo suele funcionar, porque el otro extremo aprende la dirección real del
  `PUNCH` que le llega (siempre que su NAT no filtre por puerto).
- **Routers que aceptan tráfico no solicitado en su IP pública** en vez de
  descartarlo: conntrack da por buena la conexión entrante y al salir después
  el equipo de dentro le cambia el puerto. Reproducible con
  `PERMISSIVE_INPUT=1 scripts/nat-sim.sh`. El cortafuegos por defecto de un
  router doméstico lo descarta y no ocurre.
- **Sin relevo automático al relay** cuando la perforación falla.

### Verificación

| Qué | Dónde |
|---|---|
| Protocolo, registro y anti-amplificación | tests de `rendezvous.rs` |
| Perforación con NAT cono, simétrico y filtrado por IP o por puerto (red simulada) | `crates/gravital-talk-transport/tests/nat_punch.rs` |
| Servidor de encuentro del relay sobre UDP real | `crates/gravital-talk-relay/tests/rendezvous.rs` |
| NAT real del kernel (namespaces + iptables), con control negativo | `make nat-sim` |

---

## STUN — Descubrimiento de IP pública

Implementado en `crates/gravital-talk-transport/src/stun.rs` según RFC 5389.

**Servidores:**
- `stun.l.google.com:19302` (primario)
- `stun1.l.google.com:19302` (fallback)

**Algoritmo:**
1. Crear socket UDP local en `local_port` (0 = puerto efímero del OS).
2. Enviar Binding Request: `[type=0x0001][len=0][magic=0x2112A442][tx_id=12 bytes aleatorios]`.
3. Recibir respuesta con timeout de 5 s.
4. Verificar `tx_id` coincide (descarta paquetes de otras fuentes).
5. Extraer atributo `XOR-MAPPED-ADDRESS` (type `0x0020`) y deshacer XOR con magic cookie.
6. Si no hay `XOR-MAPPED-ADDRESS`, usar `MAPPED-ADDRESS` como fallback.

**En Rust:**
```rust
use gravital_talk_transport::stun::discover_public_addr;

let public_addr = discover_public_addr(0).await?;
println!("IP pública: {}", public_addr);
```

**Limitaciones:** STUN funciona para NAT de cono completo (full cone) y NAT de cono restringido (restricted cone). Para NAT simétrico, STUN devuelve una IP diferente por cada peer de destino — en ese caso, el relay es necesario.

---

## Handshake abierto — `handshake_open()`

El handshake estándar requiere la IP del cliente. `handshake_open()` elimina ese requisito: acepta el primer `ClientHello` válido de cualquier dirección.

Ver [`session-model.md §9`](./session-model.md) para documentación completa y ejemplos de código.

---

## Flujo HOST (Android)

1. Usuario pulsa **"Crear llamada"** en PairingActivity.
2. `PairingViewModel.startHosting(relayUrl?)` ejecuta en paralelo:
   - `GravitalTalkJni.nativeCreate(...)` → handle + puerto local.
   - `ConnectivityManager.getLinkProperties()` → IP LAN.
   - `GravitalTalkJni.nativeDiscoverPublicAddr(localPort)` → IP pública STUN.
3. Construye URI `gravital-talk://pair?v=1&lan=...&pub=...&relay=...`.
4. `QRCodeWriter().encode(uri, BarcodeFormat.QR_CODE, 512, 512)` → Bitmap.
5. Muestra QR + código texto `GRVT-XXXX` en pantalla.
6. En coroutine IO: `nativeAcceptAny(handle)` bloquea esperando cliente.
7. Cuando retorna OK → emite `PairingScreen.Connected(handle)` → lanza `MainActivity`.
8. `MainActivity.attachExistingHandle(handle)` toma ownership del handle.

---

## Flujo CLIENT (Android)

**Vía QR:**
1. Usuario pulsa **"Unirse a llamada"** → pantalla Join con cámara.
2. CameraX captura frames → ML Kit `BarcodeScanning` detecta QR.
3. `PairingViewModel.joinFromQr(qrData)` parsea URI.
4. `nativeCreate(...)` → handle propio del cliente.
5. Intenta conexión progresiva:
   ```kotlin
   for (addr in listOf(lan, pub)) {
       val ok = withTimeoutOrNull(timeoutMs) { nativeConnect(handle, addr) }
       if (ok == 0) { connected = true; break }
   }
   if (!connected && relay != null) nativeConnect(handle, relay)
   ```
6. Si conectado → emite `Connected(handle)` → lanza `MainActivity`.

**Vía código manual:**
1. Usuario ingresa `host:port` del relay en campo de texto.
2. `joinFromRelay(host, port)` → igual que arriba pero solo intenta relay.

---

## Flujo de colgar

Cualquiera de los dos lados puede colgar:
- Pulsar `btnHangUp` en MainActivity → `PttViewModel.disconnect()`.
- `disconnect()` cierra la sesión nativa y navega a `PairingActivity`.
- El otro lado detecta la desconexión por timeout de heartbeat (10 s).

---

## Diagrama de secuencia

```
HOST (Android)                              CLIENT (Android)
─────────────────────────────────────────────────────────────
nativeCreate() ──────────────────────────────────────────────
                                              nativeCreate()
[QR en pantalla]
nativeAcceptAny() ─── bloquea ─────────────────────────────
                                            nativeConnect(lan) ──→ timeout 2s
                                            nativeConnect(pub) ──→ OK
                                            ↑ conectado ↑
     ← HANDSHAKE_INIT ←──────────────────────────────────────
     HANDSHAKE_ACCEPT ──────────────────────────────────────→
     ← HANDSHAKE_CONFIRM ←────────────────────────────────────
nativeAcceptAny() retorna OK
[PTT activo]                               [PTT activo]
─────────────────────────────────────────────────────────────
  AUDIO ──────────────────────────────────────────────────→
  ←────────────────────────────────────────────────── AUDIO
```

---

## Consideraciones de seguridad

- El handshake usa X25519 ECDH + ChaCha20-Poly1305 tanto en modo estándar como en modo abierto.
- La diferencia entre `handshake_open()` y `handshake_server()` es únicamente cuándo se conoce la IP del peer — no el nivel de cifrado.
- Un atacante que intercepte el QR y conecte antes que el cliente legítimo obtendría la sesión. Mitigación: el QR expira cuando el host cuelga o cuando alguien se conecta (la sesión es punto a punto).
- En entornos donde la seguridad del QR es crítica, usar relay con autenticación propia.

---

## Testing

```bash
# Verificar que STUN funciona (requiere internet)
cargo test -p gravital-talk-transport -- stun

# Compilar FFI con features Android
cargo check -p gravital-talk-ffi --features android

# Test de pairing en LAN: dos terminales con CLI
# Terminal A (host):
gs relay --bind 0.0.0.0 --udp-port 9000 &
gs ptt --relay 127.0.0.1:9000 --room test

# Terminal B (cliente):
gs ptt --relay <IP_A>:9000 --room test
```
