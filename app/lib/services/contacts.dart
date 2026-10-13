import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:flutter/widgets.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../models/connection.dart';
import 'background_session.dart';
import 'knock_codec.dart';
import 'mailbox.dart';
import 'session_controller.dart';

/// Un dispositivo con el que ya se habló y al que se puede volver a llamar.
@immutable
class Contact {
  const Contact({
    required this.id,
    required this.secret,
    required this.name,
    this.device = '',
    this.custom = false,
    this.note = '',
    this.muted = false,
    this.autoAnswer = false,
    this.lastLocal,
    this.lastPublic,
    this.lastSeen = 0,
  });

  factory Contact.fromJson(Map<String, dynamic> j) => Contact(
        id: j['id'] as String,
        secret: j['secret'] as String,
        name: j['name'] as String? ?? 'Contacto',
        device: j['device'] as String? ?? '',
        custom: j['custom'] as bool? ?? false,
        note: j['note'] as String? ?? '',
        muted: j['muted'] as bool? ?? false,
        autoAnswer: j['autoAnswer'] as bool? ?? false,
        lastLocal: j['lastLocal'] as String?,
        lastPublic: j['lastPublic'] as String?,
        lastSeen: j['lastSeen'] as int? ?? 0,
      );

  /// Id público derivado del secreto (hex): viaja en claro en cada llamada.
  final String id;

  /// Secreto compartido: el token de la sala en la que se conocieron.
  final String secret;
  final String name;

  /// Identificador estable del otro dispositivo, si lo ha dicho. Sirve para
  /// no duplicar el contacto al volver a emparejar por código.
  final String device;

  /// `true` si el nombre lo puso el usuario: ya no se pisa con el que diga el
  /// otro dispositivo.
  final bool custom;

  /// Nota libre del usuario sobre el contacto. No sale de este dispositivo.
  final String note;

  /// `true` si sus llamadas no deben avisar.
  final bool muted;

  /// `true` si sus llamadas se contestan solas, como un walkie-talkie.
  final bool autoAnswer;

  /// Última IP de red local y pública conocidas del contacto.
  final String? lastLocal;
  final String? lastPublic;

  /// Última vez que se habló o se supo de él (ms desde época).
  final int lastSeen;

  Contact copyWith({
    String? name,
    String? device,
    bool? custom,
    String? note,
    bool? muted,
    bool? autoAnswer,
    String? lastLocal,
    String? lastPublic,
    int? lastSeen,
  }) =>
      Contact(
        id: id,
        secret: secret,
        name: name ?? this.name,
        device: device ?? this.device,
        custom: custom ?? this.custom,
        note: note ?? this.note,
        muted: muted ?? this.muted,
        autoAnswer: autoAnswer ?? this.autoAnswer,
        lastLocal: lastLocal ?? this.lastLocal,
        lastPublic: lastPublic ?? this.lastPublic,
        lastSeen: lastSeen ?? this.lastSeen,
      );

  Map<String, dynamic> toJson() => {
        'id': id,
        'secret': secret,
        'name': name,
        'device': device,
        'custom': custom,
        if (note.isNotEmpty) 'note': note,
        if (muted) 'muted': true,
        if (autoAnswer) 'autoAnswer': true,
        if (lastLocal != null) 'lastLocal': lastLocal,
        if (lastPublic != null) 'lastPublic': lastPublic,
        'lastSeen': lastSeen,
      };
}

/// La invitación: el único enlace que hay que compartir.
///
/// ```text
/// gravital-talk://i?v=2&s=SECRETO&n=Ana&d=ID&ip=192.168.1.10
/// ```
///
/// Lleva el secreto que compartirán los dos. Quien lo abre queda guardado como
/// contacto de quien lo envió, y al revés; si quien invitó sigue esperando,
/// además entran a hablar sin más pasos. Hay que pasarlo por un canal de
/// confianza.
///
/// Se sigue entendiendo la forma anterior (`gravital-talk://contact?…&dev=`).
@immutable
class ContactLink {
  const ContactLink({
    required this.secret,
    this.name = '',
    this.device = '',
    this.ip,
  });

  final String secret;
  final String name;
  final String device;

  /// IP de red local de quien invita, para saludarle si están en la misma.
  final String? ip;

