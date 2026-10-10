/// Tests del onboarding: el botón "Empezar" debe llevar a la home.
///
/// El bug que cubren: al pulsar "Empezar" no pasaba nada y había que cerrar y
/// reabrir la app. La causa era `Navigator.of(context)` usando el contexto del
/// `Consumer`, que está POR ENCIMA de `MaterialApp` —quien crea el Navigator—,
/// así que la llamada fallaba sin encontrar un Navigator ascendente.
///
/// Estos tests verifican que terminarlo cambia el estado a HomeScreen, no que
/// navegue.
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:gravital_talk_app/app.dart';
import 'package:gravital_talk_app/screens/home_screen.dart';
import 'package:gravital_talk_app/screens/onboarding_screen.dart';
import 'package:gravital_talk_app/services/engine.dart';
import 'package:gravital_talk_app/services/event_log.dart';
import 'package:gravital_talk_app/services/room_api.dart';
import 'package:gravital_talk_app/services/room_events.dart';
import 'package:gravital_talk_app/services/session_controller.dart';
import 'package:gravital_talk_app/services/settings_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Pantalla de destino simplificada.
class Destino extends StatelessWidget {
  const Destino({super.key});

  @override
  Widget build(BuildContext context) => const Scaffold(
        body: Center(child: Text('destino')),
      );
}

Future<SessionController> _controller(EventLog log) async {
  final c = SessionController(
    DemoSessionEngine(),
    RoomApi(),
    log,
    SettingsStore(),
    RoomEvents(),
  );
  await c.init();
  return c;
}

void main() {
  group('OnboardingScreen', () {
    testWidgets('Saltar completa y navega usando un contexto válido',
        (tester) async {
      SharedPreferences.setMockInitialValues({});
      final log = EventLog();
      await _controller(log);

      var completado = 0;
      await tester.pumpWidget(MultiProvider(
        providers: [
          ChangeNotifierProvider.value(value: log),
          ChangeNotifierProvider.value(value: RoomEvents()),
          ChangeNotifierProvider.value(
            value: await _controller(log),
          ),
        ],
        child: MaterialApp(
          home: Builder(
            builder: (context) => OnboardingScreen(
              onDone: () {
                completado++;
                // El contexto del Builder sí está bajo el Navigator que crea
                // MaterialApp: es justo lo que faltaba en el código original.
                Navigator.of(context).pushReplacement(
                  MaterialPageRoute(builder: (_) => const Destino()),
                );
              },
            ),
          ),
        ),
      ));

      expect(find.byType(OnboardingScreen), findsOneWidget);

      await tester.tap(find.text('Saltar'));
      await tester.pumpAndSettle();

      expect(completado, 1, reason: 'Saltar también debe completarlo');
      expect(find.byType(Destino), findsOneWidget);
    });

    testWidgets('el último paso muestra el botón Empezar', (tester) async {
      SharedPreferences.setMockInitialValues({});
      await tester.pumpWidget(MaterialApp(
        home: OnboardingScreen(onDone: () {}),
      ));

      // Se avanza hasta el final para que aparezca "Empezar" en lugar de la
      // flecha de siguiente.
      await tester.tap(find.byIcon(Icons.arrow_forward));
      await tester.pumpAndSettle();
      await tester.tap(find.byIcon(Icons.arrow_forward));
      await tester.pumpAndSettle();

      expect(find.text('Empezar'), findsOneWidget);
    });
  });

  group('GravitalTalkApp', () {
    testWidgets('con onboarding, al terminar muestra la home',
        (tester) async {
      SharedPreferences.setMockInitialValues({});
      var persistido = 0;
      final log = EventLog();
      final controller = await _controller(log);

      await tester.pumpWidget(GravitalTalkApp(
        controller: controller,
        log: log,
        roomEvents: RoomEvents(),
        showOnboarding: true,
        onOnboardingDone: () => persistido++,
      ));
      await tester.pumpAndSettle();

      expect(find.byType(OnboardingScreen), findsOneWidget);

      // Al pulsar Saltar debe aparecer la home SIN reabrir la app.
      await tester.tap(find.text('Saltar'));
      await tester.pumpAndSettle();

      expect(persistido, 1, reason: 'la preferencia debe guardarse');
      expect(find.byType(OnboardingScreen), findsNothing);
      expect(find.byType(HomeScreen), findsOneWidget);
    });

    testWidgets('sin onboarding arranca directamente en la home',
        (tester) async {
      SharedPreferences.setMockInitialValues({});
      final log = EventLog();
      final controller = await _controller(log);

      await tester.pumpWidget(GravitalTalkApp(
        controller: controller,
        log: log,
        roomEvents: RoomEvents(),
        showOnboarding: false,
      ));
      await tester.pumpAndSettle();

      expect(find.byType(OnboardingScreen), findsNothing);
      expect(find.byType(HomeScreen), findsOneWidget);
    });
  });
}
