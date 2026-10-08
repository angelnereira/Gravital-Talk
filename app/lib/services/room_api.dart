import 'dart:convert';

import 'package:http/http.dart' as http;

/// Interfaz del plano de control del relay (REST o gRPC).
///
/// La app usa REST por defecto (funciona en web y sin feature `grpc` en el
/// relay) y gRPC cuando está disponible, con fallback automático.
abstract interface class RoomControlApi {
  Future<RoomInfo> createRoom({
    required String host,
    required int port,
    required int sessionId,
  });

  Future<RoomInfo> resolveRoom({
    required String host,
    required int port,
    required String code,
  });

  Future<bool> healthy({required String host, required int port});

  void dispose();
}

/// Información de una sala en el relay.
class RoomInfo {
  const RoomInfo({required this.code, required this.sessionId, this.peerCount});

  final String code;
  final int sessionId;
  final int? peerCount;
}

/// Cliente de la API HTTP del relay (`/api/rooms`, `/healthz`).
///
/// El puerto de observabilidad es el mismo que expone `/metrics`.
class RoomApi implements RoomControlApi {
  RoomApi({http.Client? client}) : _client = client ?? http.Client();

  final http.Client _client;
  static const _timeout = Duration(seconds: 5);

  Uri _uri(String host, int port, String path) =>
      Uri.parse('http://$host:$port$path');

  /// `POST /api/rooms` — registra una sala con el `session_id` del host.
  @override
  Future<RoomInfo> createRoom({
    required String host,
    required int port,
    required int sessionId,
  }) async {
    final resp = await _client
        .post(
          _uri(host, port, '/api/rooms'),
          headers: {'content-type': 'application/json'},
          body: jsonEncode({'session_id': sessionId}),
        )
        .timeout(_timeout);
    if (resp.statusCode != 201) {
      throw RoomApiException(
          'crear sala falló (${resp.statusCode}): ${resp.body}');
    }
    final json = jsonDecode(resp.body) as Map<String, dynamic>;
    return RoomInfo(
      code: json['code'] as String,
      sessionId: (json['session_id'] as num).toInt(),
    );
  }

  /// `GET /api/rooms/{code}` — resuelve un código a `session_id`.
  @override
  Future<RoomInfo> resolveRoom({
    required String host,
    required int port,
    required String code,
  }) async {
    final resp = await _client
        .get(_uri(host, port, '/api/rooms/$code'))
        .timeout(_timeout);
    if (resp.statusCode != 200) {
      throw RoomApiException(resp.statusCode == 404
          ? 'sala no encontrada'
          : 'resolver sala falló (${resp.statusCode})');
    }
    final json = jsonDecode(resp.body) as Map<String, dynamic>;
    return RoomInfo(
      code: code,
      sessionId: (json['session_id'] as num).toInt(),
      peerCount: (json['peer_count'] as num?)?.toInt(),
    );
  }

  /// `GET /healthz` — verifica que el relay responde.
  @override
  Future<bool> healthy({required String host, required int port}) async {
    try {
      final resp = await _client
          .get(_uri(host, port, '/healthz'))
          .timeout(_timeout);
      return resp.statusCode == 200;
    } catch (_) {
      return false;
    }
  }

  @override
  void dispose() => _client.close();
}

/// Usa gRPC y cae a REST si el relay no expone el plano de control.
///
/// Una vez que gRPC falla, no se reintenta (evita timeouts repetidos en cada
/// operación); el resto de la sesión usa REST.
class FallbackRoomApi implements RoomControlApi {
  FallbackRoomApi({required this.grpc, required this.rest});

  final RoomControlApi grpc;
  final RoomControlApi rest;
  bool _grpcAvailable = true;

  Future<T> _call<T>(Future<T> Function(RoomControlApi api) op) async {
    if (_grpcAvailable) {
      try {
        return await op(grpc);
      } on Object {
        _grpcAvailable = false;
      }
    }
    return op(rest);
  }

  /// `true` si la última operación pudo usar gRPC.
  bool get usingGrpc => _grpcAvailable;

  @override
  Future<RoomInfo> createRoom({
    required String host,
    required int port,
    required int sessionId,
  }) =>
      _call((api) => api.createRoom(host: host, port: port, sessionId: sessionId));

  @override
  Future<RoomInfo> resolveRoom({
    required String host,
    required int port,
    required String code,
  }) =>
      _call((api) => api.resolveRoom(host: host, port: port, code: code));

  @override
  Future<bool> healthy({required String host, required int port}) async {
    // Si gRPC responde sano, es la fuente; si no (relay sin feature `grpc`
    // o caído), se consulta REST antes de dar un veredicto.
    if (_grpcAvailable) {
      try {
        if (await grpc.healthy(host: host, port: port)) return true;
      } on Object {
        _grpcAvailable = false;
      }
    }
    return rest.healthy(host: host, port: port);
  }

  @override
  void dispose() {
    grpc.dispose();
    rest.dispose();
  }
}

class RoomApiException implements Exception {
  RoomApiException(this.message);

  final String message;

  @override
  String toString() => message;
}
