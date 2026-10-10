/// Tests de `createRoom()`: crear sala sin relay, y del QR que se comparte.
///
/// El bug original que cubren: "Crear sala y compartir" fallaba con
/// *"Configura la dirección del servidor"*, porque `hostServer()` exigía un
/// relay para una operación que no lo necesita. Quien crea la sala ES el host.
///
/// El segundo bug, visto en una captura de usuario al emparejar dos móviles
/// (NativeException -2, "argumento inválido"): el QR de crear salida no llevaba
/// `host`, porque usaba el campo del relay en lugar del endpoint público del
/// anfitrión. Los tests de "regresión: QR sin host" fijan ese caso.
library;

import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/services/engine.dart';
import 'package:gravital_talk_app/services/event_log.dart';
import 'package:gravital_talk_app/services/pairing_uri.dart';
import 'package:gravital_talk_app/services/room_api.dart';
import 'package:gravital_talk_app/services/room_events.dart';
import 'package:gravital_talk_app/services/session_controller.dart';
import 'package:gravital_talk_app/services/settings_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Formato del código de sala: cuatro letras y cuatro dígitos, ninguno de los
/// cuales se confunde al leerlo en voz alta.
final _roomCodePattern = RegExp(r'^[ABCDEFGHJKLMNPQRSTUVWXYZ]{4}-[23456789]{4}$');

/// Igual que `_PairingScreenState._splitEndpoint`, duplicado a propósito: el
/// widget es privado y lo que se fija aquí es el contrato, no la clase.
(String, int) splitEndpoint(String endpoint, [int fallback = 9000]) {
  final text = endpoint.trim();
  if (text.isEmpty) return ('', fallback);

  if (text.startsWith('[')) {
    final end = text.indexOf(']');
    if (end > 0) {
      return (
        text.substring(1, end),
        int.tryParse(text.substring(end + 2)) ?? fallback,
      );
    }
  }

  final idx = text.lastIndexOf(':');
  if (idx <= 0 || idx == text.length - 1) return (text, fallback);
  final port = int.tryParse(text.substring(idx + 1));
  if (port == null) return (text, fallback);
  return (text.substring(0, idx), port);
}

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

    test('expone un endpoint utilizable para el QR', () async {
      final c = await _controller();
      await c.createRoom();

      // Sin endpoint no hay QR que compartir, y el flujo se queda a medias.
      final endpoint = c.publicEndpoint;
      expect(endpoint, isNotEmpty);

      // Y ese endpoint debe partirse en host y puerto: es lo que come el QR.
      final (host, port) = splitEndpoint(endpoint);
      expect(host, isNotEmpty, reason: 'el QR saldría sin destino');
      expect(port, greaterThan(0));
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

  group('regresión: QR sin host', () {
    // El QR de la captura de usuario era:
    //   gravital-talk://pair?v=1&room=WDSJ-9825&udp=9000&token=...
    // Sin `host`, porque se construía con el campo del relay en lugar del
    // endpoint público del anfitrión. El motor nativo recibía "" y respondía
    // NativeException(-2).
    test('un URI sin host deja el host vacío, no lo inventa', () async {
      final parsed = PairingUri.tryParse(
        'gravital-talk://pair?v=1&room=WDSJ-9825&udp=9000&token=abc',
      );

      expect(parsed, isNotNull);
      expect(parsed!.host, isEmpty,
          reason: 'esto es lo que producía el error -2');
      expect(parsed.room, 'WDSJ-9825');
      expect(parsed.isComplete, isFalse);
    });

    test('el ?? de Dart no captura una cadena vacía', () {
      // Es la trampa exacta que causó el bug: el QR traía `host` ausente, y
      // `PairingUri` lo devuelve como cadena vacía. `??` sólo actúa sobre
      // `null`, así que "" pasaba y llegaba al motor como argumento inválido.
      final String? hostDelQr = '';
      expect(hostDelQr ?? 'fallback', hostDelQr);

      final seguro = hostDelQr?.trim().isNotEmpty == true ? hostDelQr : 'fallback';
      expect(seguro, 'fallback');
    });

    test('un endpoint se parte en host y puerto', () {
      expect(splitEndpoint('203.0.113.45:9000'), ('203.0.113.45', 9000));
      expect(splitEndpoint('203.0.113.45'), ('203.0.113.45', 9000));
      expect(splitEndpoint('[2001:db8::1]:9000'), ('2001:db8::1', 9000));
      expect(splitEndpoint('red-local:9000'), ('red-local', 9000));
    });

    test('un endpoint vacío se detecta antes de conectar', () {
      // La comprobación que ahora hace la pantalla en vez de llegar al motor.
      final (host, _) = splitEndpoint('');
      expect(host, isEmpty);
    });
  });
}
