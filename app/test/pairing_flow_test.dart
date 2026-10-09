/// Tests de `createRoom()`: crear sala sin relay.
///
/// El bug que cubren: "Crear sala y compartir" fallaba con
/// *"Configura la dirección del servidor"*, porque `hostServer()` exigía un
/// relay para una operación que no lo necesita. Quien crea la sala ES el host.
///
/// Estos tests verifican el contrato nuevo: crear sala funciona sin ningún
/// servidor configurado, genera código y secreto, y expone un endpoint para el
/// QR.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/models/session.dart';
import 'package:gravital_talk_app/services/engine.dart';
import 'package:gravital_talk_app/services/event_log.dart';
import 'package:gravital_talk_app/services/room_api.dart';
import 'package:gravital_talk_app/services/room_events.dart';
import 'package:gravital_talk_app/services/session_controller.dart';
import 'package:gravital_talk_app/services/settings_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Formato del código de sala: cuatro letras y cuatro dígitos, ninguno de los
/// cuales se confunde al leerlo en voz alta.
final _roomCodePattern = RegExp(r'^[ABCDEFGHJKLMNPQRSTUVWXYZ]{4}-[23456789]{4}$');

Future<SessionController> _controller() async {
  SharedPreferences.setMockInitialValues({});
  final c = SessionController(
    DemoSessionEngine(),
    RoomApi(),
    EventLog(),
    SettingsStore(),
    RoomEvents(),
  );
  await c.init();
  return c;
}

void main() {
  group('createRoom sin relay', () {
    test('funciona sin haber configurado ningún servidor', () async {
      final c = await _controller();
      expect(c.server.host, isEmpty);

      final ok = await c.createRoom();

      expect(
        ok,
        isTrue,
        reason: 'crear sala no debe exigir un servidor: el anfitrión ES el '
            'servidor',
      );
      expect(c.error, isNull);
      expect(c.isLive, isTrue);
    });

    test('genera un código de sala con formato legible', () async {
      final c = await _controller();
      await c.createRoom();

      expect(c.roomCode, isNotNull);
      expect(c.roomCode, matches(_roomCodePattern));
    });

    test('genera un secreto de sala y lo guarda', () async {
      final c = await _controller();
      await c.createRoom();

      // El token es el PSK de Noise. Se genera, no se pide: un usuario no
      // debería tener que inventarse un secreto criptográfico.
      expect(c.server.token, isNotEmpty);
      expect(c.server.token.length, greaterThanOrEqualTo(20));
    });

    test('expone un endpoint para el QR', () async {
      final c = await _controller();
      await c.createRoom();

      // Sin endpoint no hay QR que compartir, y el flujo se queda a medias.
      expect(c.publicEndpoint, isNotEmpty);
    });

    test('el anfitrión queda como participante activo', () async {
      final c = await _controller();
      await c.createRoom();

      // Es un participante más: habla y escucha como los demás, no es un
      // servidor del que dependan los otros.
      expect(c.isLive, isTrue);
      expect(c.state, SessionState.active);
    });

    test('el anfitrión puede transmitir como cualquier participante', () async {
      final c = await _controller();
      await c.createRoom();

      // El anfitrión administra la sala pero también habla. Si el rol bloqueara
      // el PTT, no podría participar en su propia conversación.
      await c.pttDown();
      expect(c.pttActive, isTrue);

      await c.pttUp();
      expect(c.pttActive, isFalse);
    });

    test('dos salas generan código y secreto distintos', () async {
      final c1 = await _controller();
      final c2 = await _controller();
      await c1.createRoom();
      await c2.createRoom();

      // Si dos salas coincidieran en código, un invitado entraría en la sala
      // equivocada.
      expect(c1.roomCode, isNot(c2.roomCode));
      expect(c1.server.token, isNot(c2.server.token));
    });
  });

  group('joinEndpoint', () {
    test('sin código de sala falla con mensaje claro', () async {
      final c = await _controller();

      final ok = await c.joinEndpoint('192.168.1.5', 9000);

      expect(ok, isFalse);
      expect(c.error, isNotNull,
          reason: 'el usuario tiene que saber qué le falta');
    });
  });

  group('diagnóstico de alcanzabilidad', () {
    test('nunca bloquea la creación de la sala', () async {
      final c = await _controller();

      // El aviso es informativo, no un requisito: un fallo del diagnóstico no
      // puede impedir crear la sala.
      final reach = await c.diagnoseReachability();
      expect(reach == null || reach.warning != null, isTrue);

      final ok = await c.createRoom();
      expect(ok, isTrue);
    });
  });
}