  /// `null` si `raw` no es un enlace de contacto de Gravital Talk.
  static ContactLink? tryParse(String raw) {
    final uri = Uri.tryParse(raw.trim());
    if (uri == null ||
        uri.scheme != 'gravital-talk' ||
        (uri.host != 'i' && uri.host != 'contact')) {
      return null;
    }
    final q = uri.queryParameters;
    final secret = (q['s'] ?? '').trim();
    // Un secreto corto se adivina: no se acepta.
    if (secret.length < 16) return null;
    final ip = (q['ip'] ?? '').trim();
    return ContactLink(
      secret: secret,
      name: (q['n'] ?? '').trim(),
      device: (q['d'] ?? q['dev'] ?? '').trim(),
      ip: ip.isEmpty ? null : ip,
    );
  }

  String toUri() => Uri(
        scheme: 'gravital-talk',
        host: 'i',
        queryParameters: {
          'v': '2',
          's': secret,
          if (name.isNotEmpty) 'n': name,
          if (device.isNotEmpty) 'd': device,
          'ip': ?ip,
        },
      ).toString();
}

/// Solicitud de un contacto para hablar, pendiente de aceptar.
@immutable
class IncomingCall {
  const IncomingCall({
    required this.contact,
    required this.invite,
    this.auto = false,
  });

  final Contact contact;

  /// `true` si se entra sin preguntar: es la llamada que sigue a abrir una
  /// invitación, y quien la abrió ya dijo que quería hablar.
  final bool auto;

  /// Invitación a la sala que abrió quien llama.
  final String invite;
}

/// Contactos y llamadas entre ellos.
///
/// Dos dispositivos que se emparejan guardan el secreto de aquella sala. Con
/// él, volver a hablar no pide código: quien llama abre una sala nueva y le
/// manda la invitación cifrada al otro, directamente a su última dirección
/// conocida y a la red local. El otro la ve como una solicitud y la acepta.
///
/// No hay servidor en medio, y eso marca el alcance: la solicitud llega si el
/// otro dispositivo tiene la app en marcha (abierta o en segundo plano) y es
/// alcanzable —misma red, o una dirección pública que acepte entrada—. Para
/// el resto de casos sigue estando la invitación por enlace.
class ContactsService extends ChangeNotifier {
  ContactsService(
    this._controller, {
    this.codec,
    this.prefs,
    this.transport,
    this.mailbox,
  });

  /// Buzón por el que también viajan llamadas e invitaciones, para cuando
  /// los dos dispositivos no están en la misma red. `null` = sólo red local.
  final Mailbox? mailbox;
  StreamSubscription<Uint8List>? _mail;
  final Set<String> _subscribed = {};

  /// Prueba: no usar la red local, para comprobar que el buzón basta.
  static final _lanOff = Platform.environment['GT_NO_LAN'] == '1';

  /// Servidores de buzón configurados (`host:puerto`). Sin configurar, los
  /// públicos por defecto; una cadena vacía guardada los desactiva.
  static List<String> mailboxServers(SharedPreferences prefs) {
    final saved = prefs.getString('net.mailboxes');
    if (saved == null) return MqttMailbox.defaultServers;
    return [
      for (final s in saved.split(','))
        if (s.trim().isNotEmpty) s.trim(),
    ];
  }

  static Future<void> setMailboxServers(
    SharedPreferences prefs,
    List<String> servers,
  ) =>
      prefs.setString('net.mailboxes', servers.join(','));

  static String _topic(String contactId) => 'gt/1/$contactId';

  /// Escucha el buzón de cada contacto y deja de escuchar el de los que ya
  /// no están.
  void _syncMailbox() {
    final box = mailbox;
    if (box == null) return;
    final wanted = {for (final c in _contacts) c.id, ?_roomId};
    for (final id in wanted.difference(_subscribed)) {
      box.subscribe(_topic(id));
    }
    for (final id in _subscribed.difference(wanted)) {
      box.unsubscribe(_topic(id));
    }
    _subscribed
      ..clear()
      ..addAll(wanted);
  }

  /// Quien cifra las llamadas; `null` sin librería nativa.
  final KnockCodec? codec;
  final SharedPreferences? prefs;

  /// Sustituye al envío por UDP en las pruebas: recibe el datagrama sellado y
  /// la dirección de destino.
  @visibleForTesting
  final void Function(Uint8List datagram, String address)? transport;

  /// Puerto UDP fijo en el que cada dispositivo escucha a sus contactos.
  static const port = 47800;

