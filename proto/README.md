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

### Rust (automático)

`crates/gravital-talk-relay/build.rs` compila los `.proto` con `tonic-build`
+ `protoc-bin-vendored` (no requiere protoc del sistema) al activar la
feature `grpc`. No hay pasos manuales.

### Dart / Flutter (cliente de la app)

```bash
# 1. Plugin de protoc para Dart (una vez)
dart pub global activate protoc_plugin 25.1.0

# 2. protoc: sirve el binario vendorizado del repo Rust
PROTOC=$(ls -d ~/.cargo/registry/src/*/protoc-bin-vendored-linux-x86_64-*/bin/protoc)

# 3. Generar en app/lib/generated (no editar a mano)
export PATH="$HOME/.pub-cache/bin:$PATH"
$PROTOC -I proto --dart_out=grpc:app/lib/generated \
  proto/gravital/v1/server_control.proto \
  proto/gravital/v1/pairing.proto
```

El cliente resultante (`ServerControlClient`) se usa en
`app/lib/services/grpc_room_api.dart`, envuelto por `FallbackRoomApi`
(gRPC → REST). Dependencias del runtime: `grpc ^5`, `protobuf ^6`, `fixnum`.

### Otros lenguajes

```bash
# Go
protoc -I proto --go_out=. --go-grpc_out=. proto/gravital/v1/*.proto

# Validación del contrato (descriptor set)
protoc -I proto --descriptor_set_out=/tmp/descriptor.pb proto/gravital/v1/*.proto
```