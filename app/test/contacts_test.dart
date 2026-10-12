import 'dart:async';
import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/services/contacts.dart';
import 'package:gravital_talk_app/services/engine.dart';
import 'package:gravital_talk_app/services/event_log.dart';
import 'package:gravital_talk_app/services/knock_codec.dart';
import 'package:gravital_talk_app/services/mailbox.dart';
import 'package:gravital_talk_app/services/room_api.dart';
import 'package:gravital_talk_app/services/session_controller.dart';
import 'package:gravital_talk_app/services/settings_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Sellado de mentira con el mismo contrato que el nativo: cabecera con el id
/// del contacto, y sólo abre quien tiene el mismo secreto.
class _FakeCodec implements KnockCodec {
  @override
  Uint8List contactId(Uint8List secret) {
    var h = 0xcbf29ce484222325;
    for (final b in secret) {
      h = ((h ^ b) * 0x100000001b3) & 0x7fffffffffffffff;
    }
    return Uint8List(8)..buffer.asByteData().setInt64(0, h);
  }

  @override
  Uint8List knockSeal(Uint8List secret, Uint8List payload) => Uint8List.fromList([
        ...'GTKN'.codeUnits,
        1,
        ...contactId(secret),
        // Relleno donde irían nonce y tag, para respetar el tamaño mínimo.
        ...List.filled(28, 0),
        ...payload,
      ]);

  @override
  Uint8List? knockOpen(Uint8List secret, Uint8List datagram) {
    final id = contactId(secret);
    for (var i = 0; i < 8; i++) {
      if (datagram[5 + i] != id[i]) return null;
    }
    return Uint8List.sublistView(datagram, 41);
  }
}

/// Un servidor de buzón de mentira: entrega a cada suscrito lo que se
/// publica en su tema, también al que publica (como hace MQTT).
class _Broker {
  final boxes = <_FakeMailbox>[];
  int published = 0;
}

class _FakeMailbox implements Mailbox {
  _FakeMailbox(this.broker) {
    broker.boxes.add(this);
  }

  final _Broker broker;
  final topics = <String>{};
  final _in = StreamController<Uint8List>.broadcast(sync: true);

  @override
  Stream<Uint8List> get messages => _in.stream;

  @override
  void subscribe(String topic) => topics.add(topic);

  @override
  void unsubscribe(String topic) => topics.remove(topic);

  @override
  void publish(String topic, Uint8List data) {
    broker.published++;
    for (final box in broker.boxes) {
      if (box.topics.contains(topic) && !box._in.isClosed) box._in.add(data);
    }
  }

  @override
  void dispose() => _in.close();
}

/// Motor cuyo emparejamiento no termina solo, y que apunta las direcciones
/// del invitado que le llegan.
class _WaitingEngine extends DemoSessionEngine {
  final hints = <String>[];
  final _never = Completer<void>();

  @override
  Future<void> acceptAny() => _never.future;

  @override
  Future<void> connectAny(List<String> candidates, {Duration? wait}) =>
      _never.future;

  @override
  Future<void> addPathHints(List<String> addrs) async => hints.addAll(addrs);
}

class _Device {
  _Device(this.prefs, [SessionEngine? engine])
      : controller = SessionController(
          engine ?? DemoSessionEngine(),
          RoomApi(),
          EventLog(),
          SettingsStore(),
          null,
        );

  final SharedPreferences prefs;
  final SessionController controller;
  late final ContactsService contacts;

