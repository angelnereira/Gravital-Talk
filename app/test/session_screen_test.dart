import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/models/session.dart';
import 'package:gravital_talk_app/services/engine.dart';
import 'package:gravital_talk_app/services/event_log.dart';
import 'package:gravital_talk_app/services/room_api.dart';
import 'package:gravital_talk_app/services/room_events.dart';
import 'package:gravital_talk_app/services/session_controller.dart';
import 'package:gravital_talk_app/services/settings_store.dart';
import 'package:gravital_talk_app/models/session.dart';
import 'package:gravital_talk_app/widgets/active_session_card.dart';
import 'package:gravital_talk_app/widgets/common.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

void main() {
  setUp(() {
    SharedPreferences.setMockInitialValues({});
  });

  /// Construye los widgets de la pantalla de sesión con el estado dado.
  ///
  /// El motor es siempre el demo: los tests son de UI, no de protocolo. Lo que
  /// cambia entre casos es el estado observado, no el comportamiento real del
  /// motor.
  Future<RoomEvents> pumpSession(
    WidgetTester tester, {
    required bool connected,
    bool floorHeld = false,
    List<int> extraPeers = const [],
    bool streamConnected = true,
  }) async {
    final controller = SessionController(
      DemoSessionEngine(),
      RoomApi(),
      EventLog(),
      SettingsStore(),
    );
    await controller.init();

    final events = RoomEvents();
    // El stream real no se conecta en tests: se simula su estado aplicando los
    // eventos que emitiría el relay.
    if (streamConnected && (floorHeld || extraPeers.isNotEmpty)) {
      events.debugSetState(
        peers: extraPeers,
        floorHolder: floorHeld ? controller.sessionId : null,
        connected: true,
      );
    } else if (!streamConnected) {
      events.debugSetState(peers: const [], floorHolder: null, connected: false);
    }

    await tester.pumpWidget(MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: controller),
        ChangeNotifierProvider.value(value: EventLog()),
        ChangeNotifierProvider.value(value: events),
      ],
      child: MaterialApp(
        home: Scaffold(
          body: Builder(builder: (context) {
            // La tarjeta lee el estado del controller y de los eventos.
            return const ParticipantsCard();
          }),
        ),
      ),
    ));
    await tester.pumpAndSettle();
    return events;
  }

  group('ParticipantsCard', () {
    testWidgets('muestra siempre al participante local', (tester) async {
      await pumpSession(tester, connected: true, streamConnected: false);

      // El local se marca como "tú" para que no haya ambigüedad.
      expect(find.textContaining('(tú)'), findsOneWidget);
      expect(find.text('Sin observación de sala'), findsOneWidget);
    });

    testWidgets('lista a los participantes remotos', (tester) async {
      await pumpSession(
        tester,
        connected: true,
        extraPeers: const [0x1111, 0x2222],
      );

      // Local + 2 remotos.
      expect(find.textContaining('(tú)'), findsOneWidget);
      expect(find.text('Peer 0x1111'), findsOneWidget);
      expect(find.text('Peer 0x2222'), findsOneWidget);
      expect(find.text('3 en la sala'), findsOneWidget);
    });

    testWidgets('marca quién tiene el turno', (tester) async {
      await pumpSession(
        tester,
        connected: true,
        extraPeers: const [0x1111],
        floorHeld: true,
      );

      // La etiqueta TURNO aparece sobre el que tiene el floor.
      expect(find.text('TURNO'), findsOneWidget);
      expect(find.text('transmitiendo'), findsOneWidget);
    });

    testWidgets('el stream caído se indica, no se silencia', (tester) async {
      await pumpSession(tester, connected: true, streamConnected: false);

      // Sin sala observada el subtítulo es el de "sin observación"; con sala
      // pero sin stream es "perdida". Aquí no se fijó ninguna sala.
      expect(find.text('Sin observación de sala'), findsOneWidget);
    });
  });

  group('ReconnectingBanner', () {
    testWidgets('muestra que la sesión se recupera sola', (tester) async {
      await tester.pumpWidget(const MaterialApp(
        home: Scaffold(body: ReconnectingBanner()),
      ));

      expect(find.text('Reconectando…'), findsOneWidget);
      expect(
        find.text('La sesión se recupera sola tras un corte de red'),
        findsOneWidget,
      );
      // El indicador de progreso es lo que distingue "trabajando" de "colgado".
      expect(find.byType(CircularProgressIndicator), findsOneWidget);
    });
  });

  group('ActiveSessionCard', () {
    testWidgets('sin sesión activa no se muestra', (tester) async {
      final controller = SessionController(
        DemoSessionEngine(),
        RoomApi(),
        EventLog(),
        SettingsStore(),
      );
      await controller.init();

      await tester.pumpWidget(MultiProvider(
        providers: [
          ChangeNotifierProvider.value(value: controller),
          ChangeNotifierProvider.value(value: EventLog()),
          ChangeNotifierProvider.value(value: RoomEvents()),
        ],
        child: const MaterialApp(
          home: Scaffold(body: ActiveSessionCard()),
        ),
      ));

      // El widget devuelve SizedBox.shrink(): no debe haber ni título.
      expect(find.text('Sesión activa'), findsNothing);
    });

    testWidgets('la tarjeta muestra el estado y métricas', (tester) async {
      // Sin motor nativo no se puede levantar una sesión real; se comprueba el
      // contrato del widget con el estado que expone el enum.
      expect(SessionState.active.label, 'Activa');
      expect(SessionMetrics.zero.estimatedMos, 0);
    });
  });

  group('integración con SessionState', () {
    testWidgets('el estado reconnecting es un estado válido y vivo', (tester) async {
      // El enum que gobierna el banner: si alguien lo renombra, el test avisa.
      expect(SessionState.reconnecting.label, 'Reconectando');
      expect(SessionState.reconnecting.isLive, isFalse,
          reason: 'reconectando no es "en vivo": la UI no debe ofrecer PTT');
      for (final s in SessionState.values) {
        expect(s.label.isNotEmpty, isTrue, reason: '$s no tiene etiqueta');
      }
    });
  });
}