  static const _contactsKey = 'contacts.v1';
  static const _deviceIdKey = 'device.id';
  static const _deviceNameKey = 'device.name';
  static const _backgroundKey = 'contacts.background';
  static const _pendingKey = 'contacts.pendingLink';

  /// Nombre de un contacto del que aún no se sabe nada: quien abra el enlace
  /// lo sustituye por el suyo al saludar.
  static const pendingName = 'Contacto nuevo';

  /// Margen para la fecha de una llamada: tolera relojes desajustados, pero
  /// una capturada ayer no vuelve a hacer sonar el teléfono.
  static const _maxAge = Duration(minutes: 5);
  static const _callTimeout = Duration(seconds: 45);

  final SessionController _controller;

  RawDatagramSocket? _socket;
  List<Contact> _contacts = const [];
  String _deviceId = '';
  String _deviceName = '';
  bool _background = true;

  /// Contacto de la llamada en curso (hecha o aceptada): al conectar no se
  /// crea uno nuevo, se actualiza ese.
  String? _activeId;
  Timer? _callTimer;
  final Set<String> _dismissed = {};

  /// Invitación que este dispositivo está enseñando ahora (id de contacto).
  String? _invitingId;

  /// Dispositivos que ya contestaron a esa invitación: cada uno entra una vez.
  final Set<String> _greeted = {};

  /// Contacto recién añadido por enlace, del que se acepta la llamada sin
  /// preguntar durante un rato.
  String? _autoAcceptId;
  DateTime _autoAcceptUntil = DateTime.fromMillisecondsSinceEpoch(0);
  Timer? _answerTimer;

  /// Contacto al que hay que llamar ya: abrió la invitación mientras se le
  /// esperaba. Lo consume la pantalla de invitar.
  final ValueNotifier<Contact?> autoCall = ValueNotifier(null);

  /// Solicitud pendiente de aceptar o rechazar, o `null`.
  final ValueNotifier<IncomingCall?> incoming = ValueNotifier(null);

  /// Contacto que acaba de abrir una invitación de este dispositivo.
  final ValueNotifier<Contact?> accepted = ValueNotifier(null);

  Timer? _ringTimer;

  /// Suena mientras haya una llamada esperando respuesta, y como mucho medio
  /// minuto: quien llama puede haber desistido sin avisar.
  void _onIncomingChanged() {
    final call = incoming.value;
    _ringTimer?.cancel();
    _ringTimer = null;
    if (call == null || call.auto) {
      unawaited(BackgroundSession.ring(false));
      return;
    }
    unawaited(BackgroundSession.ring(true));
    _ringTimer = Timer(
      const Duration(seconds: 30),
      () => unawaited(BackgroundSession.ring(false)),
    );
  }

  /// `false` sin librería nativa (modo demostración): no hay con qué cifrar.
  bool get available => codec != null && prefs != null;

  List<Contact> get contacts => _contacts;
  String get deviceName => _deviceName;

  /// `true` si se aceptan llamadas con la app en segundo plano (Android).
  bool get backgroundAvailable => _background;

  /// Contacto al que se está llamando, o `null`.
  Contact? get calling =>
      _callTimer == null ? null : _byId(_activeId ?? '');

  Future<void> init() async {
    final prefs = this.prefs;
    if (prefs == null || codec == null) return;
    _deviceId = prefs.getString(_deviceIdKey) ?? '';
    if (_deviceId.isEmpty) {
      final rnd = Random.secure();
      _deviceId = List.generate(
          8, (_) => rnd.nextInt(256).toRadixString(16).padLeft(2, '0')).join();
      await prefs.setString(_deviceIdKey, _deviceId);
    }
    _deviceName = prefs.getString(_deviceNameKey) ??
        'Dispositivo ${_deviceId.substring(0, 4).toUpperCase()}';
    // Activo salvo que se apague: sin esto una llamada sólo llega con la app
    // a la vista.
    _background = prefs.getBool(_backgroundKey) ?? true;
    try {
      final raw = jsonDecode(prefs.getString(_contactsKey) ?? '[]') as List;
      _contacts = [
        for (final c in raw) Contact.fromJson(c as Map<String, dynamic>),
      ];
    } on Object {
      // Lista ilegible: mejor empezar vacía que no arrancar.
      _contacts = const [];
    }
    _controller
      ..deviceId = _deviceId
      ..deviceName = _deviceName
      ..onPaired = _onPaired
      ..addListener(_onController);
    incoming.addListener(_onIncomingChanged);
    if (transport == null && !_lanOff) await _listen();
    _mail = mailbox?.messages.listen((data) => handleDatagram(data, null));
    _syncMailbox();
    if (_background) unawaited(BackgroundSession.setStandby(true));
    notifyListeners();
  }

