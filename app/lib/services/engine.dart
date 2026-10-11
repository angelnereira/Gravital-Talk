import 'dart:async';
import 'dart:isolate';
import 'dart:math';

import 'package:flutter/foundation.dart';

import '../models/connection.dart';
import '../models/reachability.dart';
import '../models/session.dart';
import 'audio_pump.dart';
import 'native_bridge.dart';

/// Estado instantáneo del audio: niveles para los medidores y si hay
/// micrófono y altavoz.
class AudioLevels {
  const AudioLevels({
    this.mic = 0,
    this.rx = 0,
    this.txFrames = 0,
    this.rxFrames = 0,
    this.micOk = true,
    this.speakerOk = true,
  });

  /// Nivel del micrófono 0..1 (cero si no se transmite).
  final double mic;

  /// Nivel de lo que se está recibiendo 0..1.
  final double rx;

  final int txFrames;
  final int rxFrames;
  final bool micOk;
  final bool speakerOk;

  static const silent = AudioLevels();
}

/// Mayor frame PCM que cabe en un paquete: tope de payload (1176 B) menos la
/// secuencia de audio (4 B) y el tag AEAD (16 B).
const maxPcmFrameBytes = 1156;

/// Duración de frame que de verdad se puede enviar con [settings].
///
/// La app manda PCM sin comprimir, y un frame de 20 ms a 48 kHz son 1920 B:
/// no cabe en un paquete y cada envío fallaba. Se acorta el frame hasta que
/// quepa en vez de dejar que la sesión conecte y no transmita nada.
int effectiveFrameMs(AppSettings settings) {
  for (final ms in [settings.frameDurationMs, 10, 5]) {
    if (ms > settings.frameDurationMs) continue;
    final bytes = (settings.sampleRate * ms ~/ 1000) * settings.channels * 2;
    if (bytes > 0 && bytes <= maxPcmFrameBytes) return ms;
  }
  return 5;
}

/// Parte `host:puerto` (o `[ipv6]:puerto`). `null` si no tiene esa forma.
(String, int)? splitHostPort(String text) {
  final t = text.trim();
  final idx = t.lastIndexOf(':');
  if (idx <= 0 || idx == t.length - 1) return null;
  final port = int.tryParse(t.substring(idx + 1));
  if (port == null || port <= 0 || port > 65535) return null;
  var host = t.substring(0, idx);
  if (host.startsWith('[') && host.endsWith(']')) {
    host = host.substring(1, host.length - 1);
  }
  return host.isEmpty ? null : (host, port);
}

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

  /// Conecta probando varias direcciones del anfitrión (`ip:puerto`) y se
  /// queda con la primera por la que se llega.
  ///
  /// [wait] es cuánto se insiste por caminos directos. Hace falta alargarlo
  /// cuando la dirección de este extremo le llega al anfitrión a mano (el
  /// código de respuesta): hay que seguir sondando mientras lo escanea.
  Future<void> connectAny(List<String> candidates, {Duration? wait});

  /// Añade direcciones del otro extremo por las que buscarlo. Mientras se
  /// espera invitado, el anfitrión las sonda: así se perfora el NAT sin
  /// servidor.
  Future<void> addPathHints(List<String> addrs);

  /// Pide al router que abra el puerto de la sala (UPnP). Devuelve la
  /// dirección pública resultante, o `null` si no es posible.
  Future<String?> openRouterPort();

  /// Milisegundos sin recibir nada del otro extremo, o `null` si aún no se
  /// ha recibido nada.
  int? rxIdleMs();

  /// `true` si el audio va por un relay y no directo.
  bool get relayed;

  /// Fija el servidor de encuentro (`host:puerto`) que coordina la
  /// perforación de NAT entre redes distintas. `null` o vacío lo quita.
  ///
  /// Devuelve `false` si no se pudo resolver: la sesión sigue, pero sólo con
  /// caminos directos.
  Future<bool> setRendezvous(String? hostPort);

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

  /// Dirección pública (`ip:puerto`) del socket de esta sesión, vía STUN.
  ///
  /// Va en la invitación como segunda candidata, para quien no esté en la
  /// misma red. Sólo se puede pedir antes del handshake. Devuelve `null` si
  /// STUN no responde.
  Future<String?> discoverPublicEndpoint();

  /// Descubre la dirección local (LAN) de este dispositivo.
  ///
  /// Es el endpoint que va en el QR cuando no hay IP pública alcanzable: dos
  /// dispositivos en la misma WiFi sólo se conectan por la LAN.
  Future<String?> discoverLocalEndpoint();

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

  /// Niveles y estado del audio. Es barato: se consulta varias veces por
  /// segundo para animar los medidores.
  AudioLevels audioLevels();

  /// Aviso de audio pendiente de mostrar (sin micrófono, sin altavoz…), y lo
  /// consume. `null` si no hay nada que decir.
  String? takeAudioWarning();

  Future<void> close();
  void dispose();
}

