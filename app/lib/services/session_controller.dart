import 'dart:async';
import 'dart:io';
import 'dart:math';

import 'package:flutter/foundation.dart';

import '../models/connection.dart';
import '../models/reachability.dart';
import '../models/session.dart';
import 'background_session.dart';
import 'engine.dart';
import 'event_log.dart';
import 'native_bridge.dart';
import 'pairing_uri.dart';
import 'room_api.dart';
import 'room_events.dart';
import 'settings_store.dart';

/// Lo que se sabe del otro extremo al conectar: con esto se guarda como
/// contacto.
class PairedEvent {
  const PairedEvent({
    required this.isHost,
    required this.token,
    required this.room,
    this.peerName = '',
    this.peerDevice = '',
    this.peerHost,
  });

  final bool isHost;
  final String token;
  final String room;
  final String peerName;
  final String peerDevice;

  /// IP de red local del otro, si se conoce (el invitado conoce la del
  /// anfitrión por la invitación).
  final String? peerHost;
}

/// Orquesta el ciclo de vida de la conexión (servidor o P2P) sobre un motor.
class SessionController extends ChangeNotifier {
  SessionController(
    this._engine,
    this._roomApi,
    this._log,
    this._store,
    this._roomEvents,
  );

  final SessionEngine _engine;
  final RoomControlApi _roomApi;
  final EventLog _log;
  final SettingsStore _store;

  /// Estado de los participantes de la sala (stream `WatchRoom` del relay).
  ///
  /// Es el último parámetro y puede ser `null` para no romper los tests, que
  /// construyen el controller sin él. Quien no lo pase verá la lista de
  /// participantes vacía siempre: el stream no se conecta con nadie.
  final RoomEvents? _roomEvents;

  /// Nombre e identificador de este dispositivo: viajan en la invitación
  /// para que quien entre pueda guardarlo como contacto.
  String deviceName = '';
  String deviceId = '';

  /// Se llama cada vez que una sala conecta.
  void Function(PairedEvent event)? onPaired;

  /// Invitación con la que se entró en la sala en curso, si fue así.
  PairingUri? _joinedWith;

  /// IP de este dispositivo en la red local, o `null` sin red.
  Future<String?> localIp() async {
    try {
      final ip = await _engine.discoverLocalEndpoint();
      return ip == null || ip.isEmpty ? null : ip;
    } catch (_) {
      return null;
    }
  }

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

  /// Endpoint que el anfitrión comparte en el QR (`ip:puerto`).
  String _publicEndpoint = '';

  /// Dirección pública de la sala vista por STUN, o `null` si no se obtuvo.
  String? _stunEndpoint;

  /// `true` si la sala en curso tiene servidor de encuentro operativo.
  bool _rendezvousActive = false;

  /// `true` si el router abrió el puerto de la sala (UPnP).
  bool _portOpened = false;

  /// `true` si a esta sala se puede entrar desde otra red sin más pasos: el
  /// router abrió el puerto, o hay un servidor que presenta a los dos.
  bool get reachableFromOtherNetworks => _rendezvousActive || _portOpened;

  /// Respuesta de este dispositivo a la invitación, mientras se une.
  PairingAnswer? _answer;

  /// Código que el invitado enseña al anfitrión cuando están en redes
  /// distintas, o `null` si no se está uniendo a ninguna sala.
  String? get answerUri => _answer?.toUri();

  bool _stalled = false;
  bool _relayed = false;

  /// `true` si hace varios segundos que no llega nada del otro extremo: un
  /// cambio de red, o falta de cobertura. La sesión sigue intentando
  /// recuperarse sola.
  bool get connectionStalled => _stalled;

  /// `true` si el audio pasa por un servidor intermedio y no directo.
  bool get relayed => _relayed;

  int _peerCount = 0;

  /// Dispositivos en la sala contando este, visto desde el anfitrión. Un
  /// invitado sólo conoce al anfitrión, así que no puede dar el total.
  int get roomSize => _peerCount + 1;

  /// `true` si esta sesión es la anfitriona de la sala y puede invitar a más.
  bool get canInvite =>
      _state.isLive && _role == ConnectionRole.host && pairingUri != null;

  /// Tasa de bits a la que se está enviando el audio, o 0 si no aplica.
  int get audioBitrate => _state.isLive ? _engine.audioBitrate() : 0;

  /// Aviso no bloqueante pendiente de mostrar (p. ej. "sin micrófono").
  String? _notice;
  Timer? _levelTicker;

