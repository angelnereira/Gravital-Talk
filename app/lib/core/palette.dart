/// Paleta de Gravital Talk.
///
/// ## De dónde viene
///
/// Antes el tema era `ColorScheme.fromSeed(0xFF3D5AFE)` con las paletas tonales
/// por defecto de Material. Eso produce exactamente lo que producía: una app
/// correcta y anónima, porque `fromSeed` genera la misma progresión tonal para
/// cualquier semilla.
///
/// ## La dirección
///
/// **Equipo de radio de precisión.** Superficies frías y casi neutras, y un
/// único elemento saturado en pantalla: el audio. El medidor de nivel, el anillo
/// del PTT y el estado de la conexión son lo único con color fuerte, así que
/// cuando algo se ilumina, significa algo.
///
/// La decisión que más identidad da con menos riesgo es el color de marca:
/// **teal**. Se lee como señal y comunicaciones; el azul se lee como app
/// corporativa, y es el default de todo el mundo.
///
/// El segundo es el color de transmisión: **ámbar**, no naranja. Ámbar es la
/// convención real de radiodifusión para "en el aire". El naranja anterior era
/// arbitrario; este significa algo.
library;
import 'package:flutter/material.dart';


/// Colores de marca y semánticos.
///
/// Los valores son fijos, no generados: si se derivaran de una semilla, se
/// volvería a Material You y a la indistinción.
abstract final class GravitalPalette {
  // ── Marca ──────────────────────────────────────────────────────────────

  /// Teal de marca. El color de "señal".
  static const brand = Color(0xFF0D9488);

  /// Teal claro, para modo oscuro y para estados sobre fondo oscuro.
  static const brandLight = Color(0xFF2DD4BF);

  /// Teal profundo, para texto sobre fondos claros.
  static const brandDark = Color(0xFF115E59);

  // ── Semánticos ─────────────────────────────────────────────────────────

  /// Transmitiendo / en el aire. Convención de radiodifusión.
  static const transmit = Color(0xFFF5A623);

  /// Transmitiendo sobre fondo oscuro, un paso más claro para no perderse.
  static const transmitLight = Color(0xFFFFB84D);

  /// Conectado, calidad buena.
  static const online = Color(0xFF16A34A);

  /// Algo va mal.
  static const danger = Color(0xFFDC2626);

  /// Precaución, sin llegar a error.
  static const warning = Color(0xFFD97706);

  // ── Superficies: modo oscuro ───────────────────────────────────────────

  /// Fondo base. Azul-negro, no gris: el matiz frío es lo que da el carácter.
  static const surfaceDark = Color(0xFF0A0F16);

  /// Superficie elevada (tarjetas).
  static const surfaceContainerDark = Color(0xFF111A24);

  /// Superficie aún más elevada (diálogos, hojas).
  static const surfaceContainerHighDark = Color(0xFF1A2634);

  /// Borde sutil.
  static const outlineDark = Color(0xFF2C3E50);

  /// Texto principal en oscuro. Blanco azulado, no blanco puro.
  static const onSurfaceDark = Color(0xFFE6EDF3);

  /// Texto secundario en oscuro.
  static const onSurfaceVariantDark = Color(0xFF9AAAB8);

  // ── Superficies: modo claro ────────────────────────────────────────────

  /// Fondo base claro, con un matiz frío mínimo.
  static const surfaceLight = Color(0xFFF6F8FA);

  /// Tarjetas en claro. Blanco real: las tarjetas son lo que destaca.
  static const surfaceContainerLight = Color(0xFFFFFFFF);

  static const surfaceContainerHighLight = Color(0xFFEAF0F4);

  static const outlineLight = Color(0xFFC4CFD8);

  static const onSurfaceLight = Color(0xFF0F1923);

  static const onSurfaceVariantLight = Color(0xFF4A5B6A);
}

/// Escala tipográfica de Gravital Talk.
///
/// No cambia los tamaños base de Material —eso sería ruido—, sino los pesos y
/// el tracking, que es lo que de verdad cambia el carácter.
abstract final class GravitalType {
  /// Título de pantalla: pesado y compacto.
  static const headlineWeight = FontWeight.w700;

  /// Título de tarjeta.
  static const titleWeight = FontWeight.w600;

  /// Etiqueta y metadato.
  static const labelWeight = FontWeight.w500;

  /// Cifras: siempre tabulares.
  ///
  /// Sin esto, las métricas bailan al actualizarse y un RTT que pasa de 9 a 10
  /// mueve toda la fila.
  static const numericFeatures = <FontFeature>[
    FontFeature.tabularFigures(),
    FontFeature("tnum"),
  ];

  /// Tracking de las etiquetas en versalitas (badges, estados).
  static const labelTracking = 0.6;
}

/// Sombras y elevaciones propias.
abstract final class GravitalElevation {
  /// Sin sombra: las tarjetas se separan por borde, no por sombra.
  static const flat = 0.0;

  /// Sombra muy contenida, sólo para lo que realmente flota.
  static const overlay = 2.0;
}

/// Duraciones de animación de la marca.
///
/// Por debajo de 150 ms no se percibe como animación; por encima de 300, como
/// lentitud. El rango útil es estrecho.
abstract final class GravitalMotion {
  static const instant = Duration(milliseconds: 120);
  static const standard = Duration(milliseconds: 240);
  static const expressive = Duration(milliseconds: 360);
}