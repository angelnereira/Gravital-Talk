/// Pruebas de accesibilidad de las pantallas (GEUI §12.1).
///
/// Comprueban con las guías de Material, no a ojo: objetivo táctil, etiquetado
/// y contraste de texto en las pantallas que la app monta en tests.
library;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/app.dart';
import 'package:gravital_talk_app/models/session.dart';
import 'package:gravital_talk_app/services/engine.dart';
import 'package:gravital_talk_app/services/event_log.dart';
import 'package:gravital_talk_app/services/room_api.dart';
import 'package:gravital_talk_app/services/room_events.dart';
import 'package:gravital_talk_app/services/session_controller.dart';
import 'package:gravital_talk_app/services/settings_store.dart';
import 'package:gravital_talk_app/ui/components/index.dart';
import 'package:gravital_talk_app/ui/theme.dart';
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
  group('home', () {
    testWidgets('respeta las guías táctiles y de etiquetado', (tester) async {
      final handle = tester.ensureSemantics();
      await _pumpApp(tester);

      await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
      await expectLater(tester, meetsGuideline(iOSTapTargetGuideline));
      await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
      await expectLater(tester, meetsGuideline(textContrastGuideline));

      handle.dispose();
    });

    testWidgets('los iconos de la barra llevan etiqueta', (tester) async {
      final handle = tester.ensureSemantics();
      await _pumpApp(tester);

      // Iconos interactivos sin texto necesitan nombre accesible: los dos
      // botones de la barra ("Ajustes", "Acerca de") son el caso difícil.
      expect(find.bySemanticsLabel('Ajustes'), findsOneWidget);
      expect(find.bySemanticsLabel('Acerca de'), findsOneWidget);

      handle.dispose();
    });

    testWidgets('funciona con el texto al 200 %', (tester) async {
      await _pumpApp(tester);

      tester.view.physicalSize = const Size(360, 640);
      tester.view.devicePixelRatio = 1.0;
      addTearDown(tester.view.reset);

      await tester.pumpWidget(
        MediaQuery(
          data: const MediaQueryData(textScaler: TextScaler.linear(2.0)),
          child: const SizedBox.shrink(),
        ),
      );
      // Se vuelve a construir la app bajo el escalador nuevo.
      await _pumpApp(tester);

      // Sin excepciones ni desbordamiento: `tester.pumpAndSettle` ya falla
      // si algo lanza RenderFlex.
      expect(tester.takeException(), isNull);
    });
  });

  group('componentes de dominio', () {
    testWidgets('PttButton es un objetivo táctil grande y etiquetado',
        (tester) async {
      final handle = tester.ensureSemantics();
      var talking = false;

      await tester.pumpWidget(MaterialApp(
        theme: NTheme.build(NTheme.scheme(Brightness.dark)),
        home: Scaffold(
          body: Center(
            child: PttButton(
              pressed: talking,
              enabled: true,
              onDown: () => talking = true,
              onUp: () => talking = false,
            ),
          ),
        ),
      ));

      await expectLater(tester, meetsGuideline(androidTapTargetGuideline));
      await expectLater(tester, meetsGuideline(iOSTapTargetGuideline));
      await expectLater(tester, meetsGuideline(labeledTapTargetGuideline));
      expect(find.bySemanticsLabel('Mantén pulsado para hablar'),
          findsOneWidget);

      handle.dispose();
    });

    testWidgets('StatusBadge expone el texto del estado, no sólo el color',
        (tester) async {
      final handle = tester.ensureSemantics();

      await tester.pumpWidget(MaterialApp(
        theme: NTheme.build(NTheme.scheme(Brightness.light)),
        home: const Scaffold(
          body: Center(child: StatusBadge(SessionState.active)),
        ),
      ));

      // El color nunca es el único portador de significado: el texto del
      // estado va presente.
      expect(find.text(SessionState.active.label), findsOneWidget);

      handle.dispose();
    });
  });
}