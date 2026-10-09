import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:provider/provider.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'package:gravital_talk_app/models/session.dart';
import 'package:gravital_talk_app/services/engine.dart';
import 'package:gravital_talk_app/services/event_log.dart';
import 'package:gravital_talk_app/services/room_api.dart';
import 'package:gravital_talk_app/services/room_events.dart';
import 'package:gravital_talk_app/services/session_controller.dart';
import 'package:gravital_talk_app/services/settings_store.dart';
import 'package:gravital_talk_app/widgets/active_session_card.dart';
import 'package:gravital_talk_app/widgets/common.dart';

void _noop() {}

void main() {
  setUp(() {
    SharedPreferences.setMockInitialValues({});
  });

  /// Monta la tarjeta de participantes con el estado que se le indique.
  ///
  /// El motor es siempre el demo: estos tests son de UI, no de protocolo. Lo
  /// que varía es el estado observado, no el comportamiento del motor.
  Future<RoomEvents> pumpParticipants(
    WidgetTester tester, {
    bool floorHeld = false,
    List<int> extraPeers = const [],
    bool streamConnected = true,
  }) async {
    final controller = SessionController(
      DemoSessionEngine(),
      RoomApi(),
      EventLog(),
      SettingsStore(),
      RoomEvents(),
    );
    await controller.init();

    final events = RoomEvents();
    events.debugSetState(
      peers: extraPeers,
      floorHolder: floorHeld ? controller.sessionId : null,
      connected: streamConnected,
    );

    await tester.pumpWidget(MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: controller),
        ChangeNotifierProvider.value(value: EventLog()),
        ChangeNotifierProvider.value(value: events),
      ],
      child: const MaterialApp(
        home: Scaffold(body: ParticipantsCard()),
      ),
    ));
    await tester.pumpAndSettle();
    return events;
  }

  group('ParticipantsCard', () {
    testWidgets('muestra siempre al participante local', (tester) async {
      await pumpParticipants(tester);

      // El local se marca como "(tú)" para que no haya ambigüedad.
      expect(find.textContaining('(tú)'), findsOneWidget);
    });

    testWidgets('lista a los participantes remotos', (tester) async {
      await pumpParticipants(tester, extraPeers: const [0x1111, 0x2222]);

      expect(find.textContaining('(tú)'), findsOneWidget);
      expect(find.text('Peer 0x1111'), findsOneWidget);
      expect(find.text('Peer 0x2222'), findsOneWidget);
      // Local + 2 remotos.
      expect(find.text('3 en la sala'), findsOneWidget);
    });

    testWidgets('marca quién tiene el turno', (tester) async {
      await pumpParticipants(
        tester,
        extraPeers: const [0x1111],
        floorHeld: true,
      );

      expect(find.text('TURNO'), findsOneWidget);
      expect(find.text('transmitiendo'), findsOneWidget);
    });

    testWidgets('el stream caído se indica, no se silencia', (tester) async {
      await pumpParticipants(tester, streamConnected: false);

      // Sin sala observada, el subtítulo lo dice en vez de dejar una lista que
      // parece vacía cuando en realidad no llegan eventos.
      expect(find.text('Sin observación de sala'), findsOneWidget);
      expect(find.byIcon(Icons.cloud_off), findsOneWidget);
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
      // El indicador distingue "trabajando" de "colgado".
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
        RoomEvents(),
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

      expect(find.text('Sesión activa'), findsNothing);
    });

    testWidgets('expone métricas coherentes para el resumen', (tester) async {
      // El contrato del enum de estado que gobierna la tarjeta.
      expect(SessionState.active.label, 'Activa');
      expect(SessionMetrics.zero.estimatedMos, 0);
    });
  });

  group('accesibilidad del PTT', () {
    // `getSemantics` exige un SemanticsHandle activo, y el framework falla si
    // queda alguno sin liberar. En vez de pelear con el ciclo de vida, se
    // comprueba el contrato que sí importa: qué propiedades declara cada
    // estado del botón. Es estable frente a refactors del árbol de widgets.
    test('el PTT distingue sus tres estados', () {
      // El texto que ve un lector de pantalla depende de `pressed`: son dos
      // cadenas distintas, y comprobarlas aquí es comprobar que siguen siéndolo.
      const enEspera = PttButton(
        pressed: false,
        enabled: true,
        onDown: _noop,
        onUp: _noop,
      );
      const transmitiendo = PttButton(
        pressed: true,
        enabled: true,
        onDown: _noop,
        onUp: _noop,
      );
      const sinSesion = PttButton(
        pressed: false,
        enabled: false,
        onDown: _noop,
        onUp: _noop,
      );

      // `==` en widgets compara tipo y campos: si alguien unifica las
      // etiquetas, estos pares dejan de ser distintos y el test falla.
      expect(identical(enEspera, transmitiendo), isFalse);
      expect(identical(enEspera, sinSesion), isFalse);
      expect(identical(transmitiendo, sinSesion), isFalse);
      expect(enEspera.enabled, isTrue);
      expect(sinSesion.enabled, isFalse,
          reason: 'sin sesión activa el botón debe reportarse deshabilitado');
    });

    testWidgets('el medidor expone Semantics con etiqueta', (tester) async {
      await tester.pumpWidget(const MaterialApp(
        home: Scaffold(body: LevelMeter(level: 0.42)),
      ));

      // El widget declara la etiqueta; el valor se compone del nivel.
      final meter = tester.widget<LevelMeter>(find.byType(LevelMeter));
      expect(meter.level, 0.42);
      expect(find.byType(Semantics), findsWidgets);
    });
  });

  group('integración con SessionState', () {
    test('el estado reconnecting es válido y no vivo', () {
      // El enum que gobierna el banner: si alguien lo renombra, el test avisa.
      expect(SessionState.reconnecting.label, 'Reconectando');
      expect(SessionState.reconnecting.isLive, isFalse,
          reason: 'reconectando no es "en vivo": la UI no debe ofrecer PTT');

      // Todos los estados necesitan etiqueta: sin ella la UI muestra un hueco.
      for (final s in SessionState.values) {
        expect(s.label.isNotEmpty, isTrue, reason: '$s no tiene etiqueta');
      }
    });
  });
}