  /// Niveles de audio para los medidores.
  ///
  /// Va aparte del `notifyListeners` general a propósito: cambia varias veces
  /// por segundo y sólo debe repintar los medidores, no la pantalla entera.
  final ValueNotifier<AudioLevels> levels = ValueNotifier(AudioLevels.silent);
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

  /// Aviso no bloqueante (la sesión sigue), o `null`.
  String? get notice => _notice;

  void clearNotice() {
    _notice = null;
    notifyListeners();
  }

  /// `true` mientras el anfitrión espera a que alguien entre en su sala.
  bool get waitingForGuest =>
      _busy && _role == ConnectionRole.host && !_state.isLive;

  /// Diagnostica si este dispositivo puede recibir invitados de fuera.
  Future<NetworkReachability?> diagnoseReachability() =>
      _engine.diagnoseReachability();

  /// Endpoint que el anfitrión comparte (IP pública + puerto).
  String get publicEndpoint => _publicEndpoint;
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
    unawaited(BackgroundSession.setBubbleEnabled(_settings.pttBubble));
    notifyListeners();
  }

  void updateSettings(AppSettings s) {
    _settings = s;
    // La calidad y la reducción de ruido se pueden cambiar a media
    // conversación.
    _engine.applyAudioSettings(s);
    unawaited(BackgroundSession.setBubbleEnabled(s.pttBubble));
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
  /// Publica un error de validación local sin tocar la sesión.
  ///
  /// Antes una validación local no tenía dónde mostrarse si la pantalla aún no
  /// había llamado a `_guard`, que es quien fija `_error`.
  void reportError(String message) {
    _error = message;
    notifyListeners();
  }

  /// Descarta el error visible.
  ///
  /// Sin esto, un error se quedaba en pantalla hasta el siguiente intento de
  /// conexión: el usuario no podía ignorarlo y volver a ajustar los campos.
  void clearError() {
    _error = null;
    notifyListeners();
  }

  /// Conecta el stream de participantes de la sala [code].
  ///
  /// El relay emite `PeerJoined`/`PeerLeft`/`FloorGranted`/`FloorReleased` por
  /// `WatchRoom`, pero nadie lo pedía: la tarjeta de participantes se quedaba
  /// siempre en "tú" aunque hubiera más gente. Si el relay no tiene gRPC, el
  /// stream falla y se silencia (el plano de control funciona igual por REST).
  Future<void> _observeRoom(String host, String code) async {
    final events = _roomEvents;
    if (events == null) return;
    try {
      await events.watch(code, host: host);
    } catch (e) {
      // Un fallo del stream no debe tumbar la conexión de audio: la sesión es
      // lo importante. Se registra y se sigue.
      _log.warn('Sin observación de participantes: $e');
    }
  }

  /// Deja de observar la sala (al desconectar o reiniciar).
  Future<void> _stopObserving() async {
    try {
      await _roomEvents?.stop();
    } catch (_) {
      // Silenciar: parar la observación nunca debe romper una desconexión.
    }
  }

  /// Crea una sala **sin relay**: este dispositivo es el punto de encuentro.
  ///
  /// Es el flujo que debe funcionar por defecto, y el que estaba roto. Antes
  /// "crear sala" exigía configurar la dirección de un servidor
  /// (`Configura la dirección del servidor`), lo que es absurdo: quien crea la
  /// sala ES el servidor. El resultado era que no se podía crear ninguna sala
  /// sin desplegar un relay primero.
  ///
  /// Lo que hace, en orden:
  /// 1. Dedica un puerto local para Gravital Talk.
  /// 2. Genera un código de sala y un secreto (token PSK de Noise).
  /// 3. Descubre su IP pública por STUN, que es lo que va en el QR.
  /// 4. Se queda aceptando al primer cliente que llegue (cualquier IP).
  ///
  /// El anfitrión es **un participante más**: administra la sala, pero habla y
  /// escucha como los demás. No es un servidor del que dependen los otros para
  /// existir.
  ///
  /// Devuelve `false` y fija el error si no se puede binder el puerto.
  Future<bool> createRoom() => _guard(() async {
    // Puerto 0: el sistema elige uno libre y el QR publica el real. Un puerto
    // fijo falla en cuanto otra app (u otra sala) lo tiene cogido.
    await _engine.createSession(_settings);

    // El puerto real, no el pedido. Si `bindPort` es 0 el sistema elige uno
    // efímero, y el QR tiene que publicar ESE. Antes no se leía nunca, así que
    // el QR salía con `udp=0` y ningún invitado podía conectar.
    _localPort = await _engine.localPort();
    if (_localPort == 0) {
      throw StateError('No se pudo abrir un puerto para la sala.');
    }

    // El QR lleva la dirección de este dispositivo en la red local. No la
    // pública: la que da STUN es la del router, y sin reenvío de puertos no
    // entra nadie por ella (ni siquiera desde la misma wifi en muchos routers).
    final lanIp = await _engine.discoverLocalEndpoint();
    if (lanIp == null || lanIp.isEmpty) {
      throw StateError(
        'No hay red. Conéctate a una wifi para que puedan unirse a tu sala.',
      );
    }
    _publicEndpoint = lanIp.contains(':') ? '[$lanIp]:$_localPort' : '$lanIp:$_localPort';

    // El token es el secreto de la sala. Se genera aquí, no se configura: un
    // usuario no debería tener que inventarse un secreto criptográfico.
    final token = _generateRoomToken();
    await _engine.setRoomToken(token);

    // Para invitados de otra red: la dirección pública de este socket y, si
    // hay servidor de encuentro, el registro en él. Ninguno de los dos es
    // imprescindible —la sala funciona en la red local sin ellos—, así que un
    // fallo aquí no impide crearla.
    _rendezvousActive = _settings.rendezvous.isNotEmpty &&
        await _engine.setRendezvous(_settings.rendezvous);
    if (_settings.rendezvous.isNotEmpty && !_rendezvousActive) {
      _log.warn('No se pudo usar el servidor de encuentro '
          '${_settings.rendezvous}: sólo se podrá entrar desde esta red.');
    }
    // En paralelo: la dirección pública que ve STUN y la que da el router si
    // acepta abrir el puerto. La segunda es mejor —por ella se entra desde
    // cualquier red sin más— así que tiene preferencia.
    final found = await Future.wait([
      _engine
          .discoverPublicEndpoint()
          // Más que el tope del motor (3 s) a propósito: hay que esperar a
          // que suelte el socket antes de empezar a aceptar por él.
          .timeout(const Duration(seconds: 6), onTimeout: () => null),
      _engine
          .openRouterPort()
          .timeout(const Duration(seconds: 8), onTimeout: () => null),
    ]);
    _portOpened = found[1] != null;
    _stunEndpoint = found[1] ?? found[0];
    if (_portOpened) {
      _log.success('El router abrió el puerto: sala alcanzable en ${found[1]}.');
    }

    final code = _generateRoomCode();
    _roomCode = code;
    _sessionId = _randomSessionId();
    _engine.setSessionId(_sessionId);
    _updateServerProfile(roomCode: code);
    _server = _server.copyWith(roomCode: code, token: token);
    _store.saveServerProfile(_server);

    _mode = ConnectionMode.server;
    _role = ConnectionRole.host;
    _startTicker();
    notifyListeners();

    _log.success(
      'Sala $code lista en $_publicEndpoint (session_id=$_sessionId). '
      'Esperando peers…',
    );

    // Acepta al primer cliente de cualquier IP: es el modo emparejamiento.
    await _engine.acceptAny();
    _onConnected('Alguien se unió a la sala $code');
  });

  /// URI de emparejamiento de la sala creada: lo que codifica el QR.
  ///
  /// `null` si no hay sala que compartir.
  PairingUri? get pairingUri {
    final code = _roomCode;
    if (code == null || code.isEmpty || _publicEndpoint.isEmpty) return null;
    if (_role != ConnectionRole.host) return null;
    final idx = _publicEndpoint.lastIndexOf(':');
    var host = _publicEndpoint.substring(0, idx);
    if (host.startsWith('[')) host = host.substring(1, host.length - 1);
    return PairingUri(
      host: host,
      room: code,
      udpPort: int.tryParse(_publicEndpoint.substring(idx + 1)),
      token: _server.token,
      publicEndpoint: _stunEndpoint,
      rendezvous: _rendezvousActive ? _settings.rendezvous : null,
      name: deviceName,
      device: deviceId,
    );
  }

  /// Se une a partir de lo escaneado, pegado o recibido por enlace.
  ///
  /// Es la única puerta de entrada del invitado: QR, enlace y texto pegado
  /// acaban aquí, así que los tres se comportan igual.
  Future<bool> joinFromUri(String raw) {
    final parsed = PairingUri.tryParse(raw);
    if (parsed == null) {
      _fail('Eso no es una invitación de Gravital Talk.');
      return Future.value(false);
    }
    if (!parsed.isComplete) {
      _fail(
        'La invitación sólo trae el código de sala, no la dirección. Pide al '
        'anfitrión el QR o el enlace completo.',
      );
      return Future.value(false);
    }
    updateServerProfile(_server.copyWith(
      host: parsed.host,
      roomCode: parsed.room,
      udpPort: parsed.udpPort ?? _server.udpPort,
      token: parsed.token ?? '',
    ));
    _joinedWith = parsed;
    return _joinCandidates(
      parsed.candidates(_server.udpPort),
      rendezvous: parsed.rendezvous,
    );
  }

  /// El anfitrión recibe la respuesta del invitado (escaneada, pegada o por
  /// enlace) y empieza a sondar sus direcciones.
  ///
  /// Devuelve `false` si aquello no era una respuesta para esta sala.
  /// Aplica la respuesta de un invitado que llegó sola (por la red local o
  /// por un buzón), sin molestar si no viene a cuento. Vale también con la
  /// sala ya en marcha: así entra un tercero desde otra red.
  Future<bool> applyAnswer(String raw) async {
    final answer = PairingAnswer.tryParse(raw);
    if (answer == null ||
        _role != ConnectionRole.host ||
        answer.room != _roomCode) {
      return false;
    }
    try {
      await _engine.addPathHints(answer.addresses);
      return true;
    } catch (_) {
      return false;
    }
  }

  Future<bool> acceptAnswer(String raw) async {
    final answer = PairingAnswer.tryParse(raw);
    if (answer == null) {
      reportError('Ese código no es una respuesta de Gravital Talk.');
      return false;
    }
    if (_role != ConnectionRole.host || _state.isLive) {
      reportError('La respuesta se escanea desde la pantalla de tu sala, '
          'mientras esperas a que se unan.');
      return false;
    }
    if (answer.room != _roomCode) {
      reportError('Esa respuesta es de otra sala (${answer.room}).');
      return false;
    }
    try {
      await _engine.addPathHints(answer.addresses);
    } catch (e) {
      reportError('No se pudo usar la respuesta: $e');
      return false;
    }
    _log.info('Respuesta del invitado recibida: buscando camino…');
    return true;
  }

  /// Entra en la sala probando todas las direcciones de la invitación.
  Future<bool> _joinCandidates(List<String> candidates, {String? rendezvous}) {
    return _guard(() async {
      final profile = _server;
      await _engine.createSession(_settings);
      if (profile.token.isNotEmpty) {
        await _engine.setRoomToken(profile.token);
      }
      // El servidor de encuentro es el del anfitrión, no el de los ajustes de
      // este dispositivo: los dos tienen que registrarse en el mismo.
      final hasServer =
          rendezvous != null && await _engine.setRendezvous(rendezvous);
      _roomCode = profile.roomCode;
      _mode = ConnectionMode.server;
      _role = ConnectionRole.join;

      // La respuesta a la invitación: por dónde se llega a ESTE dispositivo.
      // Si los dos están tras un NAT, es lo que el anfitrión necesita para
      // abrir el suyo hacia aquí.
      final public = await _engine
          .discoverPublicEndpoint()
          .timeout(const Duration(seconds: 6), onTimeout: () => null);
      final lanIp = await _engine.discoverLocalEndpoint();
      final port = await _engine.localPort();
      final own = <String>[
        ?public,
        if (lanIp != null && lanIp.isNotEmpty) '$lanIp:$port',
      ];
      _answer = own.isEmpty
          ? null
          : PairingAnswer(room: profile.roomCode, addresses: own);
      _startTicker();
      notifyListeners();

      await _engine.connectAny(
        candidates,
        // Sin servidor que presente, la dirección de este dispositivo llega al
        // anfitrión cuando escanea la respuesta: hay que darle tiempo.
        wait: hasServer ? null : const Duration(seconds: 90),
      );
      _answer = null;
      _onConnected('Conectado a la sala ${profile.roomCode}');
    });
  }

  /// Se une a una sala conectándose directo al endpoint del anfitrión.
  ///
  /// `host:port` viene del QR. Si el QR sólo traía el código, se usa el host
  /// configurado a mano.
  Future<bool> joinEndpoint(String host, int port) {
    // La validación va ANTES de `_guard`. Dentro, un `return` temprano no
    // aborta: el cierre completa con normalidad y `_guard` devuelve `true`, así
    // que la app creería que se conectó. Es el mismo patrón que usa
    // `hostServer`, y por algo está fuera.
    if (_server.roomCode.isEmpty) {
      _fail('Indica el código de la sala');
      return Future.value(false);
    }
    return _guard(() async {
      final profile = _server;
      final code = profile.roomCode;

      await _engine.createSession(_settings);
      if (profile.token.isNotEmpty) {
        await _engine.setRoomToken(profile.token);
      }
      _roomCode = code;
      _mode = ConnectionMode.server;
      _role = ConnectionRole.join;
      _startTicker();
      notifyListeners();

      await _engine.connect(host, port);
      _onConnected('Conectado a la sala $code');
    });
  }

  /// Genera un código de sala legible (`XXXX-NNNN`).
  ///
  /// Formato deliberado sin caracteres ambiguos: ni I ni O ni 0 ni 1, para que
  /// se pueda leer en voz alta sin que nadie se confunda.
  String _generateRoomCode() {
    const letters = 'ABCDEFGHJKLMNPQRSTUVWXYZ';
    const digits = '23456789';
    final rng = Random();
    String pick(String from, int n) =>
        List.generate(n, (_) => from[rng.nextInt(from.length)]).join();
    return '${pick(letters, 4)}-${pick(digits, 4)}';
  }

  /// Secreto de la sala: 6 palabras-base de un alfabeto amplio.
  ///
  /// Suficiente entropía para que no se pueda adivinar, y al ser el PSK de
  /// Noise basta con conocerlo. Se muestra en el QR, así que el usuario no tiene
  /// que recordarlo.
  String _generateRoomToken() {
    const alphabet =
        'abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789';
    final rng = Random();
    return List.generate(22, (_) => alphabet[rng.nextInt(alphabet.length)])
        .join();
  }

  int _randomSessionId() => Random().nextInt(0x7FFFFFFF) + 1;

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
          token: profile.token,
        );
        code = room.code;
      }
      _roomCode = code;
      _sessionId = sid;
      _engine.setSessionId(sid);
      // Observar la sala recién creada: sin esto la lista de participantes
      // nunca recibe eventos, aunque el relay los esté emitiendo.
      await _observeRoom(profile.host, code);
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
        token: profile.token,
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

  bool _paused = false;

  /// `true` si el usuario pausó la sala: sigue dentro, pero ni oye ni habla
  /// hasta que la reanude.
  bool get paused => _paused;

  /// Pausa o reanuda la sala sin salir de ella.
  Future<void> setPaused(bool on) async {
    if (!isLive || _paused == on) return;
    if (on) await pttUp();
    _paused = on;
    _engine.setListening(!on);
    _log.info(on ? 'Sala en pausa' : 'Sala reanudada');
    _syncNotification();
    notifyListeners();
  }

  Future<void> pttDown() async {
    if (!isLive || _ptt || _paused) return;
    _ptt = true;
    _turnTone(end: false);
    _syncNotification();
    await _engine.pttPress();
    _log.info('PTT: transmitiendo');
    notifyListeners();
  }

  void _syncNotification() =>
      unawaited(BackgroundSession.update(talking: _ptt, paused: _paused));

  void _turnTone({required bool end}) {
    if (_settings.turnTones && _state.isLive && !_paused) {
      _engine.playTurnTone(end: end);
    }
  }

  /// Último estado del otro para el que ya sonó el tono.
  bool _peerTalking = false;
  bool _peerPolling = false;

  /// Suena el tono cuando el otro toma o suelta la palabra. Va con el ritmo
  /// de los medidores, no con el del estado general: medio segundo de retraso
  /// en un aviso de "empieza a hablar" lo deja sin sentido.
  Future<void> _watchPeerTurn() async {
    if (_peerPolling) return;
    _peerPolling = true;
    try {
      final talking = await _engine.peerPttActive();
      if (talking != _peerTalking) {
        _peerTalking = talking;
        _turnTone(end: !talking);
        unawaited(BackgroundSession.peerTalking(talking));
      }
    } catch (_) {
      // Motor entre dos estados: el siguiente tick reintenta.
    } finally {
      _peerPolling = false;
    }
  }

  Future<void> pttUp() async {
    if (!_ptt) return;
    _ptt = false;
    await _engine.pttRelease();
    _turnTone(end: true);
    _syncNotification();
    notifyListeners();
  }

  // ── Ciclo de vida ───────────────────────────────────────────────────────

  Future<void> disconnect() async {
    _ticker?.cancel();
    _ticker = null;
    _levelTicker?.cancel();
    _levelTicker = null;
    levels.value = AudioLevels.silent;
    _notice = null;
    _publicEndpoint = '';
    _stunEndpoint = null;
    _rendezvousActive = false;
    _portOpened = false;
    _answer = null;
    _stalled = false;
    _relayed = false;
    _peerCount = 0;
    _paused = false;
    _peerTalking = false;
    _joinedWith = null;
    unawaited(BackgroundSession.stop());
    if (_busy) _cancelled = true;
    // Dejar de observar la sala: si no, el stream sigue abierto tras colgar y
    // la siguiente conexión se encuentra eventos de la sala anterior.
    await _stopObserving();
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

  Future<void> reset() async {
    await _stopObserving();
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

  /// `true` si el usuario canceló la operación en curso.
  ///
  /// Cancelar hace que el handshake pendiente vuelva con error; sin esta marca
  /// ese error se mostraría como un fallo, cuando es justo lo que se pidió.
  bool _cancelled = false;

  Future<bool> _guard(Future<void> Function() action) async {
    _busy = true;
    _error = null;
    _cancelled = false;
    notifyListeners();
    try {
      await action();
      return !_cancelled;
    } on NativeException catch (e) {
      if (_cancelled) return false;
      _fail(_describeNative(e));
      return false;
    } on RoomApiException catch (e) {
      if (_cancelled) return false;
      _fail(e.message);
      return false;
    } on StateError catch (e) {
      if (_cancelled) return false;
      _fail(e.message);
      return false;
    } catch (e) {
      if (_cancelled) return false;
      _fail('$e');
      return false;
    } finally {
      _busy = false;
      notifyListeners();
    }
  }

  /// Traduce un fallo del motor a algo que el usuario pueda arreglar.
  String _describeNative(NativeException e) {
    if (e.status == GsStatus.timeout ||
        e.status == GsStatus.handshake ||
        e.message.contains('timeout')) {
      return _role == ConnectionRole.join
          ? 'No se pudo llegar a la sala. Comprueba que sigue abierta y '
              'vuelve a abrir la invitación.'
          : 'Nadie llegó a conectarse. Vuelve a crear la sala.';
    }
    return 'No se pudo conectar: ${e.message}';
  }

  void _onConnected(String message) {
    _state = SessionState.active;
    _connectedAt = DateTime.now();
    _log.success(message);
    _takeAudioWarning();
    _startLevelTicker();
    // A partir de aquí la sala debe aguantar con la app oculta.
    unawaited(BackgroundSession.start(_roomCode ?? ''));
    final isHost = _role == ConnectionRole.host;
    final invite = isHost ? null : _joinedWith;
    onPaired?.call(PairedEvent(
      isHost: isHost,
      token: _server.token,
      room: _roomCode ?? invite?.room ?? '',
      peerName: invite?.name ?? '',
      peerDevice: invite?.device ?? '',
      peerHost: invite == null || invite.host.isEmpty ? null : invite.host,
    ));
    notifyListeners();
  }

  /// Recoge el aviso de audio del motor, si lo hay, y lo publica.
  void _takeAudioWarning() {
    final warning = _engine.takeAudioWarning();
    if (warning == null) return;
    _notice = warning;
    _log.warn(warning);
  }

  /// Refresca los niveles de audio ~12 veces por segundo.
  void _startLevelTicker() {
    _levelTicker?.cancel();
    _levelTicker = Timer.periodic(const Duration(milliseconds: 80), (_) {
      if (!_state.isLive) return;
      unawaited(_watchPeerTurn());
      try {
        levels.value = _engine.audioLevels();
      } catch (_) {
        // Motor entre dos estados: el siguiente tick reintenta.
      }
    });
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
        if (_notice == null) _takeAudioWarning();
        if (st.isLive) {
          final idle = _engine.rxIdleMs();
          _stalled = idle != null && idle > 4000;
          _relayed = _engine.relayed;
          _peerCount = _engine.peerCount;
        } else {
          _stalled = false;
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
    _levelTicker?.cancel();
    levels.dispose();
    _engine.dispose();
    _roomApi.dispose();
    super.dispose();
  }
}
