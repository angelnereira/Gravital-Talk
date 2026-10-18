/// Tema Gravital Experience UI (GEUI).
///
/// Construye ThemeData desde tokens M3 Expressive: color HCT, escala de
/// formas, tipografía, contención tonal y movimiento por resortes.
library;

import 'package:flutter/cupertino.dart' show CupertinoPageTransitionsBuilder;
import 'package:flutter/material.dart';

import 'semantic_colors.dart';
import 'tokens.dart';

/// Tema principal de la app.
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
          borderRadius: BorderRadius.vertical(top: Radius.circular(NShape.extraLarge)),
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
      extensions: [
        SemanticColors.from(scheme),
      ],
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
    final onSurface = scheme.onSurface;

    final scaled = base.apply(
      bodyColor: onSurface,
      displayColor: onSurface,
    );

    // Cifras tabulares en toda la escala: el RTT, el jitter y la duración de la
    // conversación se actualizan varias veces por segundo, y sin esto un "9" que
    // pasa a "10" desplaza la fila entera.
    TextStyle? tune(TextStyle? s, FontWeight weight, [double? tracking]) {
      if (s == null) return null;
      return s.copyWith(
        fontWeight: weight,
        fontFeatures: const [FontFeature.tabularFigures()],
        letterSpacing: tracking,
      );
    }

    // Pesos y tracking de la escala oficial M3 (sección 5.2 del documento GEUI):
    // los títulos grandes van en 400 y tracking negativo, los medianos en 500 y
    // las etiquetas en 500.
    return scaled.copyWith(
      displayLarge: tune(scaled.displayLarge, FontWeight.w400, -0.5),
      displayMedium: tune(scaled.displayMedium, FontWeight.w400, -0.4),
      displaySmall: tune(scaled.displaySmall, FontWeight.w400, -0.3),
      headlineLarge: tune(scaled.headlineLarge, FontWeight.w400, -0.4),
      headlineMedium: tune(scaled.headlineMedium, FontWeight.w400, -0.3),
      headlineSmall: tune(scaled.headlineSmall, FontWeight.w400, -0.2),
      titleLarge: tune(scaled.titleLarge, FontWeight.w400, -0.1),
      titleMedium: tune(scaled.titleMedium, FontWeight.w500),
      titleSmall: tune(scaled.titleSmall, FontWeight.w500),
      bodyLarge: tune(scaled.bodyLarge, FontWeight.w400),
      bodyMedium: tune(scaled.bodyMedium, FontWeight.w400),
      bodySmall: tune(scaled.bodySmall, FontWeight.w400),
      labelLarge: tune(scaled.labelLarge, FontWeight.w500),
      labelMedium: tune(scaled.labelMedium, FontWeight.w500),
      labelSmall: tune(scaled.labelSmall, FontWeight.w500),
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
