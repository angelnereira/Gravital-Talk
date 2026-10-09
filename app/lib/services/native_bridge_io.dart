import 'dart:ffi';
import 'dart:io';
import 'dart:typed_data';

import 'package:ffi/ffi.dart';

import '../models/reachability.dart';
import '../models/session.dart';
import 'ffi_common.dart';

// ── Structs C ─────────────────────────────────────────────────────────────

final class GsConfigNative extends Struct {
  @Uint32()
  external int sampleRate;

  @Uint8()
  external int channels;

  @Uint8()
  external int frameDurationMs;

  @Uint32()
  external int maxBitrate;

  @Uint8()
  external int codecPreferred;

  @Uint32()
  external int capabilityFlags;

  @Uint16()
  external int jitterBufferMs;

  @Uint32()
  external int mtu;
}

final class GsMetricsNative extends Struct {
  @Float()
  external double rttMs;

  @Float()
  external double jitterMs;

  @Float()
  external double lossPercent;

  @Float()
  external double reorderPercent;

  @Float()
  external double bufferFillPercent;

  @Float()
  external double estimatedMos;

  @Uint64()
  external int packetsSent;

  @Uint64()
  external int packetsReceived;

  @Uint64()
  external int bytesSent;

  @Uint64()
  external int bytesReceived;
}

// ── Firmas nativas (tipos FFI) y Dart ─────────────────────────────────────

typedef _VersionC = Pointer<Utf8> Function();
typedef _VersionD = Pointer<Utf8> Function();
typedef _Uint32C = Uint32 Function();
typedef _Uint32D = int Function();
typedef _IntC = Int32 Function();
typedef _IntD = int Function();
typedef _ConfigDefaultC = Int32 Function(Pointer<GsConfigNative>);
typedef _ConfigDefaultD = int Function(Pointer<GsConfigNative>);
typedef _SessionCreateC = Int32 Function(
    Pointer<GsConfigNative>, Pointer<Utf8>, Uint16, Pointer<Pointer<Void>>);
typedef _SessionCreateD = int Function(
    Pointer<GsConfigNative>, Pointer<Utf8>, int, Pointer<Pointer<Void>>);
typedef _SessionDestroyC = Void Function(Pointer<Void>);
typedef _SessionDestroyD = void Function(Pointer<Void>);
typedef _HandshakeC = Int32 Function(Pointer<Void>, Pointer<Utf8>, Uint16);
typedef _HandshakeD = int Function(Pointer<Void>, Pointer<Utf8>, int);
typedef _AcceptAnyC = Int32 Function(Pointer<Void>);
typedef _AcceptAnyD = int Function(Pointer<Void>);
typedef _SendAudioC = Int32 Function(Pointer<Void>, Pointer<Uint8>, UintPtr);
typedef _SendAudioD = int Function(Pointer<Void>, Pointer<Uint8>, int);
typedef _RecvAudioC = Int32 Function(
    Pointer<Void>, Pointer<Uint8>, Pointer<UintPtr>);
typedef _RecvAudioD = int Function(
    Pointer<Void>, Pointer<Uint8>, Pointer<UintPtr>);
typedef _RecvAudioTimeoutC = Int32 Function(
    Pointer<Void>, Pointer<Uint8>, Pointer<UintPtr>, Uint32);
typedef _RecvAudioTimeoutD = int Function(
    Pointer<Void>, Pointer<Uint8>, Pointer<UintPtr>, int);
