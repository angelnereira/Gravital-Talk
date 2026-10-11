// Sonda manual: ¿entregan los buzones públicos lo que se publica?
import 'dart:async';
import 'dart:typed_data';

import 'package:gravital_talk_app/services/mailbox.dart';

Future<void> main() async {
  for (final server in MqttMailbox.defaultServers) {
    final a = MqttMailbox([server]);
    final b = MqttMailbox([server]);
    final topic = 'gt/1/probe${DateTime.now().microsecondsSinceEpoch}';
    var got = 0;
    b.messages.listen((m) => got++);
    b.subscribe(topic);
    final sw = Stopwatch()..start();
    // Publicado de inmediato: prueba también la cola previa a la conexión.
    a.publish(topic, Uint8List.fromList(List.filled(120, 7)));
    for (var i = 0; i < 40 && got == 0; i++) {
      await Future<void>.delayed(const Duration(milliseconds: 250));
      if (i == 12) a.publish(topic, Uint8List.fromList(List.filled(120, 9)));
    }
    // ignore: avoid_print
    print('$server  recibidos=$got  en ${sw.elapsedMilliseconds} ms');
    a.dispose();
    b.dispose();
  }
}
