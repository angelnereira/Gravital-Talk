import 'dart:async';
import 'dart:isolate';
import 'dart:math';

import 'package:flutter/foundation.dart';

import '../models/connection.dart';
import '../models/reachability.dart';
import '../models/session.dart';
import 'audio_pump.dart';
import 'native_bridge.dart';

/// Contrato común de ambos motores (nativo real / demo).
abstract class SessionEngine {
  EngineKind get kind;

  /// Descripción corta para la UI (versión nativa, etc.).
  String get detail;

  /// Crea la sesión bindeando un socket local.
  Future<void> createSession(AppSettings settings, {int bindPort = 0});

  /// Fija el `session_id` de sala antes del handshake.
  Future<void> setSessionId(int id);

  /// Fija el token de sala (PSK de Noise). `null` lo desactiva.
  Future<void> setRoomToken(String? token);

  Future<void> connect(String host, int port);
  Future<void> accept(String host, int port);
  Future<void> acceptAny();

  Future<void> pttPress();
  Future<void> pttRelease();
  Future<bool> peerPttActive();

  /// Informa al pipeline de audio si PTT está activo (envío de micrófono).
  Future<void> setAudioPtt(bool on);

  Future<SessionMetrics> metrics();
  Future<SessionState> state();
  Future<int> localPort();
  Future<int> sessionId();

  /// Descubre la IP pública de esta sesión vía STUN.
  ///
  /// Es lo que va en el QR: sin ella, el invitado tendría que teclear la
  /// dirección a mano. Devuelve `null` si STUN no responde.
  Future<String?> discoverPublicEndpoint();

  /// Diagnostica si este dispositivo puede recibir invitados de fuera.
  ///
  /// Es la comprobación que evita el fallo silencioso: sin ella, un anfitrión
  /// detrás de CGNAT muestra un QR que nunca va a funcionar y nadie entiende
  /// por qué.
  ///
  /// `null` si no se pudo determinar (sin red, sin STUN, o motor demo).
  Future<NetworkReachability?> diagnoseReachability();

  /// Nivel de micrófono 0..1 para el VU meter.
  double get micLevel;

  Future<void> close();
  void dispose();
}

/// Motor nativo: habla con `libgravital_talk_ffi` (Rust real).
class FfiSessionEngine implements SessionEngine {
  FfiSessionEngine(this._bridge);

  @override
  Future<String?> discoverPublicEndpoint() async {
    final session = _session;
    if (session == null) return null;
    return _bridge.discoverPublicAddress(session.localPort());
  }

  @override
  Future<NetworkReachability?> diagnoseReachability() async {
    final session = _session;
    if (session == null) return null;
    return session.diagnoseReachability();
  }

  final NativeBridge _bridge;
  NativeSession? _session;
  AudioPump? _pump;
  AppSettings _settings = const AppSettings();

  static FfiSessionEngine? tryCreate() {
    if (kIsWeb) return null;
    final bridge = NativeBridge.tryLoad();
    if (bridge == null) return null;
    try {
      if (!bridge.ping()) return null;
    } catch (_) {
      return null;
    }
    return FfiSessionEngine(bridge);
  }

  @override
  EngineKind get kind => EngineKind.native;

  @override
  String get detail =>
      'v${_bridge.nativeVersion} · ABI ${_bridge.abiVersion()} · proto ${_bridge.protocolVersion()}';

  NativeSession get _alive {
    final s = _session;
    if (s == null) throw StateError('no hay sesión creada');
    return s;
  }

  @override
  Future<void> createSession(AppSettings settings,
      {int bindPort = 0}) async {
    _settings = settings;
    await _stopPump();
    _session?.destroy();
    _session = _bridge.createSession(
      sampleRate: settings.sampleRate,
      channels: settings.channels,
      frameDurationMs: settings.frameDurationMs,
      maxBitrate: settings.maxBitrate,
      codecPreferred: settings.codec.code,
      jitterBufferMs: settings.jitterBufferMs,
      mtu: settings.mtu,
      bindPort: bindPort,
    );
  }