  Future<void> _listen() async {
    try {
      final socket = await RawDatagramSocket.bind(
        InternetAddress.anyIPv4,
        port,
        // En escritorio permite dos copias de la app en la misma máquina.
        reusePort: Platform.isLinux || Platform.isMacOS,
      );
      socket.broadcastEnabled = true;
      socket.listen((event) {
        if (event != RawSocketEvent.read) return;
        Datagram? d;
        while ((d = socket.receive()) != null) {
          _onDatagram(d!);
        }
      }, onError: (Object _) {});
      _socket = socket;
    } on SocketException {
      // Puerto ocupado: se puede llamar (con un socket de paso) pero no
      // recibir llamadas.
      _socket = null;
    }
  }

  // ── Datos del dispositivo ────────────────────────────────────────────────

  Future<void> setDeviceName(String name) async {
    final clean = name.trim();
    if (clean.isEmpty || clean == _deviceName) return;
    _deviceName = clean.length > 40 ? clean.substring(0, 40) : clean;
    _controller.deviceName = _deviceName;
    await prefs?.setString(_deviceNameKey, _deviceName);
    notifyListeners();
  }

  Future<void> setBackgroundAvailable(bool on) async {
    _background = on;
    await prefs?.setBool(_backgroundKey, on);
    await BackgroundSession.setStandby(on);
    notifyListeners();
  }

  // ── Lista ────────────────────────────────────────────────────────────────

  Contact? _byId(String id) {
    for (final c in _contacts) {
      if (c.id == id) return c;
    }
    return null;
  }

  Future<void> rename(Contact contact, String name) async {
    final clean = name.trim();
    if (clean.isEmpty) return;
    await _put(contact.copyWith(name: clean, custom: true));
  }

  /// Guarda lo que el usuario cambia en la ficha de un contacto. Se parte
  /// del contacto guardado, no del que tenga la pantalla: puede ser viejo.
  Future<void> edit(
    String contactId, {
    String? note,
    bool? muted,
    bool? autoAnswer,
  }) async {
    final current = _byId(contactId);
    if (current == null) return;
    final clean = note?.trim();
    final updated = current.copyWith(
      note: clean == null
          ? null
          : (clean.length > 280 ? clean.substring(0, 280) : clean),
      muted: muted,
      autoAnswer: autoAnswer,
    );
    _contacts = [
      for (final c in _contacts) c.id == contactId ? updated : c,
    ];
    await _save();
  }

  /// El contacto `id` tal como está guardado ahora, o `null` si se borró.
  Contact? byId(String id) => _byId(id);

  Future<void> remove(Contact contact) async {
    _contacts = [
      for (final c in _contacts)
        if (c.id != contact.id) c,
    ];
    await _save();
  }

  /// Guarda `contact` el primero de la lista, sustituyendo al que tuviera su
  /// id y a los que fueran el mismo dispositivo con un secreto anterior.
  Future<void> _put(Contact contact) async {
    _contacts = [
      contact,
      for (final c in _contacts)
        if (c.id != contact.id &&
            (contact.device.isEmpty || c.device != contact.device))
          c,
    ];
    await _save();
  }

  Future<void> _save() async {
    await prefs?.setString(
      _contactsKey,
      jsonEncode([for (final c in _contacts) c.toJson()]),
    );
    _syncMailbox();
    notifyListeners();
  }

  String? _idFor(String secret) {
    final id = codec?.contactId(Uint8List.fromList(utf8.encode(secret)));
    return id == null ? null : _hex(id);
  }

  static String _hex(List<int> bytes) =>
      bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();

  // ── Añadir por enlace ────────────────────────────────────────────────────

