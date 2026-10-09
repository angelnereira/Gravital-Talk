import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/services/pairing_uri.dart';

/// Patrón para tests que parsean: comprueba que hay algo y sale si no.
///
/// `expect(p, isNotNull)` no promueve la variable, así que sin el guard el
/// analyzer obliga a `p!` y a la vez lo marca como innecesario. El guard es la
/// forma limpia y sin avisos.
PairingUri requireParsed(String raw) {
  final parsed = PairingUri.tryParse(raw);
  expect(parsed, isNotNull, reason: 'tryParse("$raw") devolvio null');
  if (parsed == null) fail('tryParse devolvio null');
  return parsed;
}

void main() {
  group('PairingUri.tryParse', () {
    test('interpreta el URI completo con todos los campos', () {
      final p = requireParsed(
        'gravital-talk://pair?v=1&host=relay.ejemplo.com&udp=9000&obs=9100'
            '&room=GRVT-A3F2&token=secreto',
      );
      expect(p.host, 'relay.ejemplo.com');
      expect(p.room, 'GRVT-A3F2');
      expect(p.udpPort, 9000);
      expect(p.obsPort, 9100);
      expect(p.token, 'secreto');
      expect(p.isComplete, isTrue);
    });

    test('los campos opcionales faltan sin romper el parse', () {
      final p = requireParsed('gravital-talk://pair?v=1&host=h&room=R-1');
      expect(p.udpPort, isNull);
      expect(p.obsPort, isNull);
      expect(p.token, isNull);
    });

    test('acepta el URI sin esquema, que algunos escáneres cortan', () {
      final p = requireParsed('//pair?host=h&room=R-2');
      expect(p.host, 'h');
      expect(p.room, 'R-2');
    });

    test('acepta un código pelado de un QR antiguo', () {
      // La compatibilidad hacia atrás es deliberada: los QR ya repartidos sólo
      // llevaban el código, y deben seguir funcionando.
      final p = requireParsed('GRVT-A3F2');
      expect(p.room, 'GRVT-A3F2');
      expect(p.host, isEmpty, reason: 'el usuario tendrá que ponerlo a mano');
      expect(p.isComplete, isFalse);
    });

    test('normaliza el código a mayúsculas', () {
      expect(requireParsed('grvt-a3f2').room, 'GRVT-A3F2');
    });

    test('rechaza lo que no es de Gravital Talk', () {
      expect(PairingUri.tryParse('https://ejemplo.com'), isNull);
      expect(PairingUri.tryParse(''), isNull);
      expect(PairingUri.tryParse('texto cualquiera sin guion'), isNull);
      expect(PairingUri.tryParse('otro://pair?host=h&room=R'), isNull);
    });

    test('rechaza un URI sin room', () {
      expect(PairingUri.tryParse('gravital-talk://pair?host=h'), isNull);
    });
  });

  group('PairingUri.toUri', () {
    test('round-trip: lo que se genera se vuelve a interpretar igual', () {
      const original = PairingUri(
        host: 'relay.ejemplo.com',
        room: 'GRVT-A3F2',
        udpPort: 9000,
        obsPort: 9100,
        token: 'secreto',
      );
      final parsed = requireParsed(original.toUri());
      expect(parsed.host, original.host);
      expect(parsed.room, original.room);
      expect(parsed.udpPort, original.udpPort);
      expect(parsed.obsPort, original.obsPort);
      expect(parsed.token, original.token);
    });

    test('omite el token cuando está vacío', () {
      const p = PairingUri(host: 'h', room: 'R-1', token: '');
      expect(p.toUri(), isNot(contains('token')));
    });

    test('escapa los caracteres raros del host y del token', () {
      // Un token con `&` o `=` rompería el query si no se escapase.
      const p = PairingUri(host: 'h', room: 'R', token: 'a&b=c');
      final parsed = requireParsed(p.toUri());
      expect(parsed.token, 'a&b=c');
    });
  });

  group('PairingUri.toHumanReadable', () {
    test('sin host sólo menciona la sala', () {
      const p = PairingUri(host: '', room: 'GRVT-A3F2');
      expect(p.toHumanReadable(), 'Sala GRVT-A3F2');
    });

    test('con host incluye servidor y puerto por defecto', () {
      const p = PairingUri(host: 'relay.ejemplo.com', room: 'R');
      final text = p.toHumanReadable();
      expect(text, contains('Sala R'));
      expect(text, contains('relay.ejemplo.com:9000'));
    });

    test('avisa cuando la sala exige token', () {
      const p = PairingUri(host: 'h', room: 'R', token: 'x');
      expect(p.toHumanReadable(), contains('token requerido'));
    });

    test('sin token no menciona el token', () {
      const p = PairingUri(host: 'h', room: 'R');
      expect(p.toHumanReadable(), isNot(contains('token')));
    });
  });

  group('isComplete', () {
    test('hace falta el host para conectar sin teclear', () {
      const sinHost = PairingUri(host: '', room: 'R');
      const conHost = PairingUri(host: 'h', room: 'R');
      expect(sinHost.isComplete, isFalse);
      expect(conHost.isComplete, isTrue);
    });
  });
}
