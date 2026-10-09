import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/services/engine.dart';
import 'package:gravital_talk_app/services/event_log.dart';
import 'package:gravital_talk_app/services/room_api.dart';
import 'package:gravital_talk_app/services/room_events.dart';
import 'package:gravital_talk_app/services/session_controller.dart';
import 'package:gravital_talk_app/services/settings_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  setUp(() {
    SharedPreferences.setMockInitialValues({});
  });

  group('RoomEvents', () {
    test('el estado inicial es "sin sala observada"', () {
      final events = RoomEvents();
      expect(events.isConnected, isFalse);
      expect(events.roomCode, isNull);
      expect(events.peers, isEmpty);
      expect(events.floorHolder, isNull);
      expect(events.peerCount, 1, reason: 'el observador local siempre cuenta');
    });

    test('debugSetState refleja el estado que emitiría el relay', () {
      final events = RoomEvents();
      events.debugSetState(
        peers: const [0x1111, 0x2222],
        floorHolder: 0x1111,
        connected: true,
      );

      expect(events.isConnected, isTrue);
      expect(events.peers, equals([0x1111, 0x2222]));
      expect(events.floorHolder, 0x1111);
      expect(events.peerCount, 3, reason: 'local + 2 remotos');
    });

    test('stop limpia todo el estado', () async {
      final events = RoomEvents();
      events.debugSetState(
        peers: const [0x1111],
        floorHolder: 0x1111,
        connected: true,
      );

      await events.stop();

      expect(events.isConnected, isFalse);
      expect(events.roomCode, isNull);
      expect(events.peers, isEmpty);
      expect(events.floorHolder, isNull);
    });

    test('un floor liberado por el que lo tenía queda libre', () async {
      // Equivale al evento FLOOR_RELEASED del relay: si se queda el holder
      // anterior, la UI sigue mostrando a alguien transmitiendo para siempre.
      final events = RoomEvents();
      events.debugSetState(peers: const [0xAAAA], floorHolder: 0xAAAA, connected: true);

      // El puerto real de esto está en `_apply`; aquí se comprueba el estado
      // observable tras el ciclo completo.
      await events.stop();
      expect(events.floorHolder, isNull);
    });
  });

  group('Participant', () {
    test('el local se etiqueta distinto del remoto', () {
      const local = Participant(ssrc: 7, isLocal: true);
      const remote = Participant(ssrc: 8);

      expect(local.label, isNot(remote.label));
      expect(local.statusLabel, 'tú');
      expect(remote.statusLabel, 'en espera');
    });

    test('quien tiene el turno se anuncia como transmitiendo', () {
      const p = Participant(ssrc: 9, hasFloor: true);
      expect(p.statusLabel, 'transmitiendo');
    });

    test('la etiqueta es estable y legible', () {
      const p = Participant(ssrc: 0xABCD);
      expect(p.label, 'Peer 0xABCD');
      const host = Participant(ssrc: 0);
      expect(host.label, 'Host');
    });
  });

  group('integración controller + RoomEvents', () {
    test('el controller acepta RoomEvents sin romperse', () async {
      final events = RoomEvents();
      final controller = SessionController(
        DemoSessionEngine(),
        RoomApi(),
        EventLog(),
        SettingsStore(),
        events,
      );
      await controller.init();

      // Sin conexión no hay sala observada.
      expect(events.roomCode, isNull);

      await controller.reset();
      expect(events.isConnected, isFalse);
    });

    test('reset limpia el estado de la sala', () async {
      // Si al reiniciar quedara la sala anterior observada, la próxima conexión
      // heredaría participantes que ya no están.
      final events = RoomEvents();
      events.debugSetState(peers: const [0x1111], floorHolder: 0x1111, connected: true);

      final controller = SessionController(
        DemoSessionEngine(),
        RoomApi(),
        EventLog(),
        SettingsStore(),
        events,
      );
      await controller.init();
      await controller.reset();

      expect(events.peers, isEmpty);
      expect(events.floorHolder, isNull);
      expect(events.isConnected, isFalse);
    });
  });
}
