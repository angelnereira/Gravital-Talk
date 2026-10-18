/// Tema Gravital Experience UI (GEUI).
///
/// Construye ThemeData desde tokens M3 Expressive: color HCT, escala de
/// formas, tipografía, contención tonal y movimiento por resortes.
library;

import 'package:flutter/cupertino.dart' show CupertinoPageTransitionsBuilder;
import 'package:flutter/material.dart';

import 'fonts.dart';
import 'semantic_colors.dart';
import 'tokens.dart';

/// Escrituras que Google Sans Flex no cubre (chino, árabe, tailandés…).
/// Sin esto Dart eliges una fuente del sistema por su cuenta y el resultado
/// cambia entre plataformas.
const List<String> _fallback = <String>[
  'Roboto Flex',
  'Noto Sans',
  '.SF Pro Text',
];

/// Tema principal de la app.
/// Un estilo de la escala tipográfica.
///
/// Los tamaños van aquí, explícitos, y no se dejan al tema de Flutter.
///
/// La razón es concreta: en Flutter 3.47 `ThemeData.textTheme` devuelve los
/// estilos con `fontSize` nulo — el tamaño se resuelve más tarde, al renderizar.
/// Con el tamaño en nulo no hay forma de mandar el eje óptico `opsz` de Google
/// Sans Flex, que es justo lo que hace que una misma variable se dibuje bien a
/// 11 y a 57 dp. Declarar la escala (sección 5.2 del documento GEUI) devuelve el
/// tamaño al tema y hace el eje posible.
@immutable
class _Style {
  const _Style(this.size, this.lineHeight, this.weight, this.tracking);

  /// Tamaño en puntos lógicos.
  final double size;

  /// Interlineado, en múltiplos del tamaño.
  final double lineHeight;

  final FontWeight weight;

  /// Tracking en sp; negativo en los títulos grandes, como en la escala M3.
  final double tracking;
}

/// La escala oficial de Material 3 (GEUI §5.2).
const List<(String, _Style)> _scale = [
  ('displayLarge', _Style(57, 64 / 57, FontWeight.w400, -0.5)),
  ('displayMedium', _Style(45, 52 / 45, FontWeight.w400, -0.4)),
  ('displaySmall', _Style(36, 44 / 36, FontWeight.w400, -0.3)),
  ('headlineLarge', _Style(32, 40 / 32, FontWeight.w400, -0.4)),
  ('headlineMedium', _Style(28, 36 / 28, FontWeight.w400, -0.3)),
  ('headlineSmall', _Style(24, 32 / 24, FontWeight.w400, -0.2)),
  ('titleLarge', _Style(22, 28 / 22, FontWeight.w400, -0.1)),
  ('titleMedium', _Style(16, 24 / 16, FontWeight.w500, 0.1)),
  ('titleSmall', _Style(14, 20 / 14, FontWeight.w500, 0.1)),
  ('bodyLarge', _Style(16, 24 / 16, FontWeight.w400, 0.5)),
  ('bodyMedium', _Style(14, 20 / 14, FontWeight.w400, 0.25)),
  ('bodySmall', _Style(12, 16 / 12, FontWeight.w400, 0.4)),
  ('labelLarge', _Style(14, 20 / 14, FontWeight.w500, 0.1)),
  ('labelMedium', _Style(12, 16 / 12, FontWeight.w500, 0.5)),
  ('labelSmall', _Style(11, 16 / 11, FontWeight.w500, 0.5)),
];

abstract final class NTheme {
  /// Color semilla de marca Gravital (teal de "señal").
  static const Color brandSeed = Color(0xFF0D9488);

