/// Tokens del sistema Gravital Experience UI (GEUI).
///
/// Única fuente de verdad para espaciado, formas, movimiento y clases de
/// tamaño. Ningún widget fuera de este archivo define valores literales.
library;

import 'package:flutter/material.dart';

/// Espaciado: retícula base de 4 dp (Material).
abstract final class NSpace {
  static const double xs = 4;
  static const double sm = 8;
  static const double md = 12;
  static const double lg = 16;
  static const double xl = 24;
  static const double xxl = 32;
  static const double xxxl = 48;
  static const double huge = 64;

  const NSpace._();
}

/// Escala de esquinas (tokens oficiales M3 Expressive).
abstract final class NShape {
  static const double extraSmall = 4;
  static const double small = 8;
  static const double medium = 12;
  static const double large = 16;
  static const double largeIncreased = 20;
  static const double extraLarge = 28;
  static const double extraLargeIncreased = 32;
  static const double extraExtraLarge = 48;

  /// Forma total (50 % del lado menor): círculos, píldoras y botones.
  static const double full = 999;

  static RoundedRectangleBorder rounded(double r) =>
      RoundedRectangleBorder(borderRadius: BorderRadius.circular(r));

  const NShape._();
}

/// Tokens de resorte (valores de ExpressiveMotionTokens / StandardMotionTokens).
abstract final class NMotion {
  static SpringDescription _s(double stiffness, double ratio) =>
      SpringDescription.withDampingRatio(
        mass: 1,
        stiffness: stiffness,
        ratio: ratio,
      );

  // Expressive: momentos protagonistas.
  static final expressiveFastSpatial = _s(800, 0.6);
  static final expressiveDefaultSpatial = _s(380, 0.8);
  static final expressiveSlowSpatial = _s(200, 0.8);

  // Standard: interacciones repetitivas.
  static final standardFastSpatial = _s(1400, 0.9);
  static final standardDefaultSpatial = _s(700, 0.9);
  static final standardSlowSpatial = _s(300, 0.9);

  // Efectos: cambios de color y opacidad.
  static final fastEffects = _s(3800, 1.0);
  static final defaultEffects = _s(1600, 1.0);
  static final slowEffects = _s(800, 1.0);

  const NMotion._();
}

/// Clases de tamaño de ventana (adaptabilidad).
enum WindowClass {
  compact,
  medium,
  expanded,
  large,
  extraLarge;

  static WindowClass of(BuildContext context) {
    final w = MediaQuery.sizeOf(context).width;
    if (w < 600) return compact;
    if (w < 840) return medium;
    if (w < 1200) return expanded;
    if (w < 1600) return large;
    return extraLarge;
  }

  /// Navegación inferior o toolbar flotante.
  bool get isCompact => this == WindowClass.compact;
  bool get isMedium => this == WindowClass.medium;
  bool get isExpanded => this == WindowClass.expanded;
  bool get isLarge => this == WindowClass.large;
  bool get isExtraLarge => this == WindowClass.extraLarge;
}

/// Duraciones de transición (M3).
abstract final class NDuration {
  static const Duration short = Duration(milliseconds: 100);
  static const Duration medium = Duration(milliseconds: 250);
  static const Duration long = Duration(milliseconds: 450);
  static const Duration extraLong = Duration(milliseconds: 700);

  const NDuration._();
}

/// Curvas de animación (M3).
abstract final class NCurve {
  static const Cubic emphasized = Cubic(0.2, 0, 0, 1);
  static const Cubic emphasizedDecelerate = Cubic(0.05, 0.7, 0.1, 1);
  static const Cubic emphasizedAccelerate = Cubic(0.3, 0, 0.8, 0.15);

  const NCurve._();
}

/// Tamaños de objetivo táctil.
abstract final class NHitSize {
  /// Mínimo recomendado por Material (48 dp).
  static const double minTouch = 48;

  /// Botón PTT principal.
  static const double pttButton = 168;

  const NHitSize._();
}
