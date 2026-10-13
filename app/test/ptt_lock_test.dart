import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';

import 'package:gravital_talk_app/core/theme.dart';
import 'package:gravital_talk_app/widgets/common.dart';

/// Botón de hablar con un "motor" mínimo: `pressed` sigue a onDown/onUp.
class _Harness extends StatefulWidget {
  const _Harness();

  @override
  State<_Harness> createState() => _HarnessState();
}

class _HarnessState extends State<_Harness> {
  bool talking = false;
  int ups = 0;

  @override
  Widget build(BuildContext context) => Center(
        child: PttButton(
          pressed: talking,
          enabled: true,
          onDown: () => setState(() => talking = true),
          onUp: () => setState(() {
            talking = false;
            ups++;
          }),
        ),
      );
}

void main() {
  Future<_HarnessState> pump(WidgetTester tester) async {
    await tester.pumpWidget(MaterialApp(
      theme: GravitalTheme.dark(),
      home: const Scaffold(body: _Harness()),
    ));
    return tester.state<_HarnessState>(find.byType(_Harness));
  }

  testWidgets('soltar sin deslizar corta el turno', (tester) async {
    final h = await pump(tester);
    final g = await tester.startGesture(tester.getCenter(find.byType(PttButton)));
    await tester.pump();
    expect(h.talking, isTrue);
    await g.up();
    await tester.pump();
    expect(h.talking, isFalse);
  });

  testWidgets('deslizar hacia arriba bloquea el turno y un toque lo suelta',
      (tester) async {
    final h = await pump(tester);
    final center = tester.getCenter(find.byType(PttButton));
    final g = await tester.startGesture(center);
    await tester.pump();
    await g.moveBy(const Offset(0, -(pttLockDistance + 10)));
    await tester.pump();
    await g.up();
    await tester.pump();
    expect(h.talking, isTrue, reason: 'bloqueado, sigue hablando sin el dedo');
    expect(find.textContaining('BLOQUEADO'), findsOneWidget);

    await tester.tapAt(center);
    await tester.pump();
    expect(h.talking, isFalse);
    expect(h.ups, 1);

    // El toque que suelta no abre otro turno.
    await tester.pump(const Duration(milliseconds: 50));
    expect(h.talking, isFalse);
  });
}
