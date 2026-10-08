import 'package:grpc/grpc.dart';

import '../generated/gravital/v1/server_control.pb.dart' as pb;
import '../generated/gravital/v1/server_control.pbgrpc.dart' as pbgrpc;
import 'room_api.dart';

/// Cliente gRPC del plano de control (`ServerControl`).
///
/// Requiere que el relay se compile con `--features grpc` y escuche en
/// `grpc_port` (por defecto 50051). Pensado para usarse a través de
/// [FallbackRoomApi]: si el canal no responde, la app cae a REST.
class GrpcRoomApi implements RoomControlApi {
  GrpcRoomApi({this.grpcPort = 50051, this.timeout = const Duration(seconds: 4)});

  final int grpcPort;
  final Duration timeout;

  Future<T> _withClient<T>(
    String host,
    Future<T> Function(pbgrpc.ServerControlClient client) op,
  ) async {
    final channel = ClientChannel(
      host,
      port: grpcPort,
      options: ChannelOptions(
        credentials: ChannelCredentials.insecure(),
        connectionTimeout: const Duration(seconds: 3),
      ),
    );
    final client = pbgrpc.ServerControlClient(channel);
    try {
      return await op(client).timeout(timeout);
    } finally {
      await channel.shutdown();
    }
  }

  @override
  Future<RoomInfo> createRoom({
    required String host,
    required int port,
    required int sessionId,
  }) async {
    final resp = await _withClient(
      host,
      (client) => client.createRoom(
        pb.CreateRoomRequest(sessionId: sessionId),
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
  }) async {
    try {
      final resp = await _withClient(
        host,
        (client) => client.getRoom(pb.GetRoomRequest(code: code)),
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