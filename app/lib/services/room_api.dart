import 'dart:convert';

import 'package:http/http.dart' as http;

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
class RoomApi {
  RoomApi({http.Client? client}) : _client = client ?? http.Client();

  final http.Client _client;
  static const _timeout = Duration(seconds: 5);

  Uri _uri(String host, int port, String path) =>
      Uri.parse('http://$host:$port$path');

  /// `POST /api/rooms` — registra una sala con el `session_id` del host.
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

  void dispose() => _client.close();
}

class RoomApiException implements Exception {
  RoomApiException(this.message);

  final String message;

  @override
  String toString() => message;
}
