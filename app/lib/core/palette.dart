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
  //
  // La escalera está calculada, no elegida a ojo. El problema que corrige: con
  // los valores anteriores el fondo y la tarjeta tenían un salto de luminancia
  // de 1.10x, es decir invisibles, y la app se veía como una mancha informe.
  //
  // Cada paso se midió con el ratio de contraste WCAG entre superficies:

  /// Fondo base. Casi negro, con matiz azul frío.
  static const surfaceDark = Color(0xFF070B10);

  /// Superficie elevada (tarjetas). ΔL* 22.3 vs fondo.
  ///
  /// El valor que estaba aquí (`#1C2733`) sólo llegaba a ΔL* 12.2, por debajo
  /// del umbral (~20) en el que dos superficies se leen como objetos
  /// distintos. Se midió con ΔL* de CIELAB y no con ratio WCAG precisamente
  /// por eso: el ratio WCAG predice legibilidad de texto, no separación de
  /// superficies, y por eso el cambio anterior apenas se notó.
  ///
  /// Las tarjetas tienen que ser **gris medio**, no oscuras sobre oscuras.
  /// Es lo que hace la referencia de Pixel.
  static const surfaceContainerDark = Color(0xFF303D4B);

  /// Superficie aún más elevada (diálogos, hojas). ΔL* 25.4 vs fondo.
  static const surfaceContainerHighDark = Color(0xFF3A4859);

  /// Superficie de máximo nivel (chips, campos). ΔL* 28.3 vs fondo.
  static const surfaceContainerHighestDark = Color(0xFF445363);

  /// Borde de tarjeta. Blanco al 12 % sobre la tarjeta.
  ///
  /// En modo oscuro la luminancia está comprimida: 1.30x de relleno no siempre
  /// basta para que una tarjeta se lea en pantallas baratas. El borde es la
  /// garantía extra. Es sutil a propósito (44 % más de contraste que el
  /// relleno), no una línea dura.
  static const outlineDark = Color(0xFF37404B);

  /// Texto principal en oscuro. Blanco azulado, no blanco puro.
  static const onSurfaceDark = Color(0xFFE6EDF3);

  /// Texto secundario en oscuro.
  ///
  /// Sobre la tarjeta nueva da 6.10:1. El fondo ahora es más claro, así que
  /// este valor ha tenido que subir para mantener la separación.
  static const onSurfaceVariantDark = Color(0xFFCBD7E2);

  // ── Superficies: modo claro ────────────────────────────────────────────
  //
  // El problema en claro es el inverso y más grave de lo que parece: con fondo
  // `#F6F8FA` y tarjeta blanca el salto era de 1.06x —también invisible—. La
  // solución no es aclarar la tarjeta, es oscurecer el fondo.

  /// Fondo base claro.
  ///
  /// Más gris que el `#F6F8FA` anterior a propósito: contra la tarjeta blanca
  /// el salto pasa de 1.06x a 1.13x.
  static const surfaceLight = Color(0xFFEEF2F5);

  /// Tarjetas en claro. Blanco real: las tarjetas son lo que destaca.
  static const surfaceContainerLight = Color(0xFFFFFFFF);

  static const surfaceContainerHighLight = Color(0xFFFFFFFF);

  /// Borde en claro. Negro al 10 %: separa sin endurecer.
  static const outlineLight = Color(0xFFE5E5E5);

  static const onSurfaceLight = Color(0xFF0F1923);

  /// Texto secundario en claro. 8.56:1 sobre blanco.
  static const onSurfaceVariantLight = Color(0xFF3E4E5D);
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