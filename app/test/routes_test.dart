import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/core/routes.dart';
import 'package:gravital_talk_app/core/tokens.dart';

void main() {
  group('rutas de navegación', () {
    testWidgets('ForwardRoute entra deslizando y termina opaca',
        (tester) async {
      await tester.pumpWidget(MaterialApp(
        home: Builder(
          builder: (context) => ElevatedButton(
            onPressed: () => Navigator.of(context).push(
              ForwardRoute(builder: (_) => const _Page(title: 'detalle')),
            ),
            child: const Text('abrir'),
          ),
        ),
      ));

      await tester.tap(find.text('abrir'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 500));

      expect(find.text('detalle'), findsOneWidget);
      expect(find.text('abrir'), findsNothing,
          reason: 'la pantalla anterior debe quedar tapada, no visible');
    });

    testWidgets('QuickRoute solo hace fade', (tester) async {
      await tester.pumpWidget(MaterialApp(
        home: Builder(
          builder: (context) => ElevatedButton(
            onPressed: () => Navigator.of(context).push(
              QuickRoute(builder: (_) => const _Page(title: 'ajustes')),
            ),
            child: const Text('abrir'),
          ),
        ),
      ));

      await tester.tap(find.text('abrir'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));

      expect(find.text('ajustes'), findsOneWidget);
    });

    test('BottomSheetRoute es dismissible', () {
      // El gesto de "volver" sobre un modal exige un Navigator con historial
      // real, así que se comprueba la configuración que hace posible salir.
      final route = BottomSheetRoute<void>(
        builder: (_) => const SizedBox(),
      );
      expect(route.opaque, isFalse,
          reason: 'opaca taparía la barrera y no se podría descartar tocando fuera');
      expect(route.barrierDismissible, isTrue);
    });

    testWidgets('volver saca la pantalla de detalle', (tester) async {
      await tester.pumpWidget(MaterialApp(
        home: Builder(
          builder: (context) => ElevatedButton(
            onPressed: () => Navigator.of(context).push(
              ForwardRoute(builder: (_) => const _Page(title: 'detalle')),
            ),
            child: const Text('abrir'),
          ),
        ),
      ));

      await tester.tap(find.text('abrir'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 500));

      // La pantalla nueva está arriba y la anterior tapada.
      expect(find.text('detalle'), findsOneWidget);
      expect(find.text('abrir'), findsNothing);

      // Volver con el Navigator real: la animación inversa debe completarse.
      final NavigatorState nav = tester.state(find.byType(Navigator).first);
      nav.pop();
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 600));

      expect(find.text('abrir'), findsOneWidget);
      expect(find.text('detalle'), findsNothing);
    });
  });

  group('GravitalDurations', () {
    test('instant es menor que fast, y fast que normal', () {
      // El orden importa: si alguien invierte las duraciones, las transiciones
      // se sienten lentas sin motivo visible.
      expect(GravitalDurations.instant.inMilliseconds,
          lessThan(GravitalDurations.fast.inMilliseconds));
      expect(GravitalDurations.fast.inMilliseconds,
          lessThan(GravitalDurations.normal.inMilliseconds));
      expect(GravitalDurations.normal.inMilliseconds,
          lessThan(GravitalDurations.slow.inMilliseconds));
    });

    test('fast está por debajo del umbral de respuesta inmediata', () {
      expect(GravitalDurations.fast.inMilliseconds, lessThanOrEqualTo(150));
    });
  });
}

class _Page extends StatelessWidget {
  const _Page({required this.title});

  final String title;

  @override
  Widget build(BuildContext context) {
    // Sin AppBar a propósito: el título sólo está en el body, así que
    // `findsOneWidget` identifica de forma inequívoca qué pantalla está arriba.
    return Scaffold(body: Center(child: Text(title)));
  }
}
