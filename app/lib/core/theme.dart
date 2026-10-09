import 'package:flutter/material.dart';

import 'palette.dart';
import 'tokens.dart';

/// Tema Material 3 de Gravital Talk.
///
/// El tema claro y el oscuro comparten la misma estructura de superficies para
/// que una pantalla nueva herede ambos sin trabajo extra.
///
/// La diferencia con la versión anterior: aquí **no** se usa `ColorScheme.fromSeed`.
/// `fromSeed` genera paletas tonales algorítmicas, que es justo lo que hacía que
/// la app se viera como cualquier otra. Los valores salen de `GravitalPalette`.
class GravitalTheme {
  static ThemeData light() => _base(Brightness.light);
  static ThemeData dark() => _base(Brightness.dark);

  static ThemeData _base(Brightness brightness) {
    final scheme = brightness == Brightness.light
        ? _lightScheme()
        : _darkScheme();
    final text = _textTheme(brightness);

    return ThemeData(
      useMaterial3: true,
      brightness: brightness,
      colorScheme: scheme,
      splashFactory: InkSparkle.splashFactory,
      visualDensity: VisualDensity.adaptivePlatformDensity,
      textTheme: text,
      scaffoldBackgroundColor: scheme.surface,

      appBarTheme: AppBarTheme(
        centerTitle: false,
        elevation: GravitalElevation.flat,
        scrolledUnderElevation: GravitalElevation.overlay,
        backgroundColor: scheme.surface,
        foregroundColor: scheme.onSurface,
        // La barra se vuelve opaca al hacer scroll. Sin `surfaceTint` 
        // Material añade un velo de color que con el teal se ve sucio.
        surfaceTintColor: Colors.transparent,
      ),

      cardTheme: CardThemeData(
        elevation: GravitalElevation.flat,
        margin: EdgeInsets.zero,
        color: scheme.surfaceContainerLow,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(Radii.xl),
          // Borde en lugar de sombra: es lo que da el aspecto de "equipo".
          side: BorderSide(color: scheme.outlineVariant),
        ),
      ),

      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        isDense: true,
        fillColor: scheme.surfaceContainerHighest.withValues(alpha: 0.5),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(Radii.md),
          borderSide: BorderSide(color: scheme.outlineVariant),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(Radii.md),
          borderSide: BorderSide(color: scheme.outlineVariant),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(Radii.md),
          borderSide: BorderSide(color: scheme.primary, width: 2),
        ),
        errorBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(Radii.md),
          borderSide: BorderSide(color: scheme.error),
        ),
        focusedErrorBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(Radii.md),
          borderSide: BorderSide(color: scheme.error, width: 2),
        ),
      ),

      filledButtonTheme: FilledButtonThemeData(
        style: FilledButton.styleFrom(
          minimumSize: const Size.fromHeight(HitSizes.minTouch),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(Radii.lg),
          ),
          textStyle: text.labelLarge?.copyWith(
            fontWeight: GravitalType.titleWeight,
          ),
        ),
      ),

      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          minimumSize: const Size.fromHeight(HitSizes.minTouch),
          side: BorderSide(color: scheme.outline),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(Radii.lg),
          ),
        ),
      ),

      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          minimumSize: const Size(HitSizes.minTouch, HitSizes.minTouch),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(Radii.lg),
          ),
        ),
      ),

      navigationBarTheme: NavigationBarThemeData(
        height: 64,
        elevation: GravitalElevation.overlay,
        labelBehavior: NavigationDestinationLabelBehavior.alwaysShow,
        backgroundColor: scheme.surfaceContainer,
      ),

      listTileTheme: ListTileThemeData(
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(Radii.md),
        ),
        minVerticalPadding: Spacing.sm,
      ),

      snackBarTheme: SnackBarThemeData(
        behavior: SnackBarBehavior.floating,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(Radii.md),
        ),
      ),

      dividerTheme: DividerThemeData(
        color: scheme.outlineVariant,
        space: Spacing.lg,
        thickness: 1,
      ),

      bottomSheetTheme: BottomSheetThemeData(
        backgroundColor: scheme.surfaceContainerLow,
        shape: const RoundedRectangleBorder(
          borderRadius: BorderRadius.vertical(top: Radius.circular(Radii.xl)),
        ),
      ),

      dialogTheme: DialogThemeData(
        backgroundColor: scheme.surfaceContainerLow,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(Radii.xl),
        ),
      ),

      // Resplandor al pulsar: muy contenido. Por defecto Material lo hace
      // enorme y con esta paleta parece plástico.
      splashColor: scheme.primary.withValues(alpha: 0.08),
      highlightColor: scheme.primary.withValues(alpha: 0.04),
    );
  }

  /// Esquema claro, escrito a mano.
  static ColorScheme _lightScheme() {
    return const ColorScheme.light(
      primary: GravitalPalette.brand,
      onPrimary: Colors.white,
      primaryContainer: Color(0xFFCCFBF1),
      onPrimaryContainer: GravitalPalette.brandDark,
      secondary: GravitalPalette.online,
      tertiary: GravitalPalette.transmit,
      error: GravitalPalette.danger,
      surface: GravitalPalette.surfaceLight,
      onSurface: GravitalPalette.onSurfaceLight,
      onSurfaceVariant: GravitalPalette.onSurfaceVariantLight,
      surfaceContainerLowest: Colors.white,
      surfaceContainerLow: GravitalPalette.surfaceContainerLight,
      surfaceContainer: GravitalPalette.surfaceContainerHighLight,
      surfaceContainerHigh: Color(0xFFDFE7EC),
      surfaceContainerHighest: Color(0xFFD4DEE5),
      outline: GravitalPalette.outlineLight,
      outlineVariant: Color(0xFFDCE4EA),
    );
  }

  /// Esquema oscuro, escrito a mano.
  static ColorScheme _darkScheme() {
    return const ColorScheme.dark(
      // En oscuro el teal sube de luminancia para mantener el contraste.
      primary: GravitalPalette.brandLight,
      onPrimary: Color(0xFF00332F),
      primaryContainer: Color(0xFF115E59),
      onPrimaryContainer: Color(0xFFCCFBF1),
      secondary: Color(0xFF4ADE80),
      tertiary: GravitalPalette.transmitLight,
      error: Color(0xFFF87171),
      surface: GravitalPalette.surfaceDark,
      onSurface: GravitalPalette.onSurfaceDark,
      onSurfaceVariant: GravitalPalette.onSurfaceVariantDark,
      surfaceContainerLowest: Color(0xFF060B10),
      surfaceContainerLow: GravitalPalette.surfaceContainerDark,
      surfaceContainer: GravitalPalette.surfaceContainerHighDark,
      surfaceContainerHigh: Color(0xFF243444),
      surfaceContainerHighest: Color(0xFF2E4053),
      outline: GravitalPalette.outlineDark,
      outlineVariant: Color(0xFF22303E),
    );
  }

  /// Tipografía: pesos y tracking, no tamaños.
  static TextTheme _textTheme(Brightness brightness) {
    final base = Typography.material2021().black;
    final onSurface = brightness == Brightness.dark
        ? GravitalPalette.onSurfaceDark
        : GravitalPalette.onSurfaceLight;

    return base.apply(
      bodyColor: onSurface,
      displayColor: onSurface,
    ).copyWith(
      headlineSmall: base.headlineSmall?.copyWith(
        fontWeight: GravitalType.headlineWeight,
        letterSpacing: -0.5,
      ),
      titleLarge: base.titleLarge?.copyWith(
        fontWeight: GravitalType.headlineWeight,
        letterSpacing: -0.2,
      ),
      titleMedium: base.titleMedium?.copyWith(
        fontWeight: GravitalType.titleWeight,
      ),
      labelLarge: base.labelLarge?.copyWith(
        fontWeight: GravitalType.titleWeight,
      ),
      labelSmall: base.labelSmall?.copyWith(
        fontWeight: GravitalType.labelWeight,
        letterSpacing: GravitalType.labelTracking,
      ),
      // Métricas en tabular: cifras alineadas, sin bailoteo al actualizarse.
      bodyMedium: base.bodyMedium?.copyWith(fontFeatures: GravitalType.numericFeatures),
    );
  }

  const GravitalTheme._();
}