typedef _HandleC = Int32 Function(Pointer<Void>);
typedef _HandleD = int Function(Pointer<Void>);
typedef _StateC = Int32 Function(Pointer<Void>, Pointer<Uint8>);
typedef _StateD = int Function(Pointer<Void>, Pointer<Uint8>);
typedef _SessionIdC = Int32 Function(Pointer<Void>, Pointer<Uint32>);
typedef _SessionIdD = int Function(Pointer<Void>, Pointer<Uint32>);
typedef _SetSessionIdC = Int32 Function(Pointer<Void>, Uint32);
typedef _SetSessionIdD = int Function(Pointer<Void>, int);
typedef _SetRoomTokenC = Int32 Function(Pointer<Void>, Pointer<Utf8>);
typedef _SetRoomTokenD = int Function(Pointer<Void>, Pointer<Utf8>);
typedef _SetHandshakeModeC = Int32 Function(Pointer<Void>, Uint8);
typedef _SetHandshakeModeD = int Function(Pointer<Void>, int);
typedef _MetricsC = Int32 Function(Pointer<Void>, Pointer<GsMetricsNative>);
typedef _MetricsD = int Function(Pointer<Void>, Pointer<GsMetricsNative>);
typedef _LocalPortC = Int32 Function(Pointer<Void>, Pointer<Uint16>);
typedef _LocalPortD = int Function(Pointer<Void>, Pointer<Uint16>);
typedef _ErrorLastC = Pointer<Utf8> Function();
typedef _ErrorLastD = Pointer<Utf8> Function();
typedef _ClearC = Void Function();
typedef _ClearD = void Function();
typedef _DiscoverStunC = Int32 Function(Uint16, Pointer<Utf8>, UintPtr);
typedef _ReachC = Int32 Function(Uint16, Pointer<Int32>);
typedef _PreferredPortC = Int32 Function(Uint16, Pointer<Uint16>);
typedef _ReachD = int Function(int, Pointer<Int32>);
typedef _PreferredPortD = int Function(int, Pointer<Uint16>);
typedef _DiscoverStunD = int Function(int, Pointer<Utf8>, int);

/// Envoltorio tipado de `libgravital_talk_ffi` (ABI C v1).
class NativeBridge {
  NativeBridge._(DynamicLibrary lib)
      : version = lib.lookupFunction<_VersionC, _VersionD>('gs_version'),
        protocolVersion =
            lib.lookupFunction<_Uint32C, _Uint32D>('gs_protocol_version'),
        abiVersion =
            lib.lookupFunction<_Uint32C, _Uint32D>('gs_abi_version'),
        _configDefault = lib
            .lookupFunction<_ConfigDefaultC, _ConfigDefaultD>('gs_config_default'),
        _sessionCreate = lib
            .lookupFunction<_SessionCreateC, _SessionCreateD>('gs_session_create'),
        _sessionDestroy = lib
            .lookupFunction<_SessionDestroyC, _SessionDestroyD>('gs_session_destroy'),
        _sessionConnect = lib
            .lookupFunction<_HandshakeC, _HandshakeD>('gs_session_connect'),
        _sessionAccept = lib
            .lookupFunction<_HandshakeC, _HandshakeD>('gs_session_accept'),
        _sessionAcceptAny = lib
            .lookupFunction<_AcceptAnyC, _AcceptAnyD>('gs_session_accept_any'),
        _sendAudio = lib
            .lookupFunction<_SendAudioC, _SendAudioD>('gs_session_send_audio'),
        _recvAudio = lib
            .lookupFunction<_RecvAudioC, _RecvAudioD>('gs_session_recv_audio'),
        _recvAudioTimeout = lib.lookupFunction<_RecvAudioTimeoutC, _RecvAudioTimeoutD>(
            'gs_session_recv_audio_timeout'),
        _sessionClose = lib
            .lookupFunction<_HandleC, _HandleD>('gs_session_close'),
        _sessionReopen =
            lib.lookupFunction<_HandleC, _HandleD>('gs_session_reopen'),
        _sessionState = lib
            .lookupFunction<_StateC, _StateD>('gs_session_state'),
        _sessionId = lib
            .lookupFunction<_SessionIdC, _SessionIdD>('gs_session_id'),
        _setSessionId = lib.lookupFunction<_SetSessionIdC, _SetSessionIdD>(
            'gs_session_set_session_id'),
        _setRoomToken = lib.lookupFunction<_SetRoomTokenC, _SetRoomTokenD>(
            'gs_session_set_room_token'),
        _setHandshakeMode = lib.lookupFunction<_SetHandshakeModeC, _SetHandshakeModeD>(
            'gs_session_set_handshake_mode'),
        _sessionMetrics = lib
            .lookupFunction<_MetricsC, _MetricsD>('gs_session_metrics'),
        _sessionLocalPort = lib
            .lookupFunction<_LocalPortC, _LocalPortD>('gs_session_local_port'),
        _pttPress = lib
            .lookupFunction<_HandleC, _HandleD>('gs_session_ptt_press'),
        _pttRelease =
            lib.lookupFunction<_HandleC, _HandleD>('gs_session_ptt_release'),
        _peerPttActive = lib
            .lookupFunction<_HandleC, _HandleD>('gs_session_is_peer_ptt_active'),
        _errorLast = lib
            .lookupFunction<_ErrorLastC, _ErrorLastD>('gs_error_last'),
        _errorClear = lib.lookupFunction<_ClearC, _ClearD>('gs_error_clear'),
        _discoverPublicAddr = lib.lookupFunction<_DiscoverStunC, _DiscoverStunD>(
            'gs_discover_public_addr'),
        _diagnoseReachability =
            lib.lookupFunction<_ReachC, _ReachD>('gs_diagnose_reachability'),
        _preferredHostPort =
            lib.lookupFunction<_PreferredPortC, _PreferredPortD>('gs_preferred_host_port'),
        _ping = lib.lookupFunction<_IntC, _IntD>('gs_ping');