  /// Enlace para que otra persona se añada como contacto de este dispositivo.
  ///
  /// Crea ya el contacto de este lado, sin nombre: lo pone el otro al abrir
  /// el enlace. Mientras nadie lo haya abierto se reutiliza el mismo, para no
  /// llenar la lista de contactos a medias cada vez que se enseña.
  Future<String?> contactLink() async {
    final prefs = this.prefs;
    if (!available || prefs == null) return null;
    var secret = prefs.getString(_pendingKey) ?? '';
    final pending = secret.isEmpty ? null : _byId(_idFor(secret) ?? '');
    if (pending == null || pending.device.isNotEmpty) {
      const alphabet =
          'ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz23456789';
      final rnd = Random.secure();
      secret = List.generate(
          32, (_) => alphabet[rnd.nextInt(alphabet.length)]).join();
      final id = _idFor(secret);
      if (id == null) return null;
      await prefs.setString(_pendingKey, secret);
      // Al final de la lista: aún no es nadie.
      _contacts = [
        ..._contacts,
        Contact(id: id, secret: secret, name: pendingName),
      ];
      await _save();
    }
    return ContactLink(
      secret: secret,
      name: _deviceName,
      device: _deviceId,
      ip: await _controller.localIp(),
    ).toUri();
  }

  bool _wasLive = false;
  String? _talkingId;

  /// Contacto con el que se está hablando ahora, o `null`.
  Contact? get talkingWith => _byId(_talkingId ?? '');

  /// La conversación que salió de una invitación ha terminado: ya no se
  /// espera a nadie más por ella.
  void _onController() {
    _syncRoom();
    final live = _controller.isLive;
    if (_wasLive && !live && _invitingId != null) stopInviting();
    if (_wasLive && !live && _talkingId != null) {
      _talkingId = null;
      notifyListeners();
    }
    _wasLive = live;
  }

  // ── Sala abierta con un enlace de sala ───────────────────────────────────

  /// Id y token de la sala de la que este dispositivo es anfitrión: por ese
  /// buzón llega la respuesta de quien entra, aunque aún no sea un contacto.
  String? _roomId;
  String? _roomSecret;
  Timer? _roomAnswerTimer;

  /// Token de la sala en curso, o `null` si no hay sala con token.
  String? get _roomToken {
    final c = _controller;
    if (c.role == null || c.roomCode == null) return null;
    final token = c.server.token;
    return token.isEmpty ? null : token;
  }

  /// Con un enlace de sala los dos conocen su token antes de conectar: basta
  /// para que el invitado mande solo sus direcciones y el anfitrión las
  /// reciba. Sin esto, entre redes distintas había que enseñarle un segundo
  /// código al anfitrión.
  void _syncRoom() {
    final c = _controller;
    final token = _roomToken;
    final hosting = token != null && c.role == ConnectionRole.host;
    final id = hosting ? _idFor(token) : null;
    if (id != _roomId) {
      _roomId = id;
      _roomSecret = id == null ? null : token;
      _syncMailbox();
    }

    final joining = token != null &&
        c.role == ConnectionRole.join &&
        c.answerUri != null &&
        !c.isLive;
    if (!joining) {
      _roomAnswerTimer?.cancel();
      _roomAnswerTimer = null;
      return;
    }
    if (_roomAnswerTimer != null) return;
    final roomId = _idFor(token);
    if (roomId == null) return;
    // No se guarda: sólo da forma al destino del envío.
    final room = Contact(id: roomId, secret: token, name: '');
    void send() {
      final answer = _controller.answerUri;
      if (answer != null) unawaited(_send(room, 'answer', invite: answer));
    }

    send();
    _roomAnswerTimer =
        Timer.periodic(const Duration(seconds: 1), (_) => send());
  }

  /// Respuesta de un invitado a la sala de la que se es anfitrión.
  void _onRoomDatagram(KnockCodec codec, String secret, Uint8List data) {
    final plain =
        codec.knockOpen(Uint8List.fromList(utf8.encode(secret)), data);
    if (plain == null) return;
    final Object? msg;
    try {
      msg = jsonDecode(utf8.decode(plain));
    } on Object {
      return;
    }
    if (msg is! Map<String, dynamic>) return;
    final ts = msg['ts'];
    final answer = msg['uri'];
    if (msg['dev'] == _deviceId || ts is! int || answer is! String) return;
    final age = DateTime.now().millisecondsSinceEpoch - ts;
    if (age.abs() > _maxAge.inMilliseconds || msg['t'] != 'answer') return;
    unawaited(_controller.applyAnswer(answer));
  }

  /// Empieza a esperar a quien abra la invitación `link`: en cuanto salude,
  /// se le llama solo. Dura hasta [stopInviting].
  void startInviting(String link) {
    final parsed = ContactLink.tryParse(link);
    _invitingId = parsed == null ? null : _idFor(parsed.secret);
    _greeted.clear();
  }

