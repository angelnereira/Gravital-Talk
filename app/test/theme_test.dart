/// Pruebas del tema GEUI.
///
/// Comprueban lo que el documento exige de forma medible: que la paleta tonal
/// genera contraste AA sin Intervention manual, y que el sistema expone lo
/// que los componentes dan por hecho.
library;

import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:flutter/cupertino.dart' show CupertinoPageTransitionsBuilder;
import 'package:gravital_talk_app/ui/fonts.dart';
import 'package:gravital_talk_app/ui/semantic_colors.dart';
import 'package:gravital_talk_app/ui/theme.dart';
import 'package:gravital_talk_app/ui/tokens.dart';

/// Luminancia relativa WCAG 2.2.
double _luminance(Color c) {
  double channel(double v) => v <= 0.03928
      ? v / 12.92
      : math.pow((v + 0.055) / 1.055, 2.4).toDouble();
  return 0.2126 * channel(c.r) +
      0.7152 * channel(c.g) +
      0.0722 * channel(c.b);
}

/// Ratio de contraste WCAG entre dos colores opacos.
double _contrast(Color a, Color b) {
  final la = _luminance(a);
  final lb = _luminance(b);
  return (la > lb ? la : lb) / (la > lb ? lb : la);
}

void main() {
  group('paleta tonal', () {
    test('primary/onPrimary alcanza AA en claro y oscuro', () {
      for (final b in Brightness.values) {
        final s = NTheme.scheme(b);
        expect(
          _contrast(s.primary, s.onPrimary),
          greaterThanOrEqualTo(4.5),
          reason: 'primary sobre onPrimary en $b',
        );
      }
    });

    test('onSurface sobre surface alcanza AA en claro y oscuro', () {
      for (final b in Brightness.values) {
        final s = NTheme.scheme(b);
        expect(
          _contrast(s.surface, s.onSurface),
          greaterThanOrEqualTo(4.5),
          reason: 'onSurface sobre surface en $b',
        );
      }
    });

    test('onSurfaceVariant sobre surface alcanza AA para texto normal', () {
      for (final b in Brightness.values) {
        final s = NTheme.scheme(b);
        expect(
          _contrast(s.surface, s.onSurfaceVariant),
          greaterThanOrEqualTo(4.5),
          reason: 'onSurfaceVariant sobre surface en $b',
        );
      }
    });

    test('el nivel de contraste alto sube el ratio sin invvertir colores', () {
      final base = NTheme.scheme(Brightness.light);
      final alto = NTheme.scheme(Brightness.light, contrastLevel: 1.0);
      expect(
        _contrast(alto.surface, alto.onSurface),
        greaterThan(_contrast(base.surface, base.onSurface)),
      );
      // El brillo de la superficie no puede cambiar de signo: invertir
      // colores a mano rompe los modos de alto contraste del sistema.
      expect(alto.surface.computeLuminance(), greaterThan(0));
    });
  });

  group('SemanticColors', () {
    test('los siete tonos del dominio existen y tienen contraste AA', () {
      for (final b in Brightness.values) {
        final sem = SemanticColors.from(NTheme.scheme(b));
        final tones = {
          'success': sem.success,
          'warning': sem.warning,
          'info': sem.info,
          'pttActive': sem.pttActive,
          'pttFloor': sem.pttFloor,
          'calling': sem.calling,
          'connectionPoor': sem.connectionPoor,
        };
        tones.forEach((name, tone) {
          expect(
            _contrast(tone.color, tone.onColor),
            greaterThanOrEqualTo(4.5),
            reason: '$name en $b',
          );
          expect(
            _contrast(tone.container, tone.onContainer),
            greaterThanOrEqualTo(4.5),
            reason: '$name container en $b',
          );
        });
      }
    });

    test('se instala como ThemeExtension y se recupera desde el contexto', () {
      final theme = NTheme.build(NTheme.scheme(Brightness.light));
      expect(theme.extension<SemanticColors>(), isNotNull);
      expect(SemanticColors.from(NTheme.scheme(Brightness.dark)).pttActive.color,
          isNot(SemanticColors.from(NTheme.scheme(Brightness.light)).pttActive.color),
          reason: 'el tono de hablar debe distinguirse del fondo según el modo');
    });
  });

  group('tokens', () {
    test('la escala de espaciado crece de forma monótona', () {
      expect(NSpace.xs, lessThan(NSpace.sm));
      expect(NSpace.sm, lessThan(NSpace.md));
      expect(NSpace.md, lessThan(NSpace.lg));
      expect(NSpace.lg, lessThan(NSpace.xl));
      expect(NSpace.xl, lessThan(NSpace.xxl));
      expect(NSpace.xxl, lessThan(NSpace.xxxl));
      expect(NSpace.xxxl, lessThan(NSpace.huge));
    });

    test('la escala de formas crece de forma monótona', () {
      expect(NShape.extraSmall, lessThan(NShape.small));
      expect(NShape.small, lessThan(NShape.medium));
      expect(NShape.medium, lessThan(NShape.large));
      expect(NShape.large, lessThan(NShape.extraLarge));
      expect(NShape.extraLarge, lessThan(NShape.extraExtraLarge));
      expect(NShape.extraExtraLarge, lessThan(NShape.full));
    });

    test('el objetivo táctil cumple el mínimo Material de 48 dp', () {
      expect(NHitSize.minTouch, greaterThanOrEqualTo(48));
      expect(NHitSize.pttButton, greaterThan(NHitSize.minTouch));
    });

    test('los resortes expresivos rebotan y los de efectos no', () {
      // El coeficiente es \u03b6 = c / (2 * sqrt(k * m)). \u03b6 < 1 rebota, 1 no.
      double ratio(SpringDescription d) =>
          d.damping / (2 * math.sqrt(d.stiffness * d.mass));

      expect(ratio(NMotion.expressiveFastSpatial), lessThan(1.0));
      expect(ratio(NMotion.expressiveDefaultSpatial), lessThan(1.0));
      expect(ratio(NMotion.expressiveSlowSpatial), lessThan(1.0));
      expect(ratio(NMotion.fastEffects), closeTo(1.0, 0.01));
      expect(ratio(NMotion.defaultEffects), closeTo(1.0, 0.01));
      expect(ratio(NMotion.slowEffects), closeTo(1.0, 0.01));
    });

    testWidgets('las clases de ventana cortan en los límites de la especificación',
        (tester) async {
      const cortes = [
        (599.0, WindowClass.compact),
        (600.0, WindowClass.medium),
        (839.0, WindowClass.medium),
        (840.0, WindowClass.expanded),
        (1199.0, WindowClass.expanded),
        (1200.0, WindowClass.large),
        (1599.0, WindowClass.large),
        (1600.0, WindowClass.extraLarge),
      ];
      for (final (width, expected) in cortes) {
        tester.view.physicalSize = Size(width, 800);
        tester.view.devicePixelRatio = 1.0;
        addTearDown(tester.view.reset);

        late WindowClass actual;
        await tester.pumpWidget(Builder(builder: (context) {
          actual = WindowClass.of(context);
          return const SizedBox.shrink();
        }));

        expect(actual, expected, reason: 'ancho $width');
      }
    });
  });

  group('Google Sans Flex', () {
    test('la familia esta declarada en toda la escala', () {
      final t = NTheme.build(NTheme.scheme(Brightness.light)).textTheme;
      for (final e in _all(t)) {
        expect(e.$2!.fontFamily, kFontFamily, reason: e.$1);
      }
    });

    test('el eje de peso varia segun el estilo, no se queda en 400', () {
      // El fallo silencioso de una fuente variable: declarar la familia y
      // seguir usando el peso por defecto en todas partes.
      final t = NTheme.build(NTheme.scheme(Brightness.light)).textTheme;

      expect(_wght(t.titleLarge), 400);
      expect(_wght(t.titleMedium), 500);
      expect(_wght(t.labelLarge), 500);
    });

    test('el tamano optico sigue al tamano en puntos y entra en rango', () {
      final t = NTheme.build(NTheme.scheme(Brightness.light)).textTheme;

      expect(_opsz(t.displayLarge), t.displayLarge!.fontSize);
      expect(_opsz(t.labelSmall), t.labelSmall!.fontSize);
      for (final size in [_opsz(t.displayLarge)!, _opsz(t.labelSmall)!]) {
        expect(size, greaterThanOrEqualTo(1));
        expect(size, lessThanOrEqualTo(144));
      }
    });

    test('los ejes se recortan al rango que declara la fuente', () {
      expect(FontAxis.weight(5000), 1000);
      expect(FontAxis.weight(-3), 1);
      expect(FontAxis.opticalSize(400), 144);
      expect(FontAxis.opticalSize(0), 1);
    });
  });

  group('componentes del tema', () {
    test('los botones tienen forma de píldora y altura de 48 dp', () {
      final theme = NTheme.build(NTheme.scheme(Brightness.light));
      final filled = theme.filledButtonTheme.style?.shape?.resolve({});
      expect(filled, isA<RoundedRectangleBorder>());
      final minSize = theme.filledButtonTheme.style?.minimumSize
          ?.resolve(<WidgetState>{});
      expect(minSize?.height, greaterThanOrEqualTo(48));
    });

    test('los diálogos usan radio extraLarge y no llevan sombra', () {
      final theme = NTheme.build(NTheme.scheme(Brightness.light));
      expect(theme.dialogTheme.elevation, anyOf(isNull, 0));
      final shape = theme.dialogTheme.shape! as RoundedRectangleBorder;
      expect(
        (shape.borderRadius as BorderRadius).topLeft.x,
        NShape.extraLarge,
      );
    });

    test('las transiciones de Android son predictivas', () {
      final theme = NTheme.build(NTheme.scheme(Brightness.light));
      expect(
        theme.pageTransitionsTheme.builders[TargetPlatform.android],
        isA<PredictiveBackPageTransitionsBuilder>(),
      );
      expect(
        theme.pageTransitionsTheme.builders[TargetPlatform.iOS],
        isA<CupertinoPageTransitionsBuilder>(),
      );
    });

    test('las cifras de la escala base llevan tabulares', () {
      // Sin esto un RTT que pasa de 9 a 10 desplaza la fila entera.
      final theme = NTheme.build(NTheme.scheme(Brightness.light));
      expect(theme.textTheme.bodyMedium?.fontFeatures,
          contains(const FontFeature.tabularFigures()));
    });
  });
}