  final Pointer<Utf8> Function() version;
  final int Function() protocolVersion;
  final int Function() abiVersion;
  final _ConfigDefaultD _configDefault;
  final _SessionCreateD _sessionCreate;
  final _SessionDestroyD _sessionDestroy;
  final _HandshakeD _sessionConnect;
  final _HandshakeD _sessionAccept;
  final _AcceptAnyD _sessionAcceptAny;
  final _SendAudioD _sendAudio;
  final _RecvAudioD _recvAudio;
  final _RecvAudioTimeoutD _recvAudioTimeout;
  final _HandleD _sessionClose;
  final _HandleD _sessionReopen;
  final _StateD _sessionState;
  final _SessionIdD _sessionId;
  final _SetSessionIdD _setSessionId;
  final _SetRoomTokenD _setRoomToken;
  final _SetHandshakeModeD _setHandshakeMode;
  final _MetricsD _sessionMetrics;
  final _LocalPortD _sessionLocalPort;
  final _HandleD _pttPress;
  final _HandleD _pttRelease;
  final _HandleD _peerPttActive;
  final _ErrorLastD _errorLast;
  final _ClearD _errorClear;
  final _DiscoverStunD _discoverPublicAddr;
  final _ReachD _diagnoseReachability;
  final _PreferredPortD _preferredHostPort;
  final _IntD _ping;

  /// Versión SemVer de la librería nativa.
  String get nativeVersion => version().toDartString();

  /// Intenta cargar la librería nativa.
  ///
  /// [explicitPath] permite apuntar a un `.so`/`.dylib`/`.dll` concreto
  /// (ajustes avanzados). Devuelve `null` si no está disponible.
  static NativeBridge? tryLoad({String? explicitPath}) {
    final candidates = <String>[
      if (explicitPath != null && explicitPath.isNotEmpty) explicitPath,
      if (Platform.isWindows) 'gravital_talk_ffi.dll',
      if (Platform.isLinux || Platform.isAndroid) 'libgravital_talk_ffi.so',
      if (Platform.isMacOS) 'libgravital_talk_ffi.dylib',
      // Rutas de desarrollo (flutter run desde `app/`).
      if (Platform.isLinux) '../target/release/libgravital_talk_ffi.so',
      if (Platform.isLinux) '../target/debug/libgravital_talk_ffi.so',
      if (Platform.isMacOS) '../target/release/libgravital_talk_ffi.dylib',
      if (Platform.isMacOS) '../target/debug/libgravital_talk_ffi.dylib',
      if (Platform.isWindows) r'..\target\release\gravital_talk_ffi.dll',
      if (Platform.isWindows) r'..\target\debug\gravital_talk_ffi.dll',
    ];
    for (final candidate in candidates) {
      try {
        return NativeBridge._(DynamicLibrary.open(candidate));
      } on ArgumentError {
        continue;
      } on UnsupportedError {
        continue;
      }
    }
    return null;
  }

