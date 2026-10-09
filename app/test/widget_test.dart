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
  testWidgets('la home ofrece conectar sin pedir configuración de red',
      (WidgetTester tester) async {
    SharedPreferences.setMockInitialValues({});
    final log = EventLog();
    final controller = SessionController(
      DemoSessionEngine(),
      RoomApi(),
      log,
      SettingsStore(),
      RoomEvents(),
    );
    await controller.init();

    await tester.pumpWidget(GravitalTalkApp(
      controller: controller,
      log: log,
      roomEvents: RoomEvents(),
    ));
    await tester.pumpAndSettle();

    // El flujo es una sola acción: conectar. No hay elección de topología.
    expect(find.text('Conectar'), findsOneWidget);
    expect(find.text('Crea una sala o únete con QR o código'), findsOneWidget);

    // Lo que ya NO debe estar: las dos entradas que obligaban a elegir entre
    // "modo servidor" y "modo P2P" antes de saber qué quería hacer el usuario.
    expect(find.text('Servidor (sala)'), findsNothing);
    expect(find.text('P2P directo'), findsNothing);
  });

  testWidgets('entrar a conectar no muestra ningún campo de red',
      (WidgetTester tester) async {
    SharedPreferences.setMockInitialValues({});
    final log = EventLog();
    final controller = SessionController(
      DemoSessionEngine(),
      RoomApi(),
      log,
      SettingsStore(),
      RoomEvents(),
    );
    await controller.init();

    await tester.pumpWidget(GravitalTalkApp(
      controller: controller,
      log: log,
      roomEvents: RoomEvents(),
    ));
    await tester.pumpAndSettle();

    await tester.tap(find.text('Conectar'));
    await tester.pumpAndSettle();

    // El contrato de la simplificación: cero campos de red. Ni host, ni
    // puertos, ni elección de transporte.
    expect(find.text('Host del relay'), findsNothing);
    expect(find.text('Puerto UDP'), findsNothing);
    expect(find.text('Puerto HTTP (rooms)'), findsNothing);
    expect(find.text('IP o host del peer'), findsNothing);

    // Y lo que sí está: crear o unirse, con código.
    expect(find.text('Crear sala'), findsWidgets);
    // "Unirse" es título de sección y etiqueta de botón a la vez.
    expect(find.text('Unirse'), findsNWidgets(2));
    expect(find.text('Escanear QR'), findsOneWidget);
  });
}