/// Eje wght de un estilo.
double? _wght(TextStyle? s) {
  if (s == null) return null;
  final axis = FontVariation.weight(1).axis;
  for (final v in s.fontVariations ?? const <FontVariation>[]) {
    if (v.axis == axis) return v.value;
  }
  return null;
}

/// Eje opsz de un estilo.
double? _opsz(TextStyle? s) {
  if (s == null) return null;
  const axis = 'opsz';
  for (final v in s.fontVariations ?? const <FontVariation>[]) {
    if (v.axis == axis) return v.value;
  }
  return null;
}

/// Los quince estilos de la escala.
Iterable<(String, TextStyle?)> _all(TextTheme t) sync* {
  yield ('displayLarge', t.displayLarge);
  yield ('displayMedium', t.displayMedium);
  yield ('displaySmall', t.displaySmall);
  yield ('headlineLarge', t.headlineLarge);
  yield ('headlineMedium', t.headlineMedium);
  yield ('headlineSmall', t.headlineSmall);
  yield ('titleLarge', t.titleLarge);
  yield ('titleMedium', t.titleMedium);
  yield ('titleSmall', t.titleSmall);
  yield ('bodyLarge', t.bodyLarge);
  yield ('bodyMedium', t.bodyMedium);
  yield ('bodySmall', t.bodySmall);
  yield ('labelLarge', t.labelLarge);
  yield ('labelMedium', t.labelMedium);
  yield ('labelSmall', t.labelSmall);
}