  /// Ping de enlazado (siempre 0 si la librería está viva).
  bool ping() => _ping() == 0;

  /// Ejecuta un handshake bloqueante.
  ///
  /// Pensado para invocarse desde un isolate worker: recibe la dirección del
  /// handle (los `Pointer` no cruzan isolates, pero la dirección sí).
  ({int status, String? error}) runHandshake(
    int handleAddress,
    String host,
    int port, {
    required bool server,
  }) {
    final ptr = Pointer<Void>.fromAddress(handleAddress);
    final cHost = host.toNativeUtf8();
    try {
      final st = server
          ? _sessionAccept(ptr, cHost, port)
          : _sessionConnect(ptr, cHost, port);
      return (status: st, error: st == GsStatus.ok ? null : lastError());
    } finally {
      calloc.free(cHost);
    }
  }

  /// Igual que [runHandshake] pero aceptando el primer peer (P2P abierto).
  ({int status, String? error}) runAcceptAny(int handleAddress) {
    final ptr = Pointer<Void>.fromAddress(handleAddress);
    final st = _sessionAcceptAny(ptr);
    return (status: st, error: st == GsStatus.ok ? null : lastError());
  }

  /// Último error reportado por la librería (y lo limpia).
  String? lastError() {
    final ptr = _errorLast();
    if (ptr == nullptr) return null;
    final msg = ptr.toDartString();
    _errorClear();
    return msg;
  }

  void _check(int status) {
    if (status == GsStatus.ok) return;
    throw NativeException(status, lastError() ?? GsStatus.describe(status));
  }

  /// Crea una sesión nativa bindeando un socket UDP.
  NativeSession createSession({
    required int sampleRate,
    required int channels,
    required int frameDurationMs,
    required int maxBitrate,
    required int codecPreferred,
    required int jitterBufferMs,
    required int mtu,
    String bindAddr = '0.0.0.0',
    int bindPort = 0,
  }) {
    final cfg = calloc<GsConfigNative>();
    final addr = bindAddr.toNativeUtf8();
    final outHandle = calloc<Pointer<Void>>();
    try {
      _check(_configDefault(cfg));
      cfg.ref
        ..sampleRate = sampleRate
        ..channels = channels
        ..frameDurationMs = frameDurationMs
        ..maxBitrate = maxBitrate
        ..codecPreferred = codecPreferred
        ..jitterBufferMs = jitterBufferMs
        ..mtu = mtu;
      _check(_sessionCreate(cfg, addr, bindPort, outHandle));
      return NativeSession._(this, outHandle.value);
    } finally {
      calloc.free(cfg);
      calloc.free(addr);
      calloc.free(outHandle);
    }
  }

  /// Diagnóstica si este dispositivo puede recibir invitados de fuera.
  ///
  /// Es la comprobación que evita el fallo silencioso: un anfitrión detrás de
  /// CGNAT mostraría un QR que nunca funciona. Devuelve `null` si no se pudo
  /// determinar (STUN no respondió), que la UI trata como "prueba y avisa".
  NetworkReachability? diagnoseReachability(int localPort) {
    final out = calloc<Int32>();
    try {
      final st = _diagnoseReachability(localPort, out);
      if (st != GsStatus.ok) return null;
      return NetworkReachability.fromCode(out.value);
    } finally {
      calloc.free(out);
    }
  }

