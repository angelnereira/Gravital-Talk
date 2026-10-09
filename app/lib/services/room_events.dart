import 'package:flutter/foundation.dart';
import 'package:grpc/grpc.dart';

import '../generated/gravital/v1/server_control.pb.dart' as pb;
import '../generated/gravital/v1/server_control.pbgrpc.dart' as pbgrpc;

/// Estado observable de una sala, alimentado por el stream `WatchRoom`.
///
/// El plano de control del relay ya emite `PeerJoined`, `PeerLeft`,
/// `FloorGranted` y `FloorReleased` (`proto/gravital/v1/server_control.proto`),
/// pero la app no consumía ninguno: la pantalla de sesión mostraba métricas y
/// nada sobre quién hay en la sala ni quién tiene el turno. Este servicio cierra
/// ese hueco.
///
/// Extiende [ChangeNotifier] para que las pantallas puedan usar
/// `context.watch<RoomEvents>()` y reconstruirse con cada evento.
class RoomEvents extends ChangeNotifier {
  RoomEvents({
    this.grpcPort = 50051,
    this.timeout = const Duration(seconds: 4),
  });

  final int grpcPort;
  final Duration timeout;

  pbgrpc.ServerControlClient? _client;
  ClientChannel? _channel;
  ResponseStream<pb.RoomEvent>? _stream;

  /// Código de sala observada, o `null` si no hay ninguna.
  String? _roomCode;

  /// SSRCs de los participantes vistos. El orden de llegada importa: el primero
  /// es el host de la sala, que es quien arbitra el floor.
  final Set<int> _peers = <int>{};

  /// SSRC que tiene el turno concedido, o `null` si el floor está libre.
  int? _floorHolder;

  /// `true` si el stream está conectado.
  bool _connected = false;

  /// Último error del stream, si lo hay.
  String? _error;

  /// Número de participantes conocidos (incluye al observador).
  int get peerCount => _peers.length + 1;

  /// Participantes sin el observador local.
  List<int> get peers => List.unmodifiable(_peers);

  /// `true` si el stream está conectado.
  bool get isConnected => _connected;

  /// Último error, o `null` si no hay.
  String? get error => _error;

  /// SSRC del que tiene el turno, o `null` si está libre.
  int? get floorHolder => _floorHolder;

  /// Código de sala observada.
  String? get roomCode => _roomCode;

  bool _disposed = false;

  /// Fija el estado directamente (sólo para tests de UI).
  ///
  /// El stream real necesita un relay con gRPC; los tests de widget no lo
  /// tienen, así que simulan el estado que emitiría.
  void debugSetState({
    List<int> peers = const [],
    int? floorHolder,
    required bool connected,
  }) {
    _peers
      ..clear()
      ..addAll(peers);
    _floorHolder = floorHolder;
    _connected = connected;
    notifyListeners();
  }

  /// Empieza a observar la sala `code`.
  ///
  /// Si hay una suscripción previa, se cancela antes de crear la nueva para no
  /// dejar streams colgados al cambiar de sala.
  Future<void> watch(String code, {required String host}) async {
    if (_disposed) return;
    await stop();

    _roomCode = code;
    _peers.clear();
    _floorHolder = null;
    _error = null;
    _connected = false;
    notifyListeners();

    try {
      _channel = ClientChannel(
        host,
        port: grpcPort,
        options: ChannelOptions(
          credentials: ChannelCredentials.insecure(),
          connectionTimeout: const Duration(seconds: 3),
        ),
      );
      _client = pbgrpc.ServerControlClient(_channel!);
      _stream = _client!.watchRoom(
        pb.WatchRoomRequest()..roomCode = code,
      );

      _connected = true;
      notifyListeners();

      await for (final event in _stream!) {
        if (_disposed) break;
        _apply(event);
      }
      // El stream terminó: sin esto la UI se queda mostrando "conectado".
      _connected = false;
      notifyListeners();
    } catch (e) {
      _error = 'stream de sala caído: $e';
      _connected = false;
      notifyListeners();
    }
  }

  void _apply(pb.RoomEvent event) {
    switch (event.type) {
      case pb.RoomEvent_EventType.PEER_JOINED:
        if (event.peerSsrc != 0) _peers.add(event.peerSsrc);
        break;
      case pb.RoomEvent_EventType.PEER_LEFT:
        _peers.remove(event.peerSsrc);
        // Si el que se fue tenía el turno, el floor queda libre.
        if (_floorHolder == event.peerSsrc) _floorHolder = null;
        break;
      case pb.RoomEvent_EventType.FLOOR_GRANTED:
        _floorHolder = event.peerSsrc;
        break;
      case pb.RoomEvent_EventType.FLOOR_RELEASED:
        if (_floorHolder == event.peerSsrc) _floorHolder = null;
        break;
      case pb.RoomEvent_EventType.ROOM_CLOSED:
        _peers.clear();
        _floorHolder = null;
        _connected = false;
        break;
      default:
        break;
    }
    notifyListeners();
  }

  /// Detiene la observación y libera el canal.
  Future<void> stop() async {
    _connected = false;
    _roomCode = null;
    _peers.clear();
    _floorHolder = null;
    _error = null;
    final channel = _channel;
    _channel = null;
    _client = null;
    _stream = null;
    notifyListeners();
    await channel?.shutdown();
  }

  @override
  void dispose() {
    _disposed = true;
    _channel?.shutdown();
    super.dispose();
  }
}

/// Participante de una sala tal y como se muestra en la UI.
@immutable
class Participant {
  const Participant({
    required this.ssrc,
    this.isLocal = false,
    this.hasFloor = false,
  });

  /// SSRC del participante.
  final int ssrc;

  /// `true` si es este dispositivo.
  final bool isLocal;

  /// `true` si tiene el turno concedido.
  final bool hasFloor;

  /// Etiqueta estable para la lista.
  ///
  /// El ABI C no expone nombres de usuario, así que se identifica por SSRC.
  String get label => switch (ssrc) {
        0 => 'Host',
        _ => 'Peer 0x${ssrc.toRadixString(16).toUpperCase()}',
      };

  /// Descripción de estado para lectores de pantalla.
  String get statusLabel {
    if (hasFloor) return 'transmitiendo';
    if (isLocal) return 'tú';
    return 'en espera';
  }
}