  /// Construye un ThemeData completo desde un ColorScheme.
  static ThemeData build(ColorScheme scheme) {
    final base = ThemeData(colorScheme: scheme);

    return base.copyWith(
      brightness: scheme.brightness,
      colorScheme: scheme,
      scaffoldBackgroundColor: scheme.surface,

      // ── Tipografía ──────────────────────────────────────────────────────
      textTheme: _buildTextTheme(base.textTheme, scheme),

      // ── Componentes Material ─────────────────────────────────────────────
      appBarTheme: AppBarTheme(
        centerTitle: false,
        elevation: 0,
        scrolledUnderElevation: 0,
        backgroundColor: Colors.transparent,
        foregroundColor: scheme.onSurface,
        surfaceTintColor: Colors.transparent,
      ),

      cardTheme: CardThemeData(
        elevation: 0,
        margin: EdgeInsets.zero,
        color: scheme.surfaceContainerLow,
        shape: NShape.rounded(NShape.medium),
      ),

      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: scheme.surfaceContainerHighest.withValues(alpha: 0.5),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(NShape.extraSmall),
          borderSide: BorderSide(color: scheme.outlineVariant),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(NShape.extraSmall),
          borderSide: BorderSide(color: scheme.outlineVariant),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(NShape.extraSmall),
          borderSide: BorderSide(color: scheme.primary, width: 2),
        ),
        errorBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(NShape.extraSmall),
          borderSide: BorderSide(color: scheme.error),
        ),
        focusedErrorBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(NShape.extraSmall),
          borderSide: BorderSide(color: scheme.error, width: 2),
        ),
      ),

      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          minimumSize: const Size.fromHeight(NHitSize.minTouch),
          shape: NShape.rounded(NShape.full),
        ),
      ),

      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          minimumSize: const Size.fromHeight(NHitSize.minTouch),
          side: BorderSide(color: scheme.outline),
          shape: NShape.rounded(NShape.full),
        ),
      ),

      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          minimumSize: const Size(NHitSize.minTouch, NHitSize.minTouch),
          shape: NShape.rounded(NShape.full),
        ),
      ),

      navigationBarTheme: NavigationBarThemeData(
        backgroundColor: scheme.surfaceContainer,
        indicatorColor: scheme.secondaryContainer,
        labelBehavior: NavigationDestinationLabelBehavior.alwaysShow,
      ),

      listTileTheme: ListTileThemeData(
        shape: NShape.rounded(NShape.medium),
        minVerticalPadding: NSpace.sm,
      ),

      snackBarTheme: SnackBarThemeData(
        behavior: SnackBarBehavior.floating,
        backgroundColor: scheme.inverseSurface,
        contentTextStyle: TextStyle(color: scheme.onInverseSurface),
        shape: NShape.rounded(NShape.medium),
      ),

      dividerTheme: DividerThemeData(
        color: scheme.outlineVariant,
        space: NSpace.lg,
        thickness: 1,
      ),

      bottomSheetTheme: BottomSheetThemeData(
        backgroundColor: scheme.surfaceContainerLow,
        showDragHandle: true,
        shape: const RoundedRectangleBorder(
          borderRadius: BorderRadius.vertical(
            top: Radius.circular(NShape.extraLarge),
          ),
        ),
      ),

      dialogTheme: DialogThemeData(
        backgroundColor: scheme.surfaceContainerHigh,
        shape: NShape.rounded(NShape.extraLarge),
      ),

      // ── Transiciones de página ───────────────────────────────────────────
      pageTransitionsTheme: PageTransitionsTheme(
        builders: const {
          TargetPlatform.android: PredictiveBackPageTransitionsBuilder(),
          TargetPlatform.iOS: CupertinoPageTransitionsBuilder(),
          TargetPlatform.macOS: CupertinoPageTransitionsBuilder(),
        },
      ),

      // ── Splash e interacción ─────────────────────────────────────────────
      splashColor: scheme.primary.withValues(alpha: 0.08),
      highlightColor: scheme.primary.withValues(alpha: 0.04),

      // ── Extensiones ──────────────────────────────────────────────────────
      extensions: [SemanticColors.from(scheme)],
    );
  }

  /// Genera el esquema de color desde la semilla de marca.
  static ColorScheme scheme(Brightness brightness, {double contrastLevel = 0}) {
    return ColorScheme.fromSeed(
      seedColor: brandSeed,
      brightness: brightness,
      contrastLevel: contrastLevel,
    );
  }

  static TextTheme _buildTextTheme(TextTheme base, ColorScheme scheme) {
    // Google Sans Flex es un único fichero variable: los ejes los pide el
    // `TextStyle`, no el pubspec (ver lib/ui/fonts.dart). Sin `FontVariation.weight`
    // todo se dibujaría con el peso por defecto y los títulos no se
    // distinguirían del cuerpo.
    TextStyle? tune(TextStyle? s, _Style spec) {
      if (s == null) return null;
      return s.copyWith(
        fontFamily: kFontFamily,
        fontFamilyFallback: _fallback,
        fontSize: spec.size,
        height: spec.lineHeight,
        fontWeight: spec.weight,
        // Cifras tabulares en toda la escala: el RTT, el jitter y la duración de
        // la conversación se actualizan varias veces por segundo, y sin esto un
        // "9" que pasa a "10" desplaza la fila entera.
        fontFeatures: const [FontFeature.tabularFigures()],
        letterSpacing: spec.tracking,
        fontVariations: [
          FontVariation.weight(FontAxis.weight(spec.weight.value.toDouble())),
          FontVariation('opsz', FontAxis.opticalSize(spec.size)),
          kRoundnessText,
        ],
      );
    }

    return TextTheme(
      displayLarge: tune(base.displayLarge, _scale[0].$2),
      displayMedium: tune(base.displayMedium, _scale[1].$2),
      displaySmall: tune(base.displaySmall, _scale[2].$2),
      headlineLarge: tune(base.headlineLarge, _scale[3].$2),
      headlineMedium: tune(base.headlineMedium, _scale[4].$2),
      headlineSmall: tune(base.headlineSmall, _scale[5].$2),
      titleLarge: tune(base.titleLarge, _scale[6].$2),
      titleMedium: tune(base.titleMedium, _scale[7].$2),
      titleSmall: tune(base.titleSmall, _scale[8].$2),
      bodyLarge: tune(base.bodyLarge, _scale[9].$2),
      bodyMedium: tune(base.bodyMedium, _scale[10].$2),
      bodySmall: tune(base.bodySmall, _scale[11].$2),
      labelLarge: tune(base.labelLarge, _scale[12].$2),
      labelMedium: tune(base.labelMedium, _scale[13].$2),
      labelSmall: tune(base.labelSmall, _scale[14].$2),
    );
  }
}