/// Motor nativo: habla con `libgravital_talk_ffi` (Rust real).
class FfiSessionEngine implements SessionEngine {
  FfiSessionEngine(this._bridge);

  @override
  Future<String?> discoverPublicEndpoint() async {
    // Se pregunta desde el socket de la sesión: el NAT asocia el puerto
    // público a ese socket, y preguntar desde otro daría un puerto que no es
    // el de la sala.
    final address = _alive.address;
    return Isolate.run(
      () => NativeBridge.tryLoad()?.runPublicAddr(address),
    );
  }

  @override
  Future<bool> setRendezvous(String? hostPort) async {
    final target = splitHostPort(hostPort ?? '');
    final address = _alive.address;
    final (host, port) = target ?? ('', 0);
    final res = await Isolate.run(
      () => NativeBridge.tryLoad()?.runSetRendezvous(address, host, port),
    );
    return res?.status == GsStatus.ok;
  }

  @override
  Future<String?> discoverLocalEndpoint() async =>
      _bridge.discoverLocalAddress();

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

  /// Handshake en vuelo en otro isolate, si lo hay.
  ///
  /// Mientras exista, ese isolate está dentro de la librería nativa usando la
  /// sesión: destruirla en ese momento es usar memoria liberada.
  Future<void>? _handshakeInFlight;

