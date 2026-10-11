import 'dart:typed_data';

import '../models/reachability.dart';
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

  String? discoverLocalAddress() => null;
  void setCrashLogPath(String path) {}
  Uint8List? tonePcm({required bool end, required int sampleRate}) => null;
  Uint8List? contactId(Uint8List secret) => null;
  Uint8List? knockSeal(Uint8List secret, Uint8List payload) => null;
  Uint8List? knockOpen(Uint8List secret, Uint8List datagram) => null;

  bool get audioSupported => false;

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

  String? runMapPort(int handleAddress) => null;

  ({int status, String? error}) runConnectAny(
    int handleAddress,
    List<String> candidates, {
    int? waitMs,
  }) =>
      (status: GsStatus.internal, error: 'FFI no disponible en esta plataforma');

  ({int status, String? error}) runSetRendezvous(
    int handleAddress,
    String host,
    int port,
  ) =>
      (status: GsStatus.internal, error: 'FFI no disponible en esta plataforma');

  String? runPublicAddr(int handleAddress) => null;

  ({int status, String? error}) runAcceptAny(int handleAddress) =>
      (status: GsStatus.internal, error: 'FFI no disponible en esta plataforma');
}

/// Estado del audio nativo (nunca se produce en web).
class NativeAudioStats {
  const NativeAudioStats({
    required this.micLevel,
    required this.rxLevel,
    required this.txFrames,
    required this.rxFrames,
    required this.captureOk,
    required this.playbackOk,
  });

  final double micLevel;
  final double rxLevel;
  final int txFrames;
  final int rxFrames;
  final bool captureOk;
  final bool playbackOk;
}

/// Handle opaco (nunca instanciado en web).
class NativeSession {
  NativeSession._stub();

  /// Stub del factory para plataformas sin FFI.
  factory NativeSession.at(NativeBridge bridge, int address) =>
      NativeSession._stub();

  int get address => 0;

  void connect(String host, int port) => _unavailable();
  void accept(String host, int port) => _unavailable();
  void acceptAny() => _unavailable();
  void setSessionId(int id) => _unavailable();
  void setRoomToken(String? token) => _unavailable();
  void setHandshakeMode(int mode) => _unavailable();
  void sendAudio(List<int> data) => _unavailable();
  List<int>? recvAudio({int maxBytes = 8192}) => _unavailable();
  List<int>? recvAudioTimeout({int maxBytes = 8192, int timeoutMs = 250}) =>
      _unavailable();
  void close() => _unavailable();
  void reopen() => _unavailable();
  SessionState state() => _unavailable();
  int sessionId() => _unavailable();
  int localPort() => _unavailable();
  SessionMetrics metrics() => _unavailable();
  void pttPress() => _unavailable();
  void pttRelease() => _unavailable();
  bool peerPttActive() => _unavailable();
  void destroy() => _unavailable();
  void addPathHints(List<String> addrs) => _unavailable();
  int? rxIdleMs() => null;
  bool isRelayed() => false;
  int peerCount() => 0;
  void setNoiseSuppression(bool on) => _unavailable();
  void setBitrate(int bitsPerSecond) => _unavailable();
  int bitrate() => 0;
  bool get compressed => false;
  bool audioStart() => false;
  String? audioProblems() => null;
  void audioStop() {}
  void audioSetPtt(bool on) {}
  void audioSetMuted(bool on) {}
  void audioPlayTone({required bool end}) {}
  NativeAudioStats audioStats() => _unavailable();
  NetworkReachability? diagnoseReachability() => _unavailable();

  Never _unavailable() =>
      throw UnsupportedError('FFI no disponible en esta plataforma');
}