/// Variantes enfatizadas de la tipografía (no existen en TextTheme).
@immutable
class EmphasizedTextTheme extends ThemeExtension<EmphasizedTextTheme> {
  const EmphasizedTextTheme(this.text);

  final TextTheme text;

  static EmphasizedTextTheme of(BuildContext context) =>
      Theme.of(context).extension<EmphasizedTextTheme>() ??
      EmphasizedTextTheme(Theme.of(context).textTheme);

  @override
  EmphasizedTextTheme copyWith({TextTheme? text}) =>
      EmphasizedTextTheme(text ?? this.text);

  @override
  EmphasizedTextTheme lerp(EmphasizedTextTheme? other, double t) =>
      other == null
      ? this
      : EmphasizedTextTheme(TextTheme.lerp(text, other.text, t));
}

/// Construye un TextTheme enfatizado a partir del normal.
TextTheme emphasizedTheme(TextTheme base) {
  return base.copyWith(
    displayLarge: base.displayLarge?.copyWith(fontWeight: FontWeight.w500),
    displayMedium: base.displayMedium?.copyWith(fontWeight: FontWeight.w500),
    displaySmall: base.displaySmall?.copyWith(fontWeight: FontWeight.w500),
    headlineLarge: base.headlineLarge?.copyWith(fontWeight: FontWeight.w500),
    headlineMedium: base.headlineMedium?.copyWith(fontWeight: FontWeight.w500),
    headlineSmall: base.headlineSmall?.copyWith(fontWeight: FontWeight.w500),
    titleLarge: base.titleLarge?.copyWith(fontWeight: FontWeight.w500),
    titleMedium: base.titleMedium?.copyWith(fontWeight: FontWeight.w700),
    titleSmall: base.titleSmall?.copyWith(fontWeight: FontWeight.w700),
    labelLarge: base.labelLarge?.copyWith(fontWeight: FontWeight.w700),
    labelMedium: base.labelMedium?.copyWith(fontWeight: FontWeight.w700),
    labelSmall: base.labelSmall?.copyWith(fontWeight: FontWeight.w700),
  );
}