  /// Datagramas que este dispositivo ha enviado.
  final sent = <Uint8List>[];
}

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  /// Dos dispositivos cuyos envíos llegan directamente al otro.
  Future<(_Device, _Device)> pair() async {
    SharedPreferences.setMockInitialValues({});
    final a = _Device(await SharedPreferences.getInstance());
    // Preferencias separadas: cada dispositivo tiene su identidad y su lista.
    SharedPreferences.resetStatic();
    SharedPreferences.setMockInitialValues({});
    final b = _Device(await SharedPreferences.getInstance());
    final from = InternetAddress('192.168.1.20');
    a.contacts = ContactsService(a.controller,
        codec: _FakeCodec(),
        prefs: a.prefs,
        transport: (d, _) {
          a.sent.add(d);
          b.contacts.handleDatagram(d, from);
        });
    b.contacts = ContactsService(b.controller,
        codec: _FakeCodec(),
        prefs: b.prefs,
        transport: (d, _) {
          b.sent.add(d);
          a.contacts.handleDatagram(d, from);
        });
    await a.contacts.init();
    await b.contacts.init();
    return (a, b);
  }

  test('al emparejar los dos se guardan como contacto, con su nombre',
      () async {
    final (a, b) = await pair();
    await a.contacts.setDeviceName('Ana');
    await b.contacts.setDeviceName('Beto');

    // Ana abre la sala y Beto entra con su invitación.
    a.controller.onPaired!(
        const PairedEvent(isHost: true, token: 'secreto', room: 'GRVT-1'));
    b.controller.onPaired!(PairedEvent(
      isHost: false,
      token: 'secreto',
      room: 'GRVT-1',
      peerName: 'Ana',
      peerDevice: a.controller.deviceId,
      peerHost: '192.168.1.10',
    ));
    await Future<void>.delayed(const Duration(milliseconds: 200));

    expect(b.contacts.contacts.single.name, 'Ana');
    expect(b.contacts.contacts.single.lastLocal, '192.168.1.10');
    // El anfitrión no sabía quién entraba: lo aprende del saludo del invitado.
    expect(a.contacts.contacts.single.name, 'Beto');
    expect(a.contacts.contacts.single.device, b.controller.deviceId);
    expect(a.contacts.contacts.single.lastLocal, '192.168.1.20');

    // Sobreviven a un reinicio.
    final again = ContactsService(a.controller,
        codec: _FakeCodec(), prefs: a.prefs, transport: (_, _) {});
    await again.init();
    expect(again.contacts.single.name, 'Beto');
  });

  test('volver a emparejar con el mismo dispositivo no duplica el contacto',
      () async {
    final (a, b) = await pair();
    for (final token in ['uno', 'dos']) {
      b.controller.onPaired!(PairedEvent(
        isHost: false,
        token: token,
        room: 'GRVT-1',
        peerName: 'Ana',
        peerDevice: a.controller.deviceId,
      ));
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }
    expect(b.contacts.contacts, hasLength(1));
    expect(b.contacts.contacts.single.secret, 'dos');
  });

  test('llamar a un contacto le entrega la invitación de la sala nueva',
      () async {
    final (a, b) = await pair();
    a.controller.onPaired!(
        const PairedEvent(isHost: true, token: 'secreto', room: 'GRVT-1'));
    b.controller.onPaired!(
        const PairedEvent(isHost: false, token: 'secreto', room: 'GRVT-1'));
    await Future<void>.delayed(const Duration(milliseconds: 200));

    // Ana abre una sala y llama a Beto.
    final room = a.controller.createRoom();
    await Future<void>.delayed(const Duration(milliseconds: 100));
    final invite = a.controller.pairingUri?.toUri();
    expect(invite, isNotNull);
    a.contacts.call(a.contacts.contacts.single);
    await Future<void>.delayed(const Duration(milliseconds: 100));

    final call = b.contacts.incoming.value;
    expect(call, isNotNull);
    expect(call!.invite, invite);
    // La misma llamada repetida no vuelve a avisar tras rechazarla.
    b.contacts.reject();
    b.contacts.handleDatagram(a.sent.last, InternetAddress('192.168.1.10'));
    expect(b.contacts.incoming.value, isNull);

    await room;
    a.contacts.cancelCall();
    await a.controller.reset();
  });

  test('una llamada vieja, ajena o propia no hace sonar nada', () async {
    final (a, b) = await pair();
    b.controller.onPaired!(
        const PairedEvent(isHost: true, token: 'secreto', room: 'GRVT-1'));
    await Future<void>.delayed(const Duration(milliseconds: 50));
    final codec = _FakeCodec();
    Uint8List knock(String secret, Map<String, Object> msg) => codec.knockSeal(
          Uint8List.fromList(utf8.encode(secret)),
          Uint8List.fromList(utf8.encode(jsonEncode(msg))),
        );
    final now = DateTime.now().millisecondsSinceEpoch;
    final from = InternetAddress('192.168.1.10');
    const uri = 'gravital-talk://pair?room=GRVT-9';

    // De ayer: una captura reenviada.
    b.contacts.handleDatagram(
        knock('secreto', {'t': 'call', 'dev': 'x', 'ts': now - 86400000, 'uri': uri}),
        from);
    // Con un secreto que no es de ningún contacto.
    b.contacts.handleDatagram(
        knock('otro', {'t': 'call', 'dev': 'x', 'ts': now, 'uri': uri}), from);
    // La propia difusión, de vuelta por la red local.
    b.contacts.handleDatagram(
        knock('secreto',
            {'t': 'call', 'dev': b.controller.deviceId, 'ts': now, 'uri': uri}),
        from);
    // Basura.
    b.contacts.handleDatagram(Uint8List.fromList(List.filled(60, 0x47)), from);
    expect(b.contacts.incoming.value, isNull);

    b.contacts.handleDatagram(
        knock('secreto', {'t': 'call', 'dev': 'x', 'ts': now, 'uri': uri}), from);
    expect(b.contacts.incoming.value?.invite, uri);
    expect(a.sent, isEmpty);
  });

  test('un enlace de contacto deja guardados a los dos, sin sala de por medio',
      () async {
    final (a, b) = await pair();
    await a.contacts.setDeviceName('Ana');
    await b.contacts.setDeviceName('Beto');

    final link = await a.contacts.contactLink();
    expect(link, startsWith('gravital-talk://i?v=2&'));
    // Enseñarlo dos veces no crea dos contactos a medias.
    expect(await a.contacts.contactLink(), link);
    expect(a.contacts.contacts.single.name, ContactsService.pendingName);

    final added = await b.contacts.addFromLink(link!);
    expect(added?.name, 'Ana');
    await Future<void>.delayed(const Duration(milliseconds: 200));
    // El saludo de Beto pone nombre al contacto que Ana tenía pendiente.
    expect(a.contacts.contacts.single.name, 'Beto');
    expect(a.contacts.contacts.single.device, b.controller.deviceId);

    // Abierto ya por alguien, el siguiente enlace es otro secreto.
    expect(await a.contacts.contactLink(), isNot(link));
    // El enlace propio no añade nada, ni uno con el secreto recortado.
    expect(await a.contacts.addFromLink(link), isNull);
    expect(await b.contacts.addFromLink('gravital-talk://contact?s=corto'),
        isNull);
    expect(await b.contacts.addFromLink('gravital-talk://pair?room=X'), isNull);
  });

  test('invitar a un contacto a la sala en curso le llega como llamada',
      () async {
    final (a, b) = await pair();
    final link = await a.contacts.contactLink();
    await b.contacts.addFromLink(link!);
    await Future<void>.delayed(const Duration(milliseconds: 100));

    // Sin sala no hay nada que ofrecer.
    expect(a.contacts.ring(a.contacts.contacts.first), isFalse);
    final room = a.controller.createRoom();
    await Future<void>.delayed(const Duration(milliseconds: 100));
    expect(a.contacts.ring(a.contacts.contacts.first), isTrue);
    await Future<void>.delayed(const Duration(milliseconds: 100));
    expect(b.contacts.incoming.value?.invite,
        a.controller.pairingUri?.toUri());
    await room;
    await a.controller.reset();
    // Los reintentos pendientes ya no tienen sala: no deben fallar.
    await Future<void>.delayed(const Duration(seconds: 7));
  });

  test('abrir la invitación basta: el que invita llama solo y el otro entra',
      () async {
    final (a, b) = await pair();
    await a.contacts.setDeviceName('Ana');
    final link = await a.contacts.contactLink();
    a.contacts.startInviting(link!);

    await b.contacts.addFromLink(link);
    await Future<void>.delayed(const Duration(milliseconds: 100));
    // Ana estaba esperando: le toca llamar, sin que nadie pulse nada.
    final callee = a.contacts.autoCall.value;
    expect(callee, isNotNull);
    expect(callee!.device, b.controller.deviceId);
    // Los saludos repetidos no vuelven a disparar la llamada.
    a.contacts.autoCall.value = null;
    await Future<void>.delayed(const Duration(seconds: 4));
    expect(a.contacts.autoCall.value, isNull);

    // Ana abre la sala y llama (es lo que hace la pantalla de invitar).
    final room = a.controller.createRoom();
    await Future<void>.delayed(const Duration(milliseconds: 100));
    a.contacts.call(callee);
    await Future<void>.delayed(const Duration(milliseconds: 100));
    // A Beto le entra sin preguntar: acaba de abrir la invitación.
    final call = b.contacts.incoming.value;
    expect(call?.auto, isTrue);
    expect(call!.invite, a.controller.pairingUri?.toUri());

    await room;
    a.contacts.cancelCall();
    await a.controller.reset();
    // Terminada la conversación, la invitación deja de esperar.
    a.contacts.autoCall.value = null;
    b.contacts.handleDatagram(b.sent.first, InternetAddress('192.168.1.20'));
    expect(a.contacts.autoCall.value, isNull);
  });

  test('sin invitación abierta, un saludo sólo guarda el contacto', () async {
    final (a, b) = await pair();
    final link = await a.contacts.contactLink();
    await b.contacts.addFromLink(link!);
    await Future<void>.delayed(const Duration(milliseconds: 100));
    expect(a.contacts.autoCall.value, isNull);
    expect(a.contacts.contacts.single.device, b.controller.deviceId);
    // Y una llamada posterior, ya fuera del margen, sí pregunta.
  });

  test('los enlaces de contacto antiguos siguen valiendo', () {
    final old = ContactLink.tryParse(
        'gravital-talk://contact?v=1&s=0123456789abcdefXYZ&n=Ana&dev=aa11&ip=10.0.0.2');
    expect(old?.name, 'Ana');
    expect(old?.device, 'aa11');
    final now = ContactLink.tryParse(old!.toUri());
    expect(now?.secret, old.secret);
    expect(now?.device, 'aa11');
    expect(now?.ip, '10.0.0.2');
  });

  test('sin red local en común, la invitación llega por el buzón', () async {
    SharedPreferences.setMockInitialValues({});
    final a = _Device(await SharedPreferences.getInstance());
    SharedPreferences.resetStatic();
    SharedPreferences.setMockInitialValues({});
    final b = _Device(await SharedPreferences.getInstance());
    final broker = _Broker();
    // La red local no entrega nada: están en redes distintas.
    a.contacts = ContactsService(a.controller,
        codec: _FakeCodec(),
        prefs: a.prefs,
        transport: (_, _) {},
        mailbox: _FakeMailbox(broker));
    b.contacts = ContactsService(b.controller,
        codec: _FakeCodec(),
        prefs: b.prefs,
        transport: (_, _) {},
        mailbox: _FakeMailbox(broker));
    await a.contacts.init();
    await b.contacts.init();
    await a.contacts.setDeviceName('Ana');
    await b.contacts.setDeviceName('Beto');

    final link = await a.contacts.contactLink();
    a.contacts.startInviting(link!);
    await b.contacts.addFromLink(link);
    await Future<void>.delayed(const Duration(milliseconds: 100));

    expect(a.contacts.contacts.single.name, 'Beto');
    final callee = a.contacts.autoCall.value;
    expect(callee, isNotNull);

    final room = a.controller.createRoom();
    await Future<void>.delayed(const Duration(milliseconds: 100));
    a.contacts.call(callee!);
    await Future<void>.delayed(const Duration(milliseconds: 100));
    expect(b.contacts.incoming.value?.auto, isTrue);
    expect(b.contacts.incoming.value?.invite,
        a.controller.pairingUri?.toUri());

    // Al borrar un contacto se deja de escuchar su buzón.
    await room;
    a.contacts.cancelCall();
    await a.controller.reset();
    final before = broker.boxes.first.topics.length;
    await a.contacts.remove(a.contacts.contacts.single);
    expect(broker.boxes.first.topics.length, before - 1);
    await Future<void>.delayed(const Duration(seconds: 4));
  });

  test('con un enlace de sala, la respuesta del invitado llega sola', () async {
    SharedPreferences.setMockInitialValues({});
    final hostEngine = _WaitingEngine();
    final a = _Device(await SharedPreferences.getInstance(), hostEngine);
    SharedPreferences.resetStatic();
    SharedPreferences.setMockInitialValues({});
    final b = _Device(await SharedPreferences.getInstance(), _WaitingEngine());
    final from = InternetAddress('203.0.113.7');
    a.contacts = ContactsService(a.controller,
        codec: _FakeCodec(),
        prefs: a.prefs,
        transport: (d, _) => b.contacts.handleDatagram(d, from));
    b.contacts = ContactsService(b.controller,
        codec: _FakeCodec(),
        prefs: b.prefs,
        transport: (d, _) => a.contacts.handleDatagram(d, from));
    await a.contacts.init();
    await b.contacts.init();

    unawaited(a.controller.createRoom());
    await Future<void>.delayed(const Duration(milliseconds: 100));
    final link = a.controller.pairingUri?.toUri();
    expect(link, isNotNull);

    // Nadie es contacto de nadie: sólo comparten el enlace.
    unawaited(b.controller.joinFromUri(link!));
    await Future<void>.delayed(const Duration(milliseconds: 300));

    expect(b.controller.answerUri, isNotNull);
    expect(hostEngine.hints, isNotEmpty,
        reason: 'el anfitrión debe recibir las direcciones del invitado');

    a.contacts.dispose();
    b.contacts.dispose();
  });
}
