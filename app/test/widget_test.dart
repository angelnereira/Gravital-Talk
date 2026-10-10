import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/app.dart';
import 'package:gravital_talk_app/services/engine.dart';
import 'package:gravital_talk_app/services/event_log.dart';
import 'package:gravital_talk_app/services/room_api.dart';
import 'package:gravital_talk_app/services/room_events.dart';
import 'package:gravital_talk_app/services/session_controller.dart';
import 'package:gravital_talk_app/services/settings_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

Future<SessionController> _pumpApp(WidgetTester tester) async {
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
  return controller;
}

void main() {
  testWidgets('la home ofrece crear o unirse sin pedir configuración de red',
      (WidgetTester tester) async {
    await _pumpApp(tester);

    // Dos acciones, y cada una dice lo que hace.
    expect(find.text('Crear sala'), findsOneWidget);
    expect(find.text('Unirse'), findsOneWidget);

    // Lo que ya NO debe estar: elegir entre "modo servidor" y "modo P2P", ni
    // los perfiles de red a la vista.
    expect(find.text('Servidor (sala)'), findsNothing);
    expect(find.text('P2P directo'), findsNothing);
    expect(find.text('Servidor'), findsNothing);
    expect(find.text('Peer P2P'), findsNothing);
  });

  testWidgets('unirse no muestra ningún campo de red',
      (WidgetTester tester) async {
    await _pumpApp(tester);

    await tester.tap(find.text('Unirse'));
    await tester.pumpAndSettle();

    // El contrato de la simplificación: cero campos de red. La dirección
    // viaja dentro de la invitación.
    expect(find.text('Host del relay'), findsNothing);
    expect(find.text('Puerto UDP'), findsNothing);
    expect(find.text('Puerto HTTP (rooms)'), findsNothing);
    expect(find.text('IP o host del peer'), findsNothing);

    // Un único campo: la invitación.
    expect(find.byType(TextField), findsOneWidget);
    expect(find.text('Pega la invitación'), findsOneWidget);
  });

  testWidgets('una invitación sin dirección se rechaza con un motivo',
      (WidgetTester tester) async {
    final controller = await _pumpApp(tester);

    await tester.tap(find.text('Unirse'));
    await tester.pumpAndSettle();
    await tester.enterText(find.byType(TextField), 'GRVT-2847');
    await tester.tap(find.widgetWithText(FilledButton, 'Unirse'));
    await tester.pumpAndSettle();

    // Un código pelado no dice a dónde conectar: se explica, no se intenta.
    expect(controller.isLive, isFalse);
    expect(find.textContaining('sólo trae el código'), findsOneWidget);
  });

  testWidgets('crear sala lleva sola a la pantalla de hablar al conectar',
      (WidgetTester tester) async {
    final controller = await _pumpApp(tester);

    await tester.tap(find.text('Crear sala'));
    // El motor demo acepta a los 300 ms y da la sesión por activa a los
    // 1,8 s. No se usa `pumpAndSettle`: la sesión viva tiene temporizadores
    // que nunca se asientan.
    await tester.pump();
    for (var i = 0; i < 6; i++) {
      await tester.pump(const Duration(milliseconds: 500));
    }

    expect(controller.isLive, isTrue);
    expect(find.text('Mantén pulsado para hablar'), findsOneWidget);
    // Las métricas del protocolo no están a la vista: van plegadas.
    expect(find.text('Jitter'), findsNothing);

    await controller.reset();
    // Dos tiempos: al volver a la home se relanzan sus animaciones de
    // entrada, que arrancan con retardo.
    await tester.pump(const Duration(seconds: 1));
    await tester.pump(const Duration(seconds: 1));
  });
}
