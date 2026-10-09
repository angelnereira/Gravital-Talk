import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/app.dart';
import 'package:gravital_talk_app/services/engine.dart';
import 'package:gravital_talk_app/services/event_log.dart';
import 'package:gravital_talk_app/services/room_api.dart';
import 'package:gravital_talk_app/services/room_events.dart';
import 'package:gravital_talk_app/services/session_controller.dart';
import 'package:gravital_talk_app/services/settings_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  testWidgets('la home renderiza los dos modos de conexión',
      (WidgetTester tester) async {
    SharedPreferences.setMockInitialValues({});
    final log = EventLog();
    final controller = SessionController(
      DemoSessionEngine(),
      RoomApi(),
      log,
      SettingsStore(),
    );
    await controller.init();

    await tester.pumpWidget(GravitalTalkApp(
      controller: controller,
      log: log,
      roomEvents: RoomEvents(),
    ));
    await tester.pumpAndSettle();

    expect(find.text('Servidor (sala)'), findsOneWidget);
    expect(find.text('P2P directo'), findsOneWidget);
    expect(find.text('Audio en tiempo real'), findsOneWidget);
  });
}