  /// Deja de esperar (se salió de la pantalla sin que entrara nadie, o la
  /// conversación que salió de esa invitación terminó).
  void stopInviting() {
    _invitingId = null;
    _greeted.clear();
    autoCall.value = null;
  }

  /// Alguien ha contestado a la invitación en curso: a hablar.
  void _onGreeting(Contact contact) {
    if (contact.id != _invitingId || contact.device.isEmpty) return;
    if (!_greeted.add(contact.device)) return;
    if (_controller.isLive) {
      // Ya hay conversación: el que llega se suma a ella.
      ring(contact);
    } else if (!_controller.busy && _callTimer == null) {
      autoCall.value = contact;
    }
  }

  /// Guarda como contacto a quien envió el enlace `raw` y le saluda para que
  /// él sepa quién lo abrió. `null` si aquello no era un enlace de contacto.
  Future<Contact?> addFromLink(String raw) async {
    final link = ContactLink.tryParse(raw);
    if (link == null || !available) return null;
    // El enlace propio, abierto en el mismo dispositivo: no es un contacto.
    if (link.device == _deviceId) return null;
    final id = _idFor(link.secret);
    if (id == null) return null;
    final known = link.device.isEmpty
        ? null
        : _contacts.where((c) => c.device == link.device).firstOrNull;
    final contact = Contact(
      id: id,
      secret: link.secret,
      name: known != null && known.custom
          ? known.name
          : link.name.isNotEmpty
              ? link.name
              : pendingName,
      custom: known?.custom ?? false,
      device: link.device,
      lastLocal: link.ip,
      lastSeen: known?.lastSeen ?? 0,
    );
    await _put(contact);
    // Quien abre una invitación quiere hablar: si le llaman enseguida, entra.
    _autoAcceptId = contact.id;
    _autoAcceptUntil = DateTime.now().add(const Duration(seconds: 30));
    // Varias veces y espaciadas: el otro puede tardar en estar a la escucha.
    for (final wait in const [0, 1, 3, 6, 10]) {
      Timer(Duration(seconds: wait), () => _send(contact, 'hello'));
    }
    return contact;
  }

  /// Invita a `contact` a la sala ya en curso: le llega como una llamada.
  /// Devuelve `false` si no hay sala que ofrecer.
  bool ring(Contact contact) {
    final invite = _controller.pairingUri?.toUri();
    if (invite == null) return false;
    for (final wait in const [0, 1, 2, 4, 6]) {
      Timer(Duration(seconds: wait), () {
        final target = _byId(contact.id);
        if (target != null && _controller.pairingUri?.toUri() == invite) {
          _send(target, 'call', invite: invite);
        }
      });
    }
    return true;
  }

  // ── Al emparejar ─────────────────────────────────────────────────────────

  Future<void> _onPaired(PairedEvent e) async {
    final now = DateTime.now().millisecondsSinceEpoch;
    final active = _byId(_activeId ?? '');
    _stopCalling();
    _activeId = null;
    if (active != null) {
      // Era una llamada entre contactos: sólo se anota que se habló.
      _talkingId = active.id;
      await _put(active.copyWith(lastSeen: now, lastLocal: e.peerHost));
      return;
    }
    if (e.token.isEmpty) return;
    final id = _idFor(e.token);
    if (id == null) return;
    final known = e.peerDevice.isEmpty
        ? null
        : _contacts.where((c) => c.device == e.peerDevice).firstOrNull;
    final contact = Contact(
      id: id,
      secret: e.token,
      // Si ya se le conocía con un nombre puesto a mano, se conserva.
      name: known != null && known.custom
          ? known.name
          : e.peerName.isNotEmpty
              ? e.peerName
              : e.isHost
                  ? 'Invitado de ${e.room}'
                  : 'Sala ${e.room}',
      custom: known?.custom ?? false,
      device: e.peerDevice,
      lastLocal: e.peerHost,
      lastSeen: now,
    );
    _talkingId = contact.id;
    await _put(contact);
    // El anfitrión no sabe quién ha entrado: el invitado se presenta. Varias
    // veces porque es UDP y el anfitrión puede estar aún ocupado conectando.
    if (!e.isHost) {
      for (final wait in const [0, 1, 3]) {
        Timer(Duration(seconds: wait), () => _send(contact, 'hello'));
      }
    }
  }

