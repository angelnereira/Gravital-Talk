import '../models/session.dart';
import 'ffi_common.dart';

/// Stub para plataformas sin FFI (web). Nunca devuelve una librería cargada:
/// la app usa el motor demo en web.
class NativeBridge {
  NativeBridge._();

  static NativeBridge? tryLoad({String? explicitPath}) => null;

  String get nativeVersion => 'no disponible';

  int abiVersion() => 0;

  int protocolVersion() => 0;

  bool ping() => false;

  String? discoverPublicAddress(int localPort) => null;

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
  }) =>
      NativeSession._stub();

  ({int status, String? error}) runHandshake(
    int handleAddress,
    String host,
    int port, {
    required bool server,
  }) =>
      (status: GsStatus.internal, error: 'FFI no disponible en esta plataforma');

  ({int status, String? error}) runAcceptAny(int handleAddress) =>
      (status: GsStatus.internal, error: 'FFI no disponible en esta plataforma');
}

/// Handle opaco (nunca instanciado en web).
class NativeSession {
  NativeSession._stub();

  int get address => 0;

  void connect(String host, int port) => _unavailable();
  void accept(String host, int port) => _unavailable();
  void acceptAny() => _unavailable();
  void setSessionId(int id) => _unavailable();
  void sendAudio(List<int> data) => _unavailable();
  List<int>? recvAudio({int maxBytes = 8192}) => _unavailable();
  void close() => _unavailable();
  SessionState state() => _unavailable();
  int sessionId() => _unavailable();
  int localPort() => _unavailable();
  SessionMetrics metrics() => _unavailable();
  void pttPress() => _unavailable();
  void pttRelease() => _unavailable();
  bool peerPttActive() => _unavailable();
  void destroy() => _unavailable();

  Never _unavailable() =>
      throw UnsupportedError('FFI no disponible en esta plataforma');
}