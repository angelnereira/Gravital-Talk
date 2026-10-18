/// Colores semánticos del dominio Gravital Talk.
///
/// Material no define estados de una llamada PTT ni calidad de audio. Este
/// sistema los agrega como ThemeExtension, cada uno con cuatro roles.
library;

import 'package:flutter/material.dart';

/// Un tono semántico con sus cuatro roles.
@immutable
class SemanticTone {
  const SemanticTone(this.color, this.onColor, this.container, this.onContainer);

  final Color color;
  final Color onColor;
  final Color container;
  final Color onContainer;

  static SemanticTone lerp(SemanticTone a, SemanticTone b, double t) => SemanticTone(
        Color.lerp(a.color, b.color, t)!,
        Color.lerp(a.onColor, b.onColor, t)!,
        Color.lerp(a.container, b.container, t)!,
        Color.lerp(a.onContainer, b.onContainer, t)!,
      );
}

/// Colores semánticos extendidos para Gravital Talk.
@immutable
class SemanticColors extends ThemeExtension<SemanticColors> {
  const SemanticColors({
    required this.success,
    required this.warning,
    required this.info,
    required this.pttActive,
    required this.pttFloor,
    required this.calling,
    required this.connectionPoor,
  });

  /// Conectado, calidad buena.
  final SemanticTone success;

  /// Precaución, latencia alta.
  final SemanticTone warning;

  /// Neutral, informativo.
  final SemanticTone info;

  /// Hablando ahora (transmitiendo).
  final SemanticTone pttActive;

  /// Floor otorgado (turno bloqueado).
  final SemanticTone pttFloor;

  /// Llamando o llamada entrante.
  final SemanticTone calling;

  /// Conexión degradada.
  final SemanticTone connectionPoor;

  /// Construye los tonos semánticos a partir de un ColorScheme base.
  ///
  /// Cada uno parte de un tono fijo y se armoniza con el primario del esquema
  /// para que el contraste sea correcto en claro y oscuro.
  factory SemanticColors.from(ColorScheme scheme) {
    SemanticTone make(Color seed) {
      final s = ColorScheme.fromSeed(
        seedColor: seed,
        brightness: scheme.brightness,
      );
      return SemanticTone(s.primary, s.onPrimary, s.primaryContainer, s.onPrimaryContainer);
    }

    return SemanticColors(
      success: make(const Color(0xFF1E8E3E)),
      warning: make(const Color(0xFFF9AB00)),
      info: make(const Color(0xFF1A73E8)),
      pttActive: make(const Color(0xFFE8710A)),
      pttFloor: make(const Color(0xFF0B57D0)),
      calling: make(const Color(0xFF9C27B0)),
      connectionPoor: make(const Color(0xFFC5221F)),
    );
  }

  static SemanticColors of(BuildContext context) =>
      Theme.of(context).extension<SemanticColors>() ??
      SemanticColors.from(Theme.of(context).colorScheme);

  @override
  SemanticColors copyWith() => this;

  @override
  SemanticColors lerp(ThemeExtension<SemanticColors>? other, double t) {
    if (other == null || other is! SemanticColors) return this;
    return SemanticColors(
      success: SemanticTone.lerp(success, other.success, t),
      warning: SemanticTone.lerp(warning, other.warning, t),
      info: SemanticTone.lerp(info, other.info, t),
      pttActive: SemanticTone.lerp(pttActive, other.pttActive, t),
      pttFloor: SemanticTone.lerp(pttFloor, other.pttFloor, t),
      calling: SemanticTone.lerp(calling, other.calling, t),
      connectionPoor: SemanticTone.lerp(connectionPoor, other.connectionPoor, t),
    );
  }
}
