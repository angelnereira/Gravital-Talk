import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/services/validation.dart';

void main() {
  group('validateHost', () {
    test('acepta hostnames e IPv4', () {
      for (final host in [
        'relay.ejemplo.com',
        '192.168.1.10',
        'localhost',
        'a',
        '10.0.0.1',
      ]) {
        expect(validateHost(host, fieldName: 'el servidor'), isNull,
            reason: '$host debería ser válido');
      }
    });

    test('rechaza vacío y nulo', () {
      expect(validateHost('', fieldName: 'el servidor'), isNotNull);
      expect(validateHost(null, fieldName: 'el servidor'), isNotNull);
      expect(validateHost('   ', fieldName: 'el servidor'), isNotNull);
    });

    test('rechaza esquema incluido, el error más típico al copiar', () {
      // Copiar "http://relay:9100" del navegador y pegarlo entero es común.
      final err = validateHost('http://relay:9100', fieldName: 'el servidor');
      expect(err, isNotNull);
      expect(err!.message, contains('sin http://'));
    });

    test('rechaza rutas y espacios', () {
      expect(
        validateHost('relay.com/path', fieldName: 'x')!.message,
        contains('rutas'),
      );
      expect(validateHost('relay .com', fieldName: 'x'), isNotNull);
    });

    test('el mensaje nombra el campo concreto', () {
      final err = validateHost('', fieldName: 'el servidor');
      expect(err!.message, contains('servidor'));
    });

    test('el mensaje nombra el campo que se le pasa', () {
      // El campo se inyecta: si el validador llevara el nombre incrustado, este
      // test fallaría y avisaría de que hay dos caminos de validación.
      final err = validateHost('', fieldName: 'el peer');
      expect(err!.message, contains('peer'));
    });
  });

  group('validatePort', () {
    test('acepta el rango útil', () {
      expect(validatePort('1', fieldName: 'p'), isNull);
      expect(validatePort('9000', fieldName: 'p'), isNull);
      expect(validatePort('65535', fieldName: 'p'), isNull);
    });

    test('rechaza puerto 0, que significa efímero', () {
      // Alguien que teclea 0 no quiere "puerto aleatorio del sistema".
      expect(validatePort('0', fieldName: 'p'), isNotNull);
    });

    test('rechaza fuera de rango y no numérico', () {
      expect(validatePort('65536', fieldName: 'p'), isNotNull);
      expect(validatePort('-1', fieldName: 'p'), isNotNull);
      expect(validatePort('abc', fieldName: 'p'), isNotNull);
      expect(validatePort('', fieldName: 'p'), isNotNull);
      expect(validatePort(null, fieldName: 'p'), isNotNull);
    });
  });

  group('validateRoomCode', () {
    test('acepta el formato que genera el relay', () {
      expect(validateRoomCode('GRVT-A3F2'), isNull);
      expect(validateRoomCode('grvt-a3f2'), isNull, reason: 'normaliza a mayúsculas');
      expect(validateRoomCode('ABCD-1234'), isNull);
    });

    test('rechaza vacío y demasiado corto', () {
      expect(validateRoomCode(''), isNotNull);
      expect(validateRoomCode('   '), isNotNull);
      expect(validateRoomCode('AB'), isNotNull);
    });

    test('rechaza espacios internos', () {
      expect(validateRoomCode('GRVT A3F2'), isNotNull);
    });
  });

  group('validateRoomToken', () {
    test('es opcional: vacío es sala abierta', () {
      expect(validateRoomToken(''), isNull);
      expect(validateRoomToken(null), isNull);
      expect(validateRoomToken('   '), isNull);
    });

    test('exige longitud mínima cuando se rellena', () {
      expect(validateRoomToken('corta'), isNotNull);
      expect(validateRoomToken('secreto-largo-para-la-sala'), isNull);
    });
  });

  group('normalizeRoomCode', () {
    test('recorta y pone en mayúsculas', () {
      expect(normalizeRoomCode('  grvt-a3f2 '), 'GRVT-A3F2');
    });
  });
}
