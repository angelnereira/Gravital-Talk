import 'dart:async';
import 'dart:io';
import 'dart:math';

import 'package:flutter/foundation.dart';

import '../models/connection.dart';
import '../models/session.dart';
import 'engine.dart';
import 'event_log.dart';
import 'native_bridge.dart';
import 'room_api.dart';
import 'settings_store.dart';

/// Orquesta el ciclo de vida de la conexión (servidor o P2P) sobre un motor.
class SessionController extends ChangeNotifier {
  SessionController(
    this._engine,
    this._roomApi,
    this._log,
    this._store,
  );

  final SessionEngine _engine;
  final RoomControlApi _roomApi;
  final EventLog _log;
  final SettingsStore _store;

  SettingsStore get store => _store;
  EventLog get log => _log;
  EngineKind get engineKind => _engine.kind;
  String get engineDetail => _engine.detail;

  SessionState _state = SessionState.idle;
  SessionMetrics _metrics = SessionMetrics.zero;
  bool _ptt = false;
  bool _peerPtt = false;
  bool _busy = false;
  String? _error;
  String? _roomCode;
  int _sessionId = 0;
  int _localPort = 0;
  Duration _elapsed = Duration.zero;
  ConnectionMode? _mode;
  ConnectionRole? _role;
  DateTime? _connectedAt;
  Timer? _ticker;

  ServerProfile _server = const ServerProfile();
  P2pProfile _p2p = const P2pProfile();
  AppSettings _settings = const AppSettings();

  SessionState get state => _state;
  SessionMetrics get metrics => _metrics;
  bool get pttActive => _ptt;
  bool get peerPttActive => _peerPtt;
  bool get busy => _busy;
  String? get error => _error;
  String? get roomCode => _roomCode;
  int get sessionId => _sessionId;
  int get localPort => _localPort;
  Duration get elapsed => _elapsed;
  ConnectionMode? get mode => _mode;
  ConnectionRole? get role => _role;
  ServerProfile get server => _server;
  P2pProfile get p2p => _p2p;
  AppSettings get settings => _settings;
  double get micLevel => _engine.micLevel;
  bool get isLive => _state.isLive;

  /// Carga perfiles y ajustes persistidos.
  Future<void> init() async {
    _server = await _store.loadServerProfile();
    _p2p = await _store.loadP2pProfile();
    _settings = await _store.loadSettings();
    notifyListeners();
  }

  void updateSettings(AppSettings s) {
    _settings = s;
    _store.saveSettings(s);
    notifyListeners();
  }

  void updateServerProfile(ServerProfile p) {
    _server = p;
    _store.saveServerProfile(p);
    notifyListeners();
  }

  void updateP2pProfile(P2pProfile p) {
    _p2p = p;
    _store.saveP2pProfile(p);
    notifyListeners();
  }

  // ── Modo servidor (sala) ────────────────────────────────────────────────

  /// Host: crea la sala en el relay y espera clientes.
  Future<bool> hostServer({String? roomCode}) async {
    final profile = _server;
    if (profile.host.isEmpty) {
      _fail('Configura la dirección del servidor');
      return false;
    }
    return _guard(() async {
      await _engine.createSession(_settings);
      if (profile.token.isNotEmpty) {
        await _engine.setRoomToken(profile.token);
      }
      final sid = Random().nextInt(0x7FFFFFFF) + 1;

      var code = roomCode;
      if (code == null || code.isEmpty) {
        _log.info('Creando sala en ${profile.host}:${profile.observabilityPort}…');
        final room = await _roomApi.createRoom(
          host: profile.host,
          port: profile.observabilityPort,
          sessionId: sid,
        );
        code = room.code;
      }
      _roomCode = code;
      _sessionId = sid;
      _engine.setSessionId(sid);
      _log.success('Sala $code lista (session_id=$sid). Esperando peers…');
      _updateServerProfile(roomCode: code);
      _mode = ConnectionMode.server;
      _role = ConnectionRole.host;
      _startTicker();
      notifyListeners();

      await _engine.accept(profile.host, profile.udpPort);
      _onConnected('Sala activa · peers conectados');
    });
  }

  /// Join: resuelve el código y se conecta al relay.
  Future<bool> joinServer() async {
    final profile = _server;
    if (profile.host.isEmpty || profile.roomCode.isEmpty) {
      _fail('Indica servidor y código de sala');
      return false;
    }
    return _guard(() async {
      _log.info('Resolviendo sala ${profile.roomCode}…');
      final room = await _roomApi.resolveRoom(
        host: profile.host,
        port: profile.observabilityPort,
        code: profile.roomCode,
      );
      _roomCode = room.code;
      _sessionId = room.sessionId;
      _log.success(
          'Sala ${room.code} → session_id=${room.sessionId} (peers: ${room.peerCount ?? '?'})');

      await _engine.createSession(_settings);
      if (profile.token.isNotEmpty) {
        await _engine.setRoomToken(profile.token);
      }
      _engine.setSessionId(room.sessionId);
      _mode = ConnectionMode.server;
      _role = ConnectionRole.join;
      _startTicker();
      notifyListeners();

      await _engine.connect(profile.host, profile.udpPort);
      _onConnected('Conectado a la sala');
    });
  }

