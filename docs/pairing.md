# Emparejamiento P2P — Guía técnica

## Problema

El flujo original de Gravital Talk requería que ambos usuarios conocieran la IP del relay y el identificador de sesión antes de conectar. Esto obliga a intercambiar información fuera de banda (email, chat, etc.) y no funciona sin un relay intermedio.

El modo de emparejamiento por QR/código resuelve esto: el host genera un código de un solo uso que el cliente escanea, y la conexión se establece automáticamente — sin relay obligatorio.

---

## Estrategia de conectividad progresiva

```
QR contiene: { lan_addr, public_addr, relay_url? }

Cliente intenta en orden:
  1. LAN directa   → timeout 2 s   (~100% en misma WiFi)
  2. Internet P2P  → timeout 5 s   (~85% con STUN, NAT cono)
  3. Relay         → timeout 10 s  (100% si URL presente en QR)
```

Para NAT simétrico (corporate, CGNAT estricto) se requiere relay. El relay es opcional y se incluye en el QR solo si el host lo configura.

---

## URI canónica del QR

```
gravital-talk://pair?v=1&lan=192.168.1.5:49127&pub=203.0.113.45:49127&relay=relay.example.com:9000
```

| Parámetro | Descripción | Obligatorio |
|-----------|-------------|-------------|
| `v` | Versión del formato (actualmente `1`) | Sí |
| `lan` | IP local + puerto UDP de la sesión | Sí |
| `pub` | IP pública + puerto (descubierta por STUN) | No |
| `relay` | Host:port del relay (fallback) | No |

El QR se puede escanear con la cámara del teléfono o leer como texto. El código corto `GRVT-XXXX` es el handle en hex, útil para conexiones vía relay cuando se tiene el relay configurado.

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
