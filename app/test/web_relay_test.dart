import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/services/web_relay.dart';

void main() {
  group('WebRelay', () {
    test('un host normal no es relay web', () {
      expect(WebRelay.parse('relay.ejemplo.com'), isNull);
      expect(WebRelay.parse('192.168.1.10'), isNull);
      expect(WebRelay.parse(''), isNull);
    });

    test('https da API https y WebSocket wss en /ws', () {
      final r = WebRelay.parse('https://talk.gravitalcloud.com')!;
      expect(r.api('/api/rooms').toString(),
          'https://talk.gravitalcloud.com/api/rooms');
      expect(r.webSocketUrl, 'wss://talk.gravitalcloud.com/ws');
    });

    test('wss se traduce a https para la API', () {
      final r = WebRelay.parse('wss://x.up.railway.app/')!;
      expect(r.api('/healthz').toString(), 'https://x.up.railway.app/healthz');
      expect(r.webSocketUrl, 'wss://x.up.railway.app/ws');
    });

    test('http con puerto (pruebas locales) conserva el puerto', () {
      final r = WebRelay.parse('http://127.0.0.1:9100')!;
      expect(r.api('/api/rooms/ABCD-1234?token=t').toString(),
          'http://127.0.0.1:9100/api/rooms/ABCD-1234?token=t');
      expect(r.webSocketUrl, 'ws://127.0.0.1:9100/ws');
    });

    test('el servidor por defecto es un relay web', () {
      expect(WebRelay.matches(WebRelay.defaultUrl), isTrue);
    });
  });
}
