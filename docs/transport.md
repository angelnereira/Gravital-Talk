# Diseño de la capa de transporte

## 1. Requisitos

El protocolo necesita una capa que:

1. Entregue datagramas discretos sin fragmentación silenciosa.
2. Sea de baja latencia (no reintentos ciegos, no handshake TCP por paquete).
3. Esté disponible en todas las plataformas objetivo, incluido el navegador.
4. Permita saturar un enlace de al menos 1 Gbps en hardware commodity.

## 2. UDP como transporte primario

Para todos los nativos (Linux, macOS, Windows, Android, iOS, embebido), el transporte es **UDP**:

- Cada `Packet` se envía como un `sendto()`/`sendmsg()`.
- Se usa `socket2` para exponer opciones avanzadas:
  - `SO_REUSEADDR` y `SO_REUSEPORT` para bind múltiples.
  - `SO_SNDBUF` / `SO_RCVBUF` a 4 MB por default (sintonizable).
  - `IP_TOS = 0xB8` (DSCP EF, Expedited Forwarding) para priorizar tráfico real-time.
  - `SO_BUSY_POLL` opcional (Linux, reduce wakeup latency).
  - `IP_PKTINFO`/`IPV6_RECVPKTINFO` para saber qué interfaz recibió cada paquete (servidor multi-homed).
- En Linux con feature `io-uring`, `UdpTransport` puede usar `tokio-uring` en lugar del runtime estándar. Elimina una syscall por paquete y permite batch submit con `SQE`.

## 3. WebSocket para navegador

Los navegadores no exponen UDP directamente (excepción: WebTransport, aún no universal). Para corrar en el navegador:

- El transporte por default del SDK web es **WebSocket binario**.
- Cada `Packet` se envía como un frame binario (`opcode = 0x02`).
- Un **relay** corre en un servidor accesible desde internet; acepta conexiones WebSocket y UDP por el mismo `session_id` y hace forwarding entre ellas.
- WebSocket opera sobre TCP, lo que añade una latencia variable (retransmisiones). Se mitiga:
  - Deshabilitando `Nagle` (`TCP_NODELAY`) en el relay.
  - Configurando buffers mayores (`SO_SNDBUF`).
  - El jitter buffer del cliente se ajusta automáticamente hacia arriba cuando detecta transporte TCP.

## 4. Plan WebTransport (post-0.1)

WebTransport sobre QUIC ofrece semántica datagram-like en el navegador (`sendDatagram()`). Lo adoptaremos cuando:

1. Al menos Chrome + Firefox + Safari estables lo soporten sin flags.
2. Exista un servidor QUIC maduro en Rust (`quinn` es el candidato obvio).
3. Los tests de latencia muestren mejora > 20% sobre WebSocket.

Mientras tanto, `gravital-sound-transport` define un trait `Transport` que abstrae el medio, de modo que añadir WebTransport sea un crate nuevo, no un cambio en el core.

## 5. Trait `Transport`

```rust
#[async_trait]
pub trait Transport: Send + Sync {
    async fn send(&self, packet: &[u8]) -> Result<()>;
    async fn recv(&self, buf: &mut [u8]) -> Result<usize>;
    async fn close(&self) -> Result<()>;

    fn local_addr(&self) -> Option<SocketAddr> { None }
    fn peer_addr(&self) -> Option<SocketAddr> { None }

    /// Latency class hint: "ultra-low" (< 1 ms, ej. UDP LAN),
    /// "low" (< 10 ms), "medium" (< 50 ms, ej. WebSocket normal).
    /// El jitter buffer ajusta su depth según este valor.
    fn latency_class(&self) -> LatencyClass { LatencyClass::Low }
}
```

Implementaciones:

- `UdpTransport`: `tokio::net::UdpSocket` o `tokio-uring::UdpSocket`.
- `WebSocketTransport`: `tokio-tungstenite`.
- `NullTransport`: para tests.
- `MockTransport`: inyecta pérdida/jitter/reorder para testing determinista.

## 6. Tuning de sockets

Tabla de parámetros por plataforma aplicados por default:

| Parámetro           | Linux           | macOS           | Windows         |
|---------------------|-----------------|-----------------|-----------------|
| `SO_SNDBUF`         | 4 MB            | 4 MB            | 4 MB            |
| `SO_RCVBUF`         | 4 MB            | 4 MB            | 4 MB            |
| `SO_REUSEADDR`      | on              | on              | on              |
| `SO_REUSEPORT`      | on              | on              | n/a (`SO_EXCLUSIVEADDRUSE` off) |
| `IP_TOS` DSCP EF    | on              | on              | on              |
| `SO_BUSY_POLL`      | opt-in feature  | n/a             | n/a             |
| `IP_PKTINFO`        | on              | on              | on              |
| `SO_TIMESTAMPNS`    | opt-in feature  | n/a             | n/a             |

## 7. Kernel bypass (exploratorio)

Para servidores que manejan > 10k sesiones concurrentes, el overhead de syscalls es prohibitivo. El trait `Transport` está diseñado para permitir backends sin cambiar el core:

- **`AF_XDP`** (Linux): mapeos de memoria compartida kernel ↔ user-space. Planificado como crate separado `gravital-sound-transport-afxdp`.
- **DPDK**: bypass total. Requiere bind de NIC. Planificado para despliegues de alto volumen.
- **eBPF XDP** para filtros de ingress (antes de llegar al user-space). Usado para dropear rápido paquetes malformados.

Estos backends no son parte del 0.1.

## 8. NAT traversal

El 0.1 asume conectividad directa o un relay explícito. NAT traversal se añade en una fase posterior:

- **STUN** para descubrir IP pública + binding NAT.
- **ICE** para negociar candidatos.
- **TURN** como fallback cuando STUN falla.

Alternativamente, un relay Gravital puede servir como "TURN pobre" a cambio de + 1 hop de latencia.

## 9. Multiplexado

Un solo `Transport` puede servir múltiples sesiones (servidor acepta múltiples peers). La demultiplexación usa:

- `session_id` del header (para sesiones ya establecidas).
- `peer_addr` + `nonce` (durante handshake pre-ACCEPT, donde `session_id = 0`).

El relay mantiene una tabla hash `session_id → peer_addrs[]`.

## 10. Congestion control

El 0.1 no implementa congestion control activo. La aplicación controla el bitrate via configuración del codec. Futuro:

- **Receiver-side estimación** estilo Google WebRTC (GCC) adaptada.
- Señalización opcional de `CONTROL_METRICS` con `bitrate_suggestion`.
