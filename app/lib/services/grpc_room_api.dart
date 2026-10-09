import 'package:flutter/foundation.dart' show kIsWeb;
import 'package:grpc/grpc.dart';

// Importación condicional: `grpc_web_io.dart` sólo compila en web y
// `grpc_web_stub.dart` en el resto. Así `dart:js_interop` nunca entra en el
// kernel de Android/iOS/escritorio.
import 'grpc_web_stub.dart'
    if (dart.library.js_interop) 'grpc_web_io.dart';

import '../generated/gravital/v1/server_control.pb.dart' as pb;
import '../generated/gravital/v1/server_control.pbgrpc.dart' as pbgrpc;
import 'room_api.dart';

/// Cliente gRPC del plano de control (`ServerControl`).
///
/// Requiere que el relay se compile con `--features grpc` (y `grpc-web` en
/// navegador) y escuche en `grpcPort` (por defecto 50051). En VM/desktop, canal
/// gRPC nativo con HTTP/2. Pensado para usarse a través de [FallbackRoomApi].
///
/// OJO con los imports: `grpc_web.dart` tira de `dart:js_interop`, que sólo
/// existe en web. Importarlo en un build de Android/iOS/escritorio falla al
/// COMPILAR, no en tiempo de ejecución, con un error apunta a
/// `dart:js_interop is not available on this platform`. Por eso el canal de web
/// se resuelve con un import condicional en este mismo fichero.
class GrpcRoomApi implements RoomControlApi {
  GrpcRoomApi({this.grpcPort = 50051, this.timeout = const Duration(seconds: 4)});

  final int grpcPort;
  final Duration timeout;

  Future<T> _withClient<T>(
    String host,
    Future<T> Function(pbgrpc.ServerControlClient client) op,
  ) async {
    final channel = await _openChannel(host);
    final client = pbgrpc.ServerControlClient(channel);
    try {
      return await op(client).timeout(timeout);
    } finally {
      await channel.shutdown();
    }
  }

  /// Abre el canal adecuado según la plataforma.
  ///
  /// El import de `grpc_web` va tras un `if (kIsWeb)` y Condicionado por el
  /// compilador: en plataformas nativas el compilador elimina la rama, así que
  /// `dart:js_interop` nunca entra en el grafo de dependencias.
  Future<ClientChannel> _openChannel(String host) async {
    if (kIsWeb) {
      // ignore: avoid_dynamic_calls
      final grpcWeb = await _loadGrpcWeb();
      return grpcWeb(host, grpcPort);
    }
    return ClientChannel(
      host,
      port: grpcPort,
      options: ChannelOptions(
        credentials: ChannelCredentials.insecure(),
        connectionTimeout: const Duration(seconds: 3),
      ),
    );
  }

  /// Carga `grpc_web` sólo en web.
  ///
  /// La resuelve el import condicional de arriba: en web devuelve el canal
  /// gRPC-Web real; en el resto, la versión stub que lanza un error claro.
  Future<ClientChannel Function(String, int)> _loadGrpcWeb() async {
    return createGrpcWebChannel;
  }

  @override
  Future<RoomInfo> createRoom({
    required String host,
    required int port,
    required int sessionId,
    String token = '',
  }) async {
    final resp = await _withClient(
      host,
      (client) => client.createRoom(
        pb.CreateRoomRequest(sessionId: sessionId, token: token),
      ),
    );
    final room = resp.room;
    return RoomInfo(code: room.code, sessionId: room.sessionId);
  }

  @override
  Future<RoomInfo> resolveRoom({
    required String host,
    required int port,
    required String code,
    String token = '',
  }) async {
    try {
      final resp = await _withClient(
        host,
        (client) => client.getRoom(pb.GetRoomRequest(code: code, token: token)),
      );
      final room = resp.room;
      return RoomInfo(
        code: room.code,
        sessionId: room.sessionId,
        peerCount: room.peerCount,
      );
    } on GrpcError catch (e) {
      if (e.code == StatusCode.notFound) {
        throw RoomApiException('sala no encontrada');
      }
      if (e.code == StatusCode.unauthenticated) {
        throw RoomApiException('la sala exige token (revisa el token de sala)');
      }
      rethrow;
    }
  }

  @override
  Future<bool> healthy({required String host, required int port}) async {
    try {
      await _withClient(
        host,
        (client) => client.getServerHealth(pb.ServerHealthRequest()),
      );
      return true;
    } catch (_) {
      return false;
    }
  }

  @override
  void dispose() {
    // Canales efímeros por llamada: nada que liberar.
  }
}
