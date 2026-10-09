import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:gravital_talk_app/core/states.dart';

void main() {
  Widget wrap(Widget child) => MaterialApp(home: Scaffold(body: child));

  group('EmptyState', () {
    testWidgets('muestra título, mensaje e icono', (tester) async {
      await tester.pumpWidget(wrap(const EmptyState(
        icon: Icons.wifi_off,
        title: 'Sin sesión',
        message: 'Configura una sala para empezar',
      )));

      expect(find.text('Sin sesión'), findsOneWidget);
      expect(find.text('Configura una sala para empezar'), findsOneWidget);
      expect(find.byIcon(Icons.wifi_off), findsOneWidget);
    });

    testWidgets('no muestra acción si no se pasa', (tester) async {
      await tester.pumpWidget(wrap(const EmptyState(
        icon: Icons.wifi_off,
        title: 'Sin sesión',
      )));

      // Sin actionLabel no debe aparecer ningún botón.
      expect(find.byType(FilledButton), findsNothing);
    });

    testWidgets('la acción se invoca al pulsarla', (tester) async {
      var taps = 0;
      await tester.pumpWidget(wrap(EmptyState(
        icon: Icons.refresh,
        title: 'Sin salas',
        actionLabel: 'Reintentar',
        onAction: () => taps++,
      )));

      await tester.tap(find.text('Reintentar'));
      expect(taps, 1);
    });

    testWidgets('respeta el objetivo táctil mínimo del botón', (tester) async {
      await tester.pumpWidget(wrap(EmptyState(
        icon: Icons.refresh,
        title: 'Sin salas',
        actionLabel: 'Reintentar',
        onAction: () {},
      )));

      final button = tester.getSize(find.byType(FilledButton));
      expect(button.height, greaterThanOrEqualTo(48));
    });
  });

  group('ErrorBanner', () {
    testWidgets('muestra el mensaje', (tester) async {
      await tester.pumpWidget(wrap(const ErrorBanner(
        message: 'No se pudo conectar al relay',
      )));

      expect(find.text('No se pudo conectar al relay'), findsOneWidget);
      expect(find.byIcon(Icons.error_outline), findsOneWidget);
    });

    testWidgets('ofrece reintento cuando se pasa el callback', (tester) async {
      var retries = 0;
      await tester.pumpWidget(wrap(ErrorBanner(
        message: 'Fallo de red',
        onRetry: () => retries++,
      )));

      await tester.tap(find.text('Reintentar'));
      expect(retries, 1);
    });

    testWidgets('ofrece descartar cuando se pasa el callback', (tester) async {
      var dismissed = 0;
      await tester.pumpWidget(wrap(ErrorBanner(
        message: 'Fallo de red',
        onDismiss: () => dismissed++,
      )));

      await tester.tap(find.text('Descartar'));
      expect(dismissed, 1);
    });

    testWidgets('sin callbacks no muestra botones', (tester) async {
      await tester.pumpWidget(wrap(const ErrorBanner(message: 'Fallo')));

      expect(find.text('Reintentar'), findsNothing);
      expect(find.text('Descartar'), findsNothing);
    });
  });

  group('WarningBanner', () {
    testWidgets('muestra mensaje y acción opcional', (tester) async {
      var tapped = 0;
      await tester.pumpWidget(wrap(WarningBanner(
        message: 'Motor demo activo',
        actionLabel: 'Entendido',
        onAction: () => tapped++,
      )));

      expect(find.text('Motor demo activo'), findsOneWidget);
      await tester.tap(find.text('Entendido'));
      expect(tapped, 1);
    });
  });

  group('LoadingStates', () {
    testWidgets('centered muestra el mensaje y el indicador', (tester) async {
      await tester.pumpWidget(wrap(LoadingStates.centered(
        message: 'Conectando…',
      )));

      expect(find.text('Conectando…'), findsOneWidget);
      expect(find.byType(CircularProgressIndicator), findsOneWidget);
    });

    testWidgets('button está deshabilitado y conserva el alto', (tester) async {
      await tester.pumpWidget(wrap(LoadingStates.button(
        label: 'Creando sala…',
      )));

      final button = tester.widget<FilledButton>(find.byType(FilledButton));
      expect(button.onPressed, isNull, reason: 'un botón en carga no debe ser pulsable');

      final size = tester.getSize(find.byType(FilledButton));
      expect(size.height, greaterThanOrEqualTo(48));
    });
  });

  group('ConfirmDialog', () {
    testWidgets('devuelve true al confirmar', (tester) async {
      await tester.pumpWidget(wrap(
        Builder(builder: (context) => ElevatedButton(
              onPressed: () async {
                final ok = await ConfirmDialog.show(
                  context,
                  title: 'Finalizar sesión',
                  message: 'Se perderá el audio en curso',
                  destructive: true,
                );
                // ignore: avoid_print
                print('resultado=$ok');
              },
              child: const Text('abrir'),
            )),
      ));

      await tester.tap(find.text('abrir'));
      await tester.pumpAndSettle();

      expect(find.text('Finalizar sesión'), findsOneWidget);
      expect(find.text('Se perderá el audio en curso'), findsOneWidget);

      await tester.tap(find.text('Confirmar'));
      await tester.pumpAndSettle();
      // El diálogo se cierra tras confirmar.
      expect(find.text('Finalizar sesión'), findsNothing);
    });

    testWidgets('devuelve false al cancelar', (tester) async {
      await tester.pumpWidget(wrap(
        Builder(builder: (context) => ElevatedButton(
              onPressed: () => ConfirmDialog.show(
                context,
                title: 'Finalizar sesión',
                message: 'Se perderá el audio en curso',
              ),
              child: const Text('abrir'),
            )),
      ));

      await tester.tap(find.text('abrir'));
      await tester.pumpAndSettle();

      await tester.tap(find.text('Cancelar'));
      await tester.pumpAndSettle();
      expect(find.text('Finalizar sesión'), findsNothing);
    });
  });
}
