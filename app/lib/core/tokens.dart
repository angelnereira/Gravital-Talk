/// Tokens del sistema de diseño de Gravital Talk.
///
/// Centraliza spacing, radios, duraciones y tamaños para que las pantallas
/// dejen de repetir números mágicos. Un cambio de escala o de densidad se hace
/// aquí y se propaga solo.
library;

/// Escala de espaciado. Base de 4 puntos (Material).
class Spacing {
  static const double xxs = 2;
  static const double xs = 4;
  static const double sm = 8;
  static const double md = 12;
  static const double lg = 16;
  static const double xl = 20;
  static const double xxl = 24;
  static const double xxxl = 32;

  const Spacing._();
}

/// Radios de las superficies.
class Radii {
  static const double sm = 8;
  static const double md = 12;
  static const double lg = 14;

  /// Radio de las tarjetas de vidrio.
  ///
  /// Grande a propósito: sobre fondo oscuro, el radio redondo es lo que hace
  /// que una forma se lea como objeto, más que el color. Las tres referencias
  /// que se usaron para el glass usan esquinas muy redondeadas.
  static const double glass = 26;

  /// Elementos pequeños: chips, tiles de métrica.
  static const double chip = 18;
  static const double pill = 999;

  const Radii._();
}

/// Duraciones de animación.
///
/// Se llama `GravitalDurations` y no `Durations` porque Flutter Material ya
/// exporta una clase con ese nombre: al importar `material.dart` junto a estos
/// tokens, el nombre queda ambiguo y no compila.
///
/// Debajo de 150 ms una transición se percibe como instantánea. Entre 150 y
/// 300 ms es el rango cómodo. Por encima, el usuario pierde la sensación de
/// respuesta directa.
class GravitalDurations {
  static const Duration instant = Duration(milliseconds: 100);
  static const Duration fast = Duration(milliseconds: 150);
  static const Duration normal = Duration(milliseconds: 220);
  static const Duration slow = Duration(milliseconds: 320);

  const GravitalDurations._();
}

/// Elevaciones, en lugar de números sueltos.
class Elevations {
  static const double flat = 0;
  static const double card = 1;
  static const double overlay = 3;

  const Elevations._();
}

/// Tamaños de objetivo táctil.
class HitSizes {
  /// Mínimo recomendado por Material (48 dp) para cualquier objetivo táctil.
  static const double minTouch = 48;

  /// Alto del botón PTT de la pantalla de sesión.
  static const double pttButton = 132;

  const HitSizes._();
}