  Future<void> _startPump() async {
    final session = _session;
    if (session == null) return;
    await _stopPump();
    try {
      _pump = await AudioPump.start(
        handleAddress: session.address,
        sampleRate: _settings.sampleRate,
        channels: _settings.channels,
        frameDurationMs: _settings.frameDurationMs,
      );
    } catch (_) {
      _pump = null; // audio simulatorio no crítico para la señalización
    }
  }

  Future<void> _stopPump() async {
    final pump = _pump;
    _pump = null;
    await pump?.stop();
  }

  @override
  Future<void> setSessionId(int id) async => _alive.setSessionId(id);

  @override
  Future<void> setRoomToken(String? token) async => _alive.setRoomToken(token);

  @override
  Future<void> connect(String host, int port) async {
    final res = await _handshake(_alive.address, host, port, server: false);
    _throwOnError(res);
    await _startPump();
  }

  @override
  Future<void> accept(String host, int port) async {
    final res = await _handshake(_alive.address, host, port, server: true);
    _throwOnError(res);
    await _startPump();
  }

  @override
  Future<void> acceptAny() async {
    final res = await _acceptAnyIsolate(_alive.address);
    _throwOnError(res);
    await _startPump();
  }

  void _throwOnError(({int status, String? error}) res) {
    if (res.status != GsStatus.ok) {
      throw NativeException(
          res.status, res.error ?? GsStatus.describe(res.status));
    }
  }

  @override
  Future<void> pttPress() async {
    _alive.pttPress();
    _pump?.setPtt(true);
  }

  @override
  Future<void> pttRelease() async {
    _alive.pttRelease();
    _pump?.setPtt(false);
  }

  @override
  Future<void> setAudioPtt(bool on) async => _pump?.setPtt(on);

  @override
  Future<bool> peerPttActive() async => _alive.peerPttActive();

  @override
  Future<SessionMetrics> metrics() async => _alive.metrics();

  @override
  Future<SessionState> state() async => _alive.state();

  @override
  Future<int> localPort() async => _alive.localPort();

  @override
  Future<int> sessionId() async => _alive.sessionId();

  @override
  double get micLevel => _pump?.micLevel ?? 0; // Nivel real del micrófono (RMS).

  @override
  Future<void> close() async {
    await _stopPump();
    try {
      _alive.close();
    } on NativeException {
      // Sesión ya cerrada.
    }
  }

  @override
  void dispose() {
    unawaited(_stopPump());
    _session?.destroy();
    _session = null;
  }
}

/// Handshake bloqueante en un isolate para no congelar la UI.
Future<({int status, String? error})> _handshake(
    int handleAddr, String host, int port, {required bool server}) {
  return Isolate.run(() {
    final bridge = NativeBridge.tryLoad();
    if (bridge == null) {
      return (status: GsStatus.internal, error: 'librería nativa no disponible');
    }
    return bridge.runHandshake(handleAddr, host, port, server: server);
  });
}

Future<({int status, String? error})> _acceptAnyIsolate(int handleAddr) {
  return Isolate.run(() {
    final bridge = NativeBridge.tryLoad();
    if (bridge == null) {
      return (status: GsStatus.internal, error: 'librería nativa no disponible');
    }
    return bridge.runAcceptAny(handleAddr);
  });
}

/// Motor demo: simula el protocolo para desarrollar UI sin librería nativa.
class DemoSessionEngine implements SessionEngine {
  DemoSessionEngine({Random? random}) : _rng = random ?? Random();

  final Random _rng;
  SessionState _state = SessionState.idle;
  int _sessionId = 0;
  int _localPort = 0;
  bool _ptt = false;
  bool _peerPtt = false;
  DateTime _lastPeerToggle = DateTime.now();
  double _micLevel = 0;

  Timer? _handshakeTimer;

  @override
  EngineKind get kind => EngineKind.demo;

  @override
  String get detail => 'Simulación local · sin librería nativa';