  /// Puerto que dedica Gravital Talk al anfitrión según la política.
  ///
  /// `preferred` de 0 = el que sea más rápido y sencillo (9000 si está libre).
  int preferredHostPort(int preferred) {
    final out = calloc<Uint16>();
    try {
      _check(_preferredHostPort(preferred, out));
      return out.value;
    } finally {
      calloc.free(out);
    }
  }

  /// Descubre la IP pública vía STUN. Devuelve `null` si falla.
  String? discoverPublicAddress(int localPort) {
    const bufLen = 128;
    final buf = calloc<Uint8>(bufLen);
    try {
      final st = _discoverPublicAddr(localPort, buf.cast(), bufLen);
      if (st != GsStatus.ok) return null;
      return buf.cast<Utf8>().toDartString();
    } finally {
      calloc.free(buf);
    }
  }
}

/// Handle opaco a una sesión nativa.
class NativeSession {
  NativeSession._(this._bridge, this._handle);

  /// Reconstruye la vista de una sesión desde su dirección de handle.
  ///
  /// Permite que otro isolate (p. ej. el de audio) use la misma sesión:
  /// los `Pointer` no cruzan isolates, pero la dirección numérica sí.
  factory NativeSession.at(NativeBridge bridge, int address) =>
      NativeSession._(bridge, Pointer<Void>.fromAddress(address));

  final NativeBridge _bridge;
  final Pointer<Void> _handle;

  bool _destroyed = false;

  /// Dirección del handle nativo (para pasarla a otros isolates).
  int get address => _handle.address;

  void _ensureAlive() {
    if (_destroyed) {
      throw NativeException(GsStatus.closed, 'sesión destruida');
    }
  }

  /// Handshake como cliente.
  void connect(String host, int port) {
    _ensureAlive();
    final cHost = host.toNativeUtf8();
    try {
      _bridge._check(_bridge._sessionConnect(_handle, cHost, port));
    } finally {
      calloc.free(cHost);
    }
  }

  /// Handshake como servidor esperando a un peer conocido.
  void accept(String host, int port) {
    _ensureAlive();
    final cHost = host.toNativeUtf8();
    try {
      _bridge._check(_bridge._sessionAccept(_handle, cHost, port));
    } finally {
      calloc.free(cHost);
    }
  }

  /// Handshake como servidor aceptando el primer cliente (modo QR/P2P).
  void acceptAny() {
    _ensureAlive();
    _bridge._check(_bridge._sessionAcceptAny(_handle));
  }

  /// Fija el `session_id` de sala antes del handshake.
  void setSessionId(int id) {
    _ensureAlive();
    _bridge._check(_bridge._setSessionId(_handle, id));
  }

  /// Fija el token de sala (PSK de Noise). `null` lo desactiva.
  void setRoomToken(String? token) {
    _ensureAlive();
    final ptr = token == null ? nullptr : token.toNativeUtf8();
    try {
      _bridge._check(_bridge._setRoomToken(_handle, ptr));
    } finally {
      if (ptr != nullptr) calloc.free(ptr);
    }
  }

  /// Modo de handshake: 0 = Auto, 1 = Noise, 2 = Legacy.
  void setHandshakeMode(int mode) {
    _ensureAlive();
    _bridge._check(_bridge._setHandshakeMode(_handle, mode));
  }

  /// Envía un frame PCM.
  void sendAudio(Uint8List data) {
    _ensureAlive();
    final ptr = calloc<Uint8>(data.length);
    try {
      ptr.asTypedList(data.length).setAll(0, data);
      _bridge._check(_bridge._sendAudio(_handle, ptr, data.length));
    } finally {
      calloc.free(ptr);
    }
  }

  /// Espera el próximo frame PCM (bloqueante).
  Uint8List? recvAudio({int maxBytes = 8192}) {
    _ensureAlive();
    final buf = calloc<Uint8>(maxBytes);
    final len = calloc<UintPtr>();
    try {
      len.value = maxBytes;
      final st = _bridge._recvAudio(_handle, buf, len);
      if (st != GsStatus.ok) return null;
      return Uint8List.fromList(buf.asTypedList(len.value));
    } finally {
      calloc.free(buf);
      calloc.free(len);
    }
  }

