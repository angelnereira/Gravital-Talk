import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/services/background_session.dart';

void main() {
  test('la burbuja avisa de la llamada y pasa a hablar al entrar en la sala',
      () async {
    await BackgroundSession.ringing('Ana');
    expect(BackgroundSession.bubbleState()['mode'], 'ringing');
    expect(BackgroundSession.bubbleState()['caller'], 'Ana');

    await BackgroundSession.start('ABCD');
    final live = BackgroundSession.bubbleState();
    expect(live['mode'], 'session');
    // La llamada ya contestada no vuelve a sonar al salir de la sala.
    expect(live['caller'], '');

    await BackgroundSession.update(talking: true, paused: false);
    await BackgroundSession.peerTalking(true);
    expect(BackgroundSession.bubbleState()['talking'], isTrue);
    expect(BackgroundSession.bubbleState()['peerTalking'], isTrue);

    await BackgroundSession.stop();
    final off = BackgroundSession.bubbleState();
    expect(off['mode'], 'off');
    expect(off['talking'], isFalse);
    expect(off['peerTalking'], isFalse);
  });

  test('apagarla en Ajustes viaja con el estado', () async {
    await BackgroundSession.setBubbleEnabled(false);
    expect(BackgroundSession.bubbleState()['enabled'], isFalse);
    await BackgroundSession.setBubbleEnabled(true);
    expect(BackgroundSession.bubbleState()['enabled'], isTrue);
  });
}
