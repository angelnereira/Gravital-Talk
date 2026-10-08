# Contratos (protobuf) — plano de control Gravital Talk

Definición formal de la superficie de **control** del servidor (relay) y de la
**señalización P2P**, como paso previo a evaluar y adoptar gRPC.

```
proto/
└── gravital/
    └── v1/
        ├── server_control.proto   # Rooms, health, info, eventos (stream)
        └── pairing.proto          # Ofertas de emparejamiento P2P
```

## Relación con la implementación actual

| Contrato        | Implementado hoy (REST en :9100) | Futuro gRPC |
|-----------------|----------------------------------|-------------|
| `GetServerInfo` | `/healthz`, `/metrics`            | `ServerControl` |
| `CreateRoom`    | `POST /api/rooms`                 | `ServerControl` |
| `GetRoom`       | `GET  /api/rooms/{code}`          | `ServerControl` |
| `ListRooms`     | `GET  /api/rooms`                 | `ServerControl` |
| `DeleteRoom`    | `DELETE /api/rooms/{code}`        | `ServerControl` |
| `WatchRoom`     | —                                 | stream (nuevo) |
| Pairing         | QR/URI plano + STUN               | `PairingService` |

## Decisiones

1. **El plano de audio NO es gRPC** (ver `docs/grpc-evaluation.md`). Los
   datagramas de audio siguen viajando por UDP/WebSocket binario con el
   framing `gravital-talk-core`.
2. Los protos son la fuente de verdad del contrato **servidor ↔ cliente**;
   los clientes pueden generarse con `protoc`/`buf`/`tonic-build` en cualquier
   lenguaje (Rust, Dart, Go, TS).
3. Retro-compatibilidad: los endpoints REST actuales se mantienen durante la
   transición; gRPC se sirve junto a ellos en el mismo puerto (gateway) o en
   uno nuevo (50051).

## Generación de código

```bash
# Rust (candidato: tonic)
protoc -I proto --plugin=protoc-gen-tonic=... proto/gravital/v1/*.proto

# Dart (candidato: grpc-dart, plugin protoc_plugin)
protoc -I proto --dart_out=grpc:app/lib/generated proto/gravital/v1/*.proto

# Go
protoc -I proto --go_out=. --go-grpc_out=. proto/gravital/v1/*.proto
```

## Validación del contrato

```bash
docker compose --profile test run --rm tester bash -c \
  "apt-get update -qq && apt-get install -y -qq protobuf-compiler && \
   protoc -I proto --descriptor_set_out=/tmp/descriptor.pb proto/gravital/v1/*.proto && \
   echo CONTRATO_VALIDO"
```