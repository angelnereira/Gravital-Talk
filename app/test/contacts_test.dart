import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/services/contacts.dart';
import 'package:gravital_talk_app/services/engine.dart';
import 'package:gravital_talk_app/services/event_log.dart';
import 'package:gravital_talk_app/services/knock_codec.dart';
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

class _Device {
  _Device(this.prefs)
      : controller = SessionController(
          DemoSessionEngine(),
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
}