  // ── Llamar ───────────────────────────────────────────────────────────────

  /// Empieza a llamar a `contact`. La sala la abre quien llama (la pantalla
  /// de crear sala); esto envía la invitación en cuanto existe y la repite
  /// hasta que el otro entra, se cancela o pasa el plazo.
  void call(Contact contact) {
    _stopCalling();
    _activeId = contact.id;
    final started = DateTime.now();
    var hadRoom = false;
    void tick() {
      final invite = _controller.pairingUri?.toUri();
      final expired = DateTime.now().difference(started) > _callTimeout;
      // Sala cerrada sin conectar: no seguir llamando con la siguiente.
      if (_controller.isLive || expired || (hadRoom && invite == null)) {
        _stopCalling();
        if (!_controller.isLive) _activeId = null;
        notifyListeners();
        return;
      }
      if (invite == null) return;
      hadRoom = true;
      final target = _byId(contact.id);
      if (target != null) _send(target, 'call', invite: invite);
    }

    _callTimer = Timer.periodic(const Duration(seconds: 1), (_) => tick());
    tick();
    notifyListeners();
  }

  /// Deja de llamar (el usuario salió de la pantalla de espera).
  void cancelCall() {
    if (_callTimer == null) return;
    _stopCalling();
    _activeId = null;
    notifyListeners();
  }

  void _stopCalling() {
    _callTimer?.cancel();
    _callTimer = null;
  }

  Future<void> _send(Contact contact, String type, {String? invite}) async {
    final codec = this.codec;
    if (codec == null) return;
    final ip = await _controller.localIp();
    final payload = utf8.encode(jsonEncode({
      't': type,
      'dev': _deviceId,
      'name': _deviceName,
      'ts': DateTime.now().millisecondsSinceEpoch,
      'ip': ?ip,
      'uri': ?invite,
    }));
    final sealed = codec.knockSeal(
      Uint8List.fromList(utf8.encode(contact.secret)),
      Uint8List.fromList(payload),
    );
    if (sealed == null) return;
    mailbox?.publish(_topic(contact.id), sealed);
    if (_lanOff && transport == null) return;
    final targets = <String>{
      ?contact.lastLocal,
      ?contact.lastPublic,
      // Por si cambió de IP dentro de la misma red.
      '255.255.255.255',
      if (ip != null && ip.contains('.'))
        '${ip.substring(0, ip.lastIndexOf('.'))}.255',
    };
    final fake = transport;
    if (fake != null) {
      for (final target in targets) {
        fake(sealed, target);
      }
      return;
    }
    RawDatagramSocket? temporary;
    try {
      final socket = _socket ??
          (temporary = await RawDatagramSocket.bind(InternetAddress.anyIPv4, 0)
            ..broadcastEnabled = true);
      for (final target in targets) {
        final address = InternetAddress.tryParse(target);
        if (address == null || address.type != InternetAddressType.IPv4) {
          continue;
        }
        try {
          socket.send(sealed, address, port);
        } on SocketException {
          // Sin ruta hacia esa dirección: las demás siguen valiendo.
        }
      }
    } on SocketException {
      // Sin red: no hay a quién llamar.
    } finally {
      temporary?.close();
    }
  }

  // ── Recibir ──────────────────────────────────────────────────────────────

  void _onDatagram(Datagram d) => handleDatagram(d.data, d.address);