  // ── Modo P2P directo ────────────────────────────────────────────────────

  /// Host P2P: escucha y acepta el primer peer (QR/aceptar cualquiera).
  Future<bool> hostP2p() async {
    return _guard(() async {
      final p = _p2p;
      await _engine.createSession(_settings,
          bindPort: p.localPort);
      _localPort = await _engine.localPort();
      _log.info('Escuchando P2P en UDP :$_localPort — esperando peer…');
      _mode = ConnectionMode.p2p;
      _role = ConnectionRole.host;
      _startTicker();
      notifyListeners();

      await _engine.acceptAny();
      _onConnected('Peer P2P conectado');
    });
  }

  /// Join P2P: conexión directa a un peer.
  Future<bool> joinP2p() async {
    final p = _p2p;
    if (p.host.isEmpty) {
      _fail('Indica la IP del peer');
      return false;
    }
    return _guard(() async {
      await _engine.createSession(_settings);
      _mode = ConnectionMode.p2p;
      _role = ConnectionRole.join;
      _startTicker();
      notifyListeners();

      _log.info('Conectando P2P a ${p.host}:${p.port}…');
      await _engine.connect(p.host, p.port);
      _onConnected('Conectado P2P');
    });
  }

  // ── PTT ─────────────────────────────────────────────────────────────────

  Future<void> pttDown() async {
    if (!isLive || _ptt) return;
    _ptt = true;
    await _engine.pttPress();
    _log.info('PTT: transmitiendo');
    notifyListeners();
  }

  Future<void> pttUp() async {
    if (!_ptt) return;
    _ptt = false;
    await _engine.pttRelease();
    notifyListeners();
  }

  // ── Ciclo de vida ───────────────────────────────────────────────────────

  Future<void> disconnect() async {
    _ticker?.cancel();
    _ticker = null;
    try {
      await _engine.close();
    } catch (e) {
      _log.warn('Al cerrar: $e');
    }
    _engine.dispose();
    _state = SessionState.closed;
    _connectedAt = null;
    _metrics = SessionMetrics.zero;
    _ptt = false;
    _peerPtt = false;
    _log.info('Sesión cerrada');
    notifyListeners();
  }

  /// Vuelve a idle para permitir una nueva conexión.
  Future<void> reset() async {
    await disconnect();
    _state = SessionState.idle;
    _mode = null;
    _role = null;
    _error = null;
    _elapsed = Duration.zero;
    notifyListeners();
  }

  // ── Diagnóstico ─────────────────────────────────────────────────────────

  Future<String> testRelay() async {
    final ok = await _roomApi.healthy(
      host: _server.host,
      port: _server.observabilityPort,
    );
    return ok ? 'Relay OK' : 'Relay no responde';
  }

  Future<String?> discoverPublicAddress() async {
    if (kIsWeb) return null;
    final bridge = NativeBridge.tryLoad();
    return bridge?.discoverPublicAddress(0);
  }

  /// Direcciones IPv4 locales del dispositivo (útil para compartir en P2P).
  Future<List<String>> localAddresses() async {
    if (kIsWeb) return const [];
    try {
      final ifaces = await NetworkInterface.list(
        type: InternetAddressType.IPv4,
        includeLoopback: false,
      );
      return [for (final i in ifaces) for (final a in i.addresses) a.address];
    } catch (_) {
      return const [];
    }
  }

  // ── Internos ────────────────────────────────────────────────────────────

  Future<bool> _guard(Future<void> Function() action) async {
    _busy = true;
    _error = null;
    notifyListeners();
    try {
      await action();
      return true;
    } on NativeException catch (e) {
      _fail('Error nativo: $e');
      return false;
    } on RoomApiException catch (e) {
      _fail(e.message);
      return false;
    } catch (e) {
      _fail('$e');
      return false;
    } finally {
      _busy = false;
      notifyListeners();
    }
  }

  void _onConnected(String message) {
    _state = SessionState.active;
    _connectedAt = DateTime.now();
    _log.success(message);
    notifyListeners();
  }

  void _fail(String message) {
    _error = message;
    _state = SessionState.error;
    _log.error(message);
    notifyListeners();
  }

  void _updateServerProfile({String? roomCode}) {
    _server = _server.copyWith(roomCode: roomCode);
    _store.saveServerProfile(_server);
  }

  void _startTicker() {
    _ticker?.cancel();
    _ticker = Timer.periodic(const Duration(milliseconds: 500), (_) async {
      try {
        final st = await _engine.state();
        final m = await _engine.metrics();
        final peer = await _engine.peerPttActive();
        final changed = st != _state || peer != _peerPtt;
        _state = st;
        _metrics = m;
        _peerPtt = peer;
        if (_connectedAt != null && st.isLive) {
          _elapsed = DateTime.now().difference(_connectedAt!);
        }
        if (changed || st.isLive) notifyListeners();
      } catch (_) {
        // El motor aún no está listo; el siguiente tick reintenta.
      }
    });
  }

  @override
  void dispose() {
    _ticker?.cancel();
    _engine.dispose();
    _roomApi.dispose();
    super.dispose();
  }
}
