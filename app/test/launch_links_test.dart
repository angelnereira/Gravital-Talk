import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/services/launch_links.dart';

void main() {
  const own = 'gravital-talk://i?v=2&s=SECRETO&n=Ana';
  const web = 'http://localhost:47801/gravital-talk/i?v=2&s=SECRETO&n=Ana';

  test('la forma para enviar apunta al propio dispositivo', () {
    expect(LaunchLinks.shareable(own), web);
    // Lo que no es un enlace de la app se deja como está.
    expect(LaunchLinks.shareable('https://example.org/x'),
        'https://example.org/x');
  });

  test('las dos formas se leen igual, también dentro de un mensaje', () {
    expect(LaunchLinks.extract(own), own);
    expect(LaunchLinks.extract(web), own);
    expect(LaunchLinks.extract('Hablemos:\n$web\nhasta luego'), own);
    expect(
      LaunchLinks.extract('http://127.0.0.1/gravital-talk/pair?room=A'),
      'gravital-talk://pair?room=A',
    );
    // Otra web, u otra ruta de este dispositivo, no es una invitación.
    expect(LaunchLinks.extract('https://example.org/gravital-talk/i?s=x'),
        isNull);
    expect(LaunchLinks.extract('http://localhost:47801/otra/cosa'), isNull);
    expect(LaunchLinks.extract('sin enlaces'), isNull);
  });

  test('ida y vuelta conserva el enlace entero', () {
    const pair = 'gravital-talk://pair?v=1&host=192.168.0.13&room=AB-12'
        '&udp=4000&token=x&pub=1.2.3.4%3A5&n=Mi+equipo';
    expect(LaunchLinks.extract(LaunchLinks.shareable(pair)), pair);
  });

  test('el navegador entrega el enlace a la app por este dispositivo',
      () async {
    final received = <String>[];
    LaunchLinks.listen(received.add);
    await LaunchLinks.serveLocal();

    final client = HttpClient();
    Future<int> get(String path) async {
      final request = await client.getUrl(
        Uri.parse('http://127.0.0.1:${LaunchLinks.localPort}$path'),
      );
      final response = await request.close();
      await response.drain<void>();
      return response.statusCode;
    }

    try {
      expect(await get('/gravital-talk/i?v=2&s=SECRETO&n=Ana'), 200);
      expect(received, [own]);
      expect(await get('/otra/cosa'), 404);
      expect(received, hasLength(1));
    } on SocketException {
      // El puerto lo tiene otra copia de la app en esta máquina: no hay
      // servidor propio que probar.
      markTestSkipped('puerto ${LaunchLinks.localPort} ocupado');
    } finally {
      client.close(force: true);
    }
  });
}
