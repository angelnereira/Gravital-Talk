import 'dart:async';
import 'dart:isolate';
import 'dart:math';

import 'package:flutter/foundation.dart';

import '../models/connection.dart';
import '../models/session.dart';
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

  Future<void> connect(String host, int port);
  Future<void> accept(String host, int port);
  Future<void> acceptAny();

  Future<void> pttPress();
  Future<void> pttRelease();
  Future<bool> peerPttActive();

  Future<SessionMetrics> metrics();
  Future<SessionState> state();
  Future<int> localPort();
  Future<int> sessionId();

  /// Nivel de micrófono 0..1 para el VU meter.
  double get micLevel;

  Future<void> close();
  void dispose();
}

/// Motor nativo: habla con `libgravital_talk_ffi` (Rust real).
class FfiSessionEngine implements SessionEngine {
  FfiSessionEngine(this._bridge);

  final NativeBridge _bridge;
  NativeSession? _session;

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

  @override
  Future<void> setSessionId(int id) async => _alive.setSessionId(id);

  @override
  Future<void> connect(String host, int port) async {
    final res = await _handshake(_alive.address, host, port, server: false);
    _throwOnError(res);
  }

  @override
  Future<void> accept(String host, int port) async {
    final res = await _handshake(_alive.address, host, port, server: true);
    _throwOnError(res);
  }

  @override
  Future<void> acceptAny() async {
    final res = await _acceptAnyIsolate(_alive.address);
    _throwOnError(res);
  }

  void _throwOnError(({int status, String? error}) res) {
    if (res.status != GsStatus.ok) {
      throw NativeException(
          res.status, res.error ?? GsStatus.describe(res.status));
    }
  }

  @override
  Future<void> pttPress() async => _alive.pttPress();

  @override
  Future<void> pttRelease() async => _alive.pttRelease();

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
  double get micLevel => 0; // La captura real llega con el pipeline de audio.

  @override
  Future<void> close() async {
    try {
      _alive.close();
    } on NativeException {
      // Sesión ya cerrada.
    }
  }

  @override
  void dispose() {
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