  @override
  Future<void> createSession(AppSettings settings,
      {int bindPort = 0}) async {
    _localPort = bindPort != 0 ? bindPort : 40000 + _rng.nextInt(20000);
    _state = SessionState.idle;
  }

  @override
  Future<void> setSessionId(int id) async => _sessionId = id;

  @override
  Future<void> setRoomToken(String? token) async {
    // El motor demo no implementa PSK; se ignora.
  }

  Future<void> _beginHandshake() async {
    _state = SessionState.handshaking;
    await Future<void>.delayed(const Duration(milliseconds: 300));
    _handshakeTimer?.cancel();
    _handshakeTimer = Timer(const Duration(milliseconds: 1500), () {
      if (_sessionId == 0) {
        _sessionId = 0x10000000 + _rng.nextInt(0x0FFFFFFF);
      }
      _state = SessionState.active;
    });
  }

  @override
  Future<void> connect(String host, int port) => _beginHandshake();

  @override
  Future<void> accept(String host, int port) => _beginHandshake();

  @override
  Future<void> acceptAny() => _beginHandshake();

  @override
  Future<void> pttPress() async => _ptt = true;

  @override
  Future<void> pttRelease() async => _ptt = false;

  @override
  Future<void> setAudioPtt(bool on) async => _ptt = on;

  @override
  Future<bool> peerPttActive() async {
    if (_state != SessionState.active) return false;
    final now = DateTime.now();
    if (now.difference(_lastPeerToggle).inMilliseconds > 1800) {
      _lastPeerToggle = now;
      _peerPtt = _rng.nextInt(100) < 30; // 30 % del tiempo habla el peer
    }
    return _peerPtt && !_ptt;
  }

  @override
  Future<SessionMetrics> metrics() async {
    if (_state != SessionState.active) return SessionMetrics.zero;
    double walk(double current, double min, double max, double step) {
      final next = current + (_rng.nextDouble() - 0.5) * step;
      return next.clamp(min, max);
    }

    _micLevel = _ptt
        ? walk(_micLevel == 0 ? 0.6 : _micLevel, 0.15, 0.98, 0.3)
        : 0;
    final rtt = walk(12 + _rng.nextDouble() * 8, 4, 120, 4);
    final jitter = walk(4 + _rng.nextDouble() * 3, 0.5, 30, 2);
    final loss = walk(0.4 + _rng.nextDouble() * 0.6, 0, 8, 0.5);
    final mos = (4.5 - rtt / 120 - loss / 5 - jitter / 60).clamp(1.0, 4.8);
    return SessionMetrics(
      rttMs: rtt,
      jitterMs: jitter,
      lossPercent: loss,
      reorderPercent: walk(0.2, 0, 2, 0.2),
      bufferFillPercent: walk(45, 10, 95, 8),
      estimatedMos: mos,
      packetsSent: 1000 + _rng.nextInt(9000),
      packetsReceived: 1000 + _rng.nextInt(9000),
      bytesSent: 512000 + _rng.nextInt(900000),
      bytesReceived: 512000 + _rng.nextInt(900000),
    );
  }

  @override
  Future<SessionState> state() async => _state;

  @override
  Future<int> localPort() async => _localPort;

  @override
  Future<int> sessionId() async => _sessionId;

  @override
  double get micLevel => _micLevel;

  // El motor demo no tiene red real. Devuelve un endpoint ficticio para que la
  // UI se pueda recorrer de punta a punta: sin él, la pantalla de crear sala
  // no puede mostrar QR y no hay forma de probar el flujo.
  //
  // Es deliberadamente obvio que es falso: la IP es de documentación. Quien lo
  // ve en la pantalla no puede confundirlo con una dirección real.
  @override
  Future<String?> discoverPublicEndpoint() async => '203.0.113.7:40000';

  @override
  Future<NetworkReachability?> diagnoseReachability() async =>
      NetworkReachability.lanOnly;

  @override
  Future<void> close() async {
    _handshakeTimer?.cancel();
    _state = SessionState.closed;
  }

  @override
  void dispose() {
    _handshakeTimer?.cancel();
  }
}
