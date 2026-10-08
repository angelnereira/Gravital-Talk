import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/services/room_api.dart';

class _GrpcFake implements RoomControlApi {
  _GrpcFake({this.failing = true});

  final bool failing;
  int calls = 0;

  @override
  Future<RoomInfo> createRoom({
    required String host,
    required int port,
    required int sessionId,
  }) async {
    calls++;
    if (failing) throw RoomApiException('grpc no disponible');
    return RoomInfo(code: 'GRPC-0001', sessionId: sessionId);
  }

  @override
  Future<RoomInfo> resolveRoom({
    required String host,
    required int port,
    required String code,
  }) async {
    calls++;
    if (failing) throw RoomApiException('grpc no disponible');
    return RoomInfo(code: code, sessionId: 42, peerCount: 1);
  }

  @override
  Future<bool> healthy({required String host, required int port}) async {
    calls++;
    return !failing;
  }

  @override
  void dispose() {}
}

class _RestFake implements RoomControlApi {
  int calls = 0;

  @override
  Future<RoomInfo> createRoom({
    required String host,
    required int port,
    required int sessionId,
  }) async {
    calls++;
    return RoomInfo(code: 'REST-1111', sessionId: sessionId);
  }

  @override
  Future<RoomInfo> resolveRoom({
    required String host,
    required int port,
    required String code,
  }) async {
    calls++;
    return RoomInfo(code: code, sessionId: 7, peerCount: 2);
  }

  @override
  Future<bool> healthy({required String host, required int port}) async {
    calls++;
    return true;
  }

  @override
  void dispose() {}
}

void main() {
  test('fallback: usa REST cuando gRPC falla y no reintenta', () async {
    final grpc = _GrpcFake(failing: true);
    final rest = _RestFake();
    final api = FallbackRoomApi(grpc: grpc, rest: rest);

    final r1 = await api.createRoom(host: 'relay', port: 9100, sessionId: 9);
    expect(r1.code, 'REST-1111');
    expect(r1.sessionId, 9);
    expect(grpc.calls, 1, reason: 'gRPC se intenta una vez');
    expect(rest.calls, 1);
    expect(api.usingGrpc, isFalse);

    // La segunda operación ya no intenta gRPC (sin timeouts repetidos).
    final r2 = await api.createRoom(host: 'relay', port: 9100, sessionId: 9);
    expect(r2.code, 'REST-1111');
    expect(grpc.calls, 1);
    expect(rest.calls, 2);
  });

  test('fallback: usa gRPC cuando responde', () async {
    final grpc = _GrpcFake(failing: false);
    final rest = _RestFake();
    final api = FallbackRoomApi(grpc: grpc, rest: rest);

    final room = await api.resolveRoom(host: 'relay', port: 9100, code: 'ABCD-2345');
    expect(room.code, 'ABCD-2345');
    expect(room.sessionId, 42);
    expect(api.usingGrpc, isTrue);
    expect(rest.calls, 0, reason: 'REST no debe usarse si gRPC responde');
  });

  test('healthy cae a REST si gRPC falla', () async {
    final grpc = _GrpcFake(failing: true);
    final rest = _RestFake();
    final api = FallbackRoomApi(grpc: grpc, rest: rest);

    expect(await api.healthy(host: 'relay', port: 9100), isTrue);
    expect(rest.calls, 1);
  });
}
