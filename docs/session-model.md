# Modelo de sesión

## 1. Estados

Una sesión Gravital Sound atraviesa los siguientes estados. La máquina es implementada como un tipo con parámetros marcadores (phantom types), de modo que transiciones inválidas son error de compilación.

```
          ┌──────────┐
          │   Idle   │
          └─────┬────┘
                │ connect() | accept()
                ▼
          ┌──────────────┐
          │ Handshaking  │
          └──────┬───────┘
                 │ handshake 3-way ok
                 ▼
          ┌──────────┐  pause()      ┌──────────┐
          │  Active  │ ────────────▶ │  Paused  │
          │          │ ◀──────────── │          │
          └────┬─────┘   resume()    └────┬─────┘
               │                          │
               │ close() | timeout        │ close()
               ▼                          ▼
          ┌──────────┐              ┌──────────┐
          │ Closing  │ ───────────▶ │  Closed  │
          └──────────┘              └──────────┘
```

## 2. Transiciones permitidas

| Desde            | Mensaje/Evento             | Hacia           |
|------------------|----------------------------|-----------------|
| `Idle`           | `connect()`                | `Handshaking`   |
| `Idle`           | `accept()`                 | `Handshaking`   |
| `Handshaking`    | recibe `HANDSHAKE_CONFIRM` | `Active`        |
| `Handshaking`    | recibe `HANDSHAKE_ACCEPT`  | `Active` (cliente) |
| `Handshaking`    | timeout 10 s               | `Closed`        |
| `Active`         | `pause()` o recibe `CONTROL_PAUSE` | `Paused` |
| `Active`         | `close()` o recibe `CLOSE` | `Closing`       |
| `Active`         | heartbeat ausente > 10 s   | `Closing`       |
| `Paused`         | `resume()` o recibe `CONTROL_RESUME` | `Active` |
| `Paused`         | `close()` o recibe `CLOSE` | `Closing`       |
| `Closing`        | recibe `CLOSE` | `Closed`        |
| `Closing`        | timeout 500 ms             | `Closed`        |

Cualquier otra combinación es inválida y se rechaza. En el código Rust, intentar `session.send_audio()` desde un `Session<Handshaking>` es un error de compilación porque `send_audio` sólo está implementado para `Session<Active>`.

## 3. Timers

| Nombre                  | Valor inicial | Descripción                                   |
|-------------------------|---------------|-----------------------------------------------|
| `HANDSHAKE_TIMEOUT`     | 10 s          | Tiempo total del handshake antes de abort.    |
| `HANDSHAKE_RETRY_BASE`  | 200 ms        | Backoff inicial del cliente (×2, max 5 tries).|
| `HEARTBEAT_INTERVAL`    | 1 s           | Cadencia de `HEARTBEAT` si no hay otro tráfico. |
| `HEARTBEAT_TIMEOUT`     | 10 s          | Sin tráfico durante este tiempo → `Closing`.  |
| `CLOSE_GRACE`           | 500 ms        | Espera respuesta a `CLOSE` antes de forzar `Closed`. |
| `METRICS_PUSH_INTERVAL` | 5 s           | Opcional. Envía `CONTROL_METRICS` al peer.    |

Todos los timers son configurables vía `Config`; los defaults anteriores se eligen para uso en internet público con RTT típico de ≤ 200 ms.

## 4. Identidad de sesión

- El `session_id` es un `u32` asignado por el servidor durante `HANDSHAKE_ACCEPT`.
- El servidor debe garantizar que `session_id` no colisiona con sesiones activas. Una implementación viable usa un `AtomicU32` incremental + validación en tabla.
- Los mensajes con `session_id` no reconocido se descartan silenciosamente, salvo `HANDSHAKE_INIT` (donde `session_id == 0`).

## 5. Reordenamiento

Los paquetes con `sequence` más viejo que el último entregado se descartan si no caben en el jitter buffer (ver §6). Los paquetes fuera de orden pero dentro de la ventana se ordenan antes de entregar al codec.

## 6. Jitter buffer

El receptor mantiene un ring buffer de tamaño configurable (`jitter_buffer_ms`, default 40 ms). Operaciones:

- **Insert**: coloca un paquete en la posición derivada de su `sequence` relativa al base.
- **Pop**: entrega el siguiente paquete esperado, bloqueando hasta que esté disponible o hasta el timeout de playout.
- **Advance**: avanza la base cuando el paquete esperado no ha llegado en el tiempo máximo, contándolo como `lost`.

El buffer usa estructuras lock-free (ver [ADR-007 futuro]) para soportar productor en el hilo de red y consumidor en el hilo de audio.

## 7. Identificación del rol

Durante el handshake se distingue el **iniciador** (cliente) del **receptor** (servidor). Post-handshake, los roles son simétricos — ambos pueden enviar audio, pausar, enviar métricas. El único privilegio persistente del servidor es asignar `session_id`.

## 8. Cambio de codec mid-session

No soportado en 0.1. Cambiar codec requiere cerrar la sesión y reabrir una nueva.