  /// Igual que [recvAudio] pero con timeout (ms).
  ///
  /// Devuelve `null` si no hay frame en el plazo (`GS_ERR_TIMEOUT`); lanza
  /// [NativeException] para otros errores. Con `timeoutMs = 0` no bloquea.
  Uint8List? recvAudioTimeout({int maxBytes = 8192, int timeoutMs = 250}) {
    _ensureAlive();
    final buf = calloc<Uint8>(maxBytes);
    final len = calloc<UintPtr>();
    try {
      len.value = maxBytes;
      final st = _bridge._recvAudioTimeout(_handle, buf, len, timeoutMs);
      if (st == GsStatus.timeout) return null;
      if (st != GsStatus.ok) {
        throw NativeException(
            st, _bridge.lastError() ?? GsStatus.describe(st));
      }
      return Uint8List.fromList(buf.asTypedList(len.value));
    } finally {
      calloc.free(buf);
      calloc.free(len);
    }
  }

  /// Rearma la sesión cerrada para poder emparejar de nuevo.
  ///
  /// Es el requisito de "cualquiera de los dos cierra y vuelve a solicitar el
  /// emparejamiento". Sin esto, `close()` deja el handle inservible y sólo cabe
  /// destruirlo y recrearlo, perdiendo la configuración de la sala.
  void reopen() {
    if (_destroyed) return;
    _bridge._check(_bridge._sessionReopen(_handle));
  }

  /// Cierra la sesión (envía CLOSE).
  void close() {
    if (_destroyed) return;
    _bridge._check(_bridge._sessionClose(_handle));
  }

  /// Estado actual.
  SessionState state() {
    _ensureAlive();
    final out = calloc<Uint8>();
    try {
      _bridge._check(_bridge._sessionState(_handle, out));
      return SessionState.fromCode(out.value);
    } finally {
      calloc.free(out);
    }
  }

  /// `session_id` negociado (0 antes del handshake).
  int sessionId() {
    _ensureAlive();
    final out = calloc<Uint32>();
    try {
      _bridge._check(_bridge._sessionId(_handle, out));
      return out.value;
    } finally {
      calloc.free(out);
    }
  }

  /// Puerto UDP local.
  int localPort() {
    _ensureAlive();
    final out = calloc<Uint16>();
    try {
      _bridge._check(_bridge._sessionLocalPort(_handle, out));
      return out.value;
    } finally {
      calloc.free(out);
    }
  }

  /// Snapshot de métricas.
  SessionMetrics metrics() {
    _ensureAlive();
    final out = calloc<GsMetricsNative>();
    try {
      _bridge._check(_bridge._sessionMetrics(_handle, out));
      final m = out.ref;
      return SessionMetrics(
        rttMs: m.rttMs,
        jitterMs: m.jitterMs,
        lossPercent: m.lossPercent,
        reorderPercent: m.reorderPercent,
        bufferFillPercent: m.bufferFillPercent,
        estimatedMos: m.estimatedMos,
        packetsSent: m.packetsSent,
        packetsReceived: m.packetsReceived,
        bytesSent: m.bytesSent,
        bytesReceived: m.bytesReceived,
      );
    } finally {
      calloc.free(out);
    }
  }

  void pttPress() {
    _ensureAlive();
    _bridge._check(_bridge._pttPress(_handle));
  }

  void pttRelease() {
    _ensureAlive();
    _bridge._check(_bridge._pttRelease(_handle));
  }

  bool peerPttActive() {
    _ensureAlive();
    return _bridge._peerPttActive(_handle) == 1;
  }

  /// Libera el handle nativo (no envía CLOSE; usar `close()` antes).
  void destroy() {
    if (_destroyed) return;
    _destroyed = true;
    _bridge._sessionDestroy(_handle);
  }
}