  /// Procesa un datagrama llegado de `address`.
  @visibleForTesting
  void handleDatagram(Uint8List data, InternetAddress? address) {
    final codec = this.codec;
    // "GTKN", versión, 8 bytes de id: lo justo para saber de quién es.
    if (codec == null || data.length < 41 || data[0] != 0x47) return;
    final id = _hex(data.sublist(5, 13));
    final contact = _byId(id);
    if (contact == null) {
      final room = _roomSecret;
      if (room != null && id == _roomId) _onRoomDatagram(codec, room, data);
      return;
    }
    final plain = codec.knockOpen(
      Uint8List.fromList(utf8.encode(contact.secret)),
      data,
    );
    if (plain == null) return;
    final Map<String, dynamic> msg;
    try {
      msg = jsonDecode(utf8.decode(plain)) as Map<String, dynamic>;
    } on Object {
      return;
    }
    final device = msg['dev'];
    final ts = msg['ts'];
    // Una difusión propia vuelve por la red local: no es una llamada.
    if (device is! String || device == _deviceId || ts is! int) return;
    final age = DateTime.now().millisecondsSinceEpoch - ts;
    if (age.abs() > _maxAge.inMilliseconds) return;

    final name = msg['name'];
    // Lo que llega por un buzón no trae dirección de origen.
    final from = address?.address;
    final private = address != null && _isPrivate(address);
    final updated = contact.copyWith(
      device: device,
      name: !contact.custom && name is String && name.trim().isNotEmpty
          ? name.trim()
          : null,
      lastLocal: private ? from : null,
      lastPublic: private ? null : from,
      lastSeen: DateTime.now().millisecondsSinceEpoch,
    );
    unawaited(_put(updated));

    final invite = msg['uri'];
    if (msg['t'] == 'hello') {
      // Primer saludo por una invitación de este dispositivo: la aceptaron.
      if (contact.device.isEmpty && contact.name == pendingName) {
        accepted.value = updated;
        if (WidgetsBinding.instance.lifecycleState !=
            AppLifecycleState.resumed) {
          unawaited(BackgroundSession.notifyInfo(
            '${updated.name} aceptó tu invitación',
            'Ya está en tus contactos.',
          ));
        }
      }
      _onGreeting(updated);
    }
    // El invitado manda solo las direcciones por las que se le alcanza: es
    // el antiguo "código de respuesta", sin que nadie tenga que enseñarlo.
    if (msg['t'] == 'answer' && invite is String) {
      unawaited(_controller.applyAnswer(invite));
      return;
    }
    if (msg['t'] != 'call' || invite is! String) return;
    if (_controller.isLive || _controller.busy) return;
    if (_dismissed.contains(invite) || incoming.value?.invite == invite) return;
    if (updated.muted) return;
    final auto = updated.autoAnswer ||
        updated.id == _autoAcceptId &&
            DateTime.now().isBefore(_autoAcceptUntil);
    if (auto) _autoAcceptId = null;
    incoming.value = IncomingCall(contact: updated, invite: invite, auto: auto);
    if (!auto &&
        WidgetsBinding.instance.lifecycleState != AppLifecycleState.resumed) {
      unawaited(BackgroundSession.notifyCall(updated.name));
    }
  }

  static bool _isPrivate(InternetAddress a) {
    final b = a.rawAddress;
    if (b.length != 4) return false;
    return b[0] == 10 ||
        (b[0] == 172 && b[1] >= 16 && b[1] < 32) ||
        (b[0] == 192 && b[1] == 168) ||
        (b[0] == 169 && b[1] == 254) ||
        b[0] == 127;
  }

  /// Acepta la solicitud pendiente y devuelve la invitación con la que entrar.
  String? accept() {
    final call = incoming.value;
    if (call == null) return null;
    incoming.value = null;
    _dismissed.add(call.invite);
    _activeId = call.contact.id;
    unawaited(BackgroundSession.cancelCall());
    _sendAnswers(call.contact.id);
    return call.invite;
  }

  /// Mientras se intenta entrar, manda al que llamó las direcciones por las
  /// que se alcanza a este dispositivo, para que su router deje pasar.
  void _sendAnswers(String contactId) {
    _answerTimer?.cancel();
    var ticks = 0;
    _answerTimer = Timer.periodic(const Duration(seconds: 1), (timer) {
      final answer = _controller.answerUri;
      final contact = _byId(contactId);
      if (++ticks > 25 || _controller.isLive || contact == null) {
        timer.cancel();
        return;
      }
      if (answer != null) _send(contact, 'answer', invite: answer);
    });
  }

  /// Rechaza la solicitud pendiente: esa llamada no vuelve a avisar.
  void reject() {
    final call = incoming.value;
    if (call == null) return;
    _dismissed.add(call.invite);
    incoming.value = null;
    unawaited(BackgroundSession.cancelCall());
  }

  @override
  void dispose() {
    _controller.removeListener(_onController);
    _mail?.cancel();
    mailbox?.dispose();
    _stopCalling();
    _answerTimer?.cancel();
    _roomAnswerTimer?.cancel();
    _ringTimer?.cancel();
    incoming.removeListener(_onIncomingChanged);
    accepted.dispose();
    autoCall.dispose();
    _socket?.close();
    incoming.dispose();
    super.dispose();
  }
}