  /// `true` si el audio lo lleva la librería nativa (escritorio) y no el pump.
  bool _nativeAudio = false;
  int _frameMs = 10;
  String? _audioWarning;

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
    await _stopAudio();
    if (_handshakeInFlight == null) _session?.destroy();
    _frameMs = effectiveFrameMs(settings);
    _session = _bridge.createSession(
      sampleRate: settings.sampleRate,
      channels: settings.channels,
      frameDurationMs: _frameMs,
      maxBitrate: settings.maxBitrate,
      // Lo que viaja es PCM: esta librería no trae el codificador Opus, y
      // anunciar un codec que no se usa confundiría al otro extremo.
      codecPreferred: AudioCodec.pcm.code,
      jitterBufferMs: settings.jitterBufferMs,
      mtu: settings.mtu,
      bindPort: bindPort,
    );
  }

  /// Arranca el audio de la sesión recién conectada.
  ///
  /// Primero el nativo: es el de menor latencia y el único que hay en
  /// escritorio. Si la librería no lo trae (móvil), se usan los plugins.
  Future<void> _startAudio() async {
    final session = _session;
    if (session == null) return;
    await _stopAudio();
    try {
      if (session.audioStart()) {
        _nativeAudio = true;
        final why = session.audioProblems();
        final st = session.audioStats();
        if (!st.captureOk) {
          _audioWarning = 'No se pudo abrir el micrófono: puedes escuchar pero '
              'no hablar.';
        } else if (!st.playbackOk) {
          _audioWarning = 'No se pudo abrir el altavoz: no oirás al otro.';
        }
        if (why != null) debugPrint('gravital-talk: $why');
        return;
      }
      if (!AudioPump.supported) {
        _audioWarning = 'Esta plataforma no tiene audio: la conexión funciona '
            'pero no se oye nada.';
        return;
      }
      _pump = await AudioPump.start(
        session: session,
        sampleRate: _settings.sampleRate,
        channels: _settings.channels,
        frameDurationMs: _frameMs,
      );
    } catch (e) {
      // La sesión ya está conectada: que falle el audio se avisa, no se oculta
      // ni tumba la conexión.
      _audioWarning = 'No se pudo arrancar el audio: $e';
    }
  }

  Future<void> _stopAudio() async {
    if (_nativeAudio) {
      _nativeAudio = false;
      _session?.audioStop();
    }
    final pump = _pump;
    _pump = null;
    await pump?.stop();
  }

  @override
  Future<void> setSessionId(int id) async => _alive.setSessionId(id);

  @override
  Future<void> setRoomToken(String? token) async => _alive.setRoomToken(token);

  @override
  Future<void> connect(String host, int port) =>
      _runHandshake(_handshake(_alive.address, host, port, server: false));

  @override
  Future<void> connectAny(List<String> candidates, {Duration? wait}) {
    final address = _alive.address;
    final waitMs = wait?.inMilliseconds;
    return _runHandshake(Isolate.run(() {
      final bridge = NativeBridge.tryLoad();
      if (bridge == null) {
        return (
          status: GsStatus.internal,
          error: 'librería nativa no disponible',
        );
      }
      return bridge.runConnectAny(address, candidates, waitMs: waitMs);
    }));
  }

  @override
  Future<void> addPathHints(List<String> addrs) async =>
      _alive.addPathHints(addrs);

  @override
  Future<String?> openRouterPort() {
    final address = _alive.address;
    return Isolate.run(() => NativeBridge.tryLoad()?.runMapPort(address));
  }

  @override
  int? rxIdleMs() => _session?.rxIdleMs();

  @override
  bool get relayed {
    try {
      return _session?.isRelayed() ?? false;
    } on NativeException {
      return false;
    }
  }

  @override
  Future<void> accept(String host, int port) =>
      _runHandshake(_handshake(_alive.address, host, port, server: true));

  @override
  Future<void> acceptAny() => _runHandshake(_acceptAnyIsolate(_alive.address));

  Future<void> _runHandshake(
    Future<({int status, String? error})> handshake,
  ) async {
    final done = Completer<void>();
    _handshakeInFlight = done.future;
    final ({int status, String? error}) res;
    try {
      res = await handshake;
    } finally {
      _handshakeInFlight = null;
      done.complete();
    }
    _throwOnError(res);
    await _startAudio();
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
    _setAudioPtt(true);
  }

  @override
  Future<void> pttRelease() async {
    _setAudioPtt(false);
    _alive.pttRelease();
  }

  @override
  Future<void> setAudioPtt(bool on) async => _setAudioPtt(on);

  void _setAudioPtt(bool on) {
    if (_nativeAudio) {
      _session?.audioSetPtt(on);
    } else {
      _pump?.setPtt(on);
    }
  }

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
  double get micLevel => audioLevels().mic;

  @override
  AudioLevels audioLevels() {
    final session = _session;
    if (_nativeAudio && session != null) {
      try {
        final st = session.audioStats();
        return AudioLevels(
          mic: st.micLevel,
          rx: st.rxLevel,
          txFrames: st.txFrames,
          rxFrames: st.rxFrames,
          micOk: st.captureOk,
          speakerOk: st.playbackOk,
        );
      } on NativeException {
        return AudioLevels.silent;
      }
    }
    final pump = _pump;
    if (pump == null) return AudioLevels.silent;
    return AudioLevels(
      mic: pump.micLevel,
      rx: pump.rxLevel,
      txFrames: pump.txFrames,
      rxFrames: pump.rxFrames,
      micOk: pump.micOk,
      speakerOk: pump.speakerOk,
    );
  }

  @override
  String? takeAudioWarning() {
    final w = _audioWarning ?? _pump?.takeWarning();
    _audioWarning = null;
    return w;
  }

  @override
  Future<void> close() async {
    await _stopAudio();
    try {
      _alive.close();
    } on NativeException {
      // Sesión ya cerrada.
    } on StateError {
      return;
    }
    // Cerrar es lo que saca al handshake de su espera (un anfitrión puede
    // llevar minutos esperando invitado). Se le da margen para volver antes
    // de que `dispose` libere la sesión.
    final pending = _handshakeInFlight;
    if (pending != null) {
      await pending.timeout(const Duration(seconds: 3), onTimeout: () {});
    }
  }

  @override
  void dispose() {
    // El audio nativo se para en línea: sus hilos usan la sesión que se
    // destruye a continuación. El pump ya se paró en `close()`.
    if (_nativeAudio) {
      _nativeAudio = false;
      _session?.audioStop();
    }
    // Si el handshake sigue dentro de la librería, la sesión se abandona sin
    // liberar: una fuga pequeña es preferible a liberar memoria en uso.
    if (_handshakeInFlight == null) _session?.destroy();
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
  Future<void> connectAny(List<String> candidates, {Duration? wait}) =>
      _beginHandshake();

  @override
  Future<void> addPathHints(List<String> addrs) async {}

  @override
  Future<String?> openRouterPort() async => null;

  @override
  int? rxIdleMs() => null;

  @override
  bool get relayed => false;

  @override
  Future<bool> setRendezvous(String? hostPort) async => true;

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

  @override
  AudioLevels audioLevels() => AudioLevels(
        mic: _micLevel,
        rx: _peerPtt && !_ptt ? 0.2 + _rng.nextDouble() * 0.5 : 0,
      );

  @override
  String? takeAudioWarning() => null;

  // El motor demo no tiene red real. Devuelve un endpoint ficticio para que la
  // UI se pueda recorrer de punta a punta: sin él, la pantalla de crear sala
  // no puede mostrar QR y no hay forma de probar el flujo.
  //
  // Es deliberadamente obvio que es falso: la IP es de documentación. Quien lo
  // ve en la pantalla no puede confundirlo con una dirección real.
  @override
  Future<String?> discoverPublicEndpoint() async => null;

  // IP del rango de documentación (RFC 5737): deja recorrer el flujo de crear
  // sala sin que nadie pueda confundirla con una dirección real.
  @override
  Future<String?> discoverLocalEndpoint() async => '192.0.2.10';

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
