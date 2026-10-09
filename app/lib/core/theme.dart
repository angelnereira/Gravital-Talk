import 'package:flutter/material.dart';

import 'tokens.dart';

/// Paleta y tema Material 3 de Gravital Talk.
///
/// El tema claro y el oscuro comparten la misma estructura de superficies para
/// que una pantalla nueva herede ambos sin trabajo extra: si sólo defines
/// `light()`, el oscuro queda con los valores por defecto de Material y los
/// contrastes se resienten.
class GravitalTheme {
  /// Color semilla del esquema (azul Gravital).
  static const seed = Color(0xFF3D5AFE);

  /// Color de acento para estados positivos (conectado, MOS alto).
  static const accent = Color(0xFF00E5A0);

  /// Color de PTT activo (transmitiendo).
  static const pttActive = Color(0xFFFF3D00);

  /// Color de error.
  static const danger = Color(0xFFFF5252);

  /// Colores compartidos por ambos esquemas.
  static const online = Color(0xFF00C853);
  static const offline = Color(0xFF9E9E9E);

  static ThemeData light() => _base(Brightness.light);
  static ThemeData dark() => _base(Brightness.dark);

  static ThemeData _base(Brightness brightness) {
    final scheme = ColorScheme.fromSeed(
      seedColor: seed,
      brightness: brightness,
    );

    final text = _textTheme(brightness);

    return ThemeData(
      useMaterial3: true,
      colorScheme: scheme,
      brightness: brightness,
      splashFactory: InkSparkle.splashFactory,
      visualDensity: VisualDensity.adaptivePlatformDensity,
      textTheme: text,
      scaffoldBackgroundColor: scheme.surface,

      appBarTheme: AppBarTheme(
        centerTitle: false,
        elevation: Elevations.flat,
        scrolledUnderElevation: Elevations.card,
        backgroundColor: scheme.surface,
        foregroundColor: scheme.onSurface,
      ),

      cardTheme: CardThemeData(
        elevation: Elevations.flat,
        margin: EdgeInsets.zero,
        color: scheme.surfaceContainerLow,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(Radii.xl),
          side: BorderSide(color: scheme.outlineVariant.withValues(alpha: 0.6)),
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
          textStyle: text.labelLarge?.copyWith(fontWeight: FontWeight.w700),
        ),
      ),

      outlinedButtonTheme: OutlinedButtonThemeData(
        style: OutlinedButton.styleFrom(
          minimumSize: const Size.fromHeight(HitSizes.minTouch),
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
        elevation: Elevations.card,
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
          borderRadius: BorderRadius.circular(Radii.xl)),
      ),
    );
  }

  /// Tipografía compacta y legible.
  ///
  /// Se mantiene `fontSize` entero para evitar renders borrosos en pantallas
  /// de densidad fraccionaria.
  static TextTheme _textTheme(Brightness brightness) {
    final base = Typography.material2021().black;
    final onSurface = brightness == Brightness.dark
        ? const Color(0xFFECEFF4)
        : const Color(0xFF1A1C1E);

    return base.apply(bodyColor: onSurface, displayColor: onSurface).copyWith(
      headlineSmall: base.headlineSmall?.copyWith(fontWeight: FontWeight.w700),
      titleLarge: base.titleLarge?.copyWith(fontWeight: FontWeight.w700),
      titleMedium: base.titleMedium?.copyWith(fontWeight: FontWeight.w700),
      labelLarge: base.labelLarge?.copyWith(fontWeight: FontWeight.w600),
      labelSmall: base.labelSmall?.copyWith(letterSpacing: 0.4),
    );
  }

  const GravitalTheme._();
}
