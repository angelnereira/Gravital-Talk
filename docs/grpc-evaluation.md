# Evaluación: gRPC para el plano de control de Gravital Talk

> Estado: **análisis y contratos definidos** (`proto/gravital/v1/*.proto`).
> Decisión de implementación pendiente; este documento da el marco.

## 1. Qué problema resolvemos

El relay expone una API REST ad-hoc (HTTP/1.1 raw en `:9100`) para salas y
salud. Es suficiente pero:

- No hay tipado formal: cada cliente reimplementa el parseo (el CLI usa un
  parser JSON a mano; la app Flutter usa `http` + `jsonDecode`).
- No hay streams (un dashboard no puede "escuchar" eventos de sala).
- No hay autenticación/rate limiting estructurado.
- Cada SDK (Rust, Dart, futuro Go/TS) reescribe el contrato a mano.

gRPC aporta: contrato tipado (`proto3`), streaming bidireccional, código
generado para todos los lenguajes objetivo, HTTP/2 multiplexado, y ecosistema
maduro (tonic en Rust, grpc-dart, connect-go, buf).

## 2. Dónde SÍ y dónde NO

| Plano | Tecnología | Motivo |
|---|---|---|
| **Control** (rooms, salud, info, eventos, admin) | **gRPC** (tonic) | RPCs de baja frecuencia, tipados, streaming útil |
| **Audio** (frames PCM/Opus cifrados) | **UDP** nativo + **WebSocket** binario en navegador | Latencia: gRPC/HTTP2 introduce cabeceras, control de flujo y retransmisión TCP que rompen el presupuesto de p50 < 5 ms y p99 < 25 ms |
| **Handshake** criptográfico 4-way | UDP wire protocol (sin cambios) | Ya está; el relay no participa en NEGOCIAR claves, solo enruta |
| **Emparejamiento P2P** (QR/URI) | gRPC opcional como *anclaje de señalización* | Candidato; hoy es STUN + URI descriptiva |

**Conclusión**: gRPC convive con el plano de audio, no lo sustituye. Es un
**plano de control** con su propio puerto (p. ej. `50051`), igual que hoy
conviven UDP `9000` y HTTP `9100`.

## 3. Cómo encaja en la arquitectura

```
App Flutter / CLI / SDKs
   │  gRPC (control)              UDP/WS (media)
   ▼                              ▼
┌─────────────────────────────────────────────────────┐
│ relay (gs-relay)                                    │
│  ├─ gs SessionServer (futuro: tonic 50051)          │
│  ├─ gs ApiServer REST   (hoy: hyper 9100)  [transición]│
│  ├─ UDP 9000  ─ router (session_id → peers)         │
│  └─ WebSocket 9090 (bridge nodo→browser)            │
└─────────────────────────────────────────────────────┘
```

Los contratos (`proto/gravital/v1/*.proto`) son aceptados por el repo:
`CreateRoom`/`GetRoom`/`ListRooms`/`DeleteRoom` mapean 1:1 con los endpoints
REST actuales; `WatchRoom` y `GetServerHealth` son nuevos y explotan lo que
REST no daba (streaming, métricas agregadas).

## 4. Alternativas evaluadas

| Opción | Ventajas | Desventajas | Veredicto |
|---|---|---|---|
| **tonic (gRPC, Rust)** | Contrato estricto, streaming, generación `tonic-build` en CI, ecosistema (tower, auth middleware) | HTTP/2 + TLS más pesado; TO-DO transporte para browser sin proxy | **Recomendado** para el servidor |
| **connect-go / connect-es** | Interop gRPC + RPC sobre HTTP/1.1/HTTP2, browsers sin proxy | Enfoque Go/TS; en Rust hay que usar el transport del crate `connect` (menos maduro) | Alternativa si el dashboard va en TS |
| **REST refinado (OpenAPI)** | Simple | Sin streaming nativo, sin tipado en cliente, duplica el contrato | No recomendado como destino final |
| **JSON-RPC sobre WS** | Simple | Sin generación de código, sin estándar de streaming | No recomendado |

## 5. Mitigaciones de riesgo

1. **Latencia del control**: el tráfico de control es ~1 RPC por operación
   (crear/unirse a sala, health cada 30 s). HTTP/2 sobre localhost/cloud
   añade < 1 ms vs REST. Impacto en el plano de audio: nulo (canales separados).
2. **Tamaño binario**: `tonic`/`hyper`/`h2` suben el binario del relay
   (~10–15 MB). Aceptable; se puede segregar la feature `grpc` (default off)
   hasta estabilizar.
3. **Browser**: Flutter Web no habla gRPC directo; se resuelve con
   `grpc-web` (Envoy/grpcwebproxy) o con `connect-es` en la app cuando se
   adopte. La app hoy usa REST; el contrato proto permite generar ambos.
4. **Auth/rate limiting**: gRPC + tower da interceptores uniformes
   (auth, límites por token/peer) — más fácil de auditar que el REST actual.

## 6. Plan de adopción

| Fase | Trabajo | Entregable |
|---|---|---|
| 0 (hecho) | Contratos `proto/gravital/v1` | `server_control.proto`, `pairing.proto` |
| 1 | `cargo add tonic` en `gravital-talk-relay`, `tonic-build` en build.rs, feature `grpc` | `ServerControl` servido en `:50051` junto al REST |
| 2 | Generar clientes Dart (grpc-dart) y usarlos en la app bajo feature flag | App con control gRPC, fallback REST |
| 3 | `WatchRoom` streaming + eventos (`PEER_JOINED`, `FLOOR_GRANTED`) | Dashboard de salas en Grafana/realtime |
| 4 | gRPC-web proxy para Flutter Web | Web usa el mismo contrato |
| 5 | Auditar auth + rate limits sobre tonic (tower) | Cierre de la brecha de seguridad 0.3 |

## 7. Conclusión

**Sí, adoptar gRPC para el plano de control**, empezando por las operaciones
de sala y salud ya implementadas, sin tocar el plano de audio. Los contratos
quedan definidos ya; la primera implementación (tonic, feature `grpc`) se
estima en una iteración corta y no rompe la API REST actual (conviven durante
la transición). El emparejamiento P2P puede beneficiarse de `PairingService`
como anclaje de señalización opcional.

Relación con la hoja de ruta: ver `docs/roadmap.md` (hito 0.3.1).