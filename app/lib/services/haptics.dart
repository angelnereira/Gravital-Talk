import 'dart:async';

import 'package:flutter/services.dart';

/// Vibraciones cortas que confirman un gesto o avisan de algo.
///
/// Donde no hay motor de vibración (escritorio) no hacen nada. La vibración
/// larga de una llamada entrante no va aquí: la lleva el timbre nativo.
class Haptics {
  Haptics._();

  /// Se pulsó el botón de hablar.
  static void pttDown() => _run(HapticFeedback.mediumImpact);

  /// Se soltó el botón de hablar.
  static void pttUp() => _run(HapticFeedback.lightImpact);

  /// Se leyó un código.
  static void scanned() => _run(HapticFeedback.heavyImpact);

  /// Llegó un aviso con la app a la vista.
  static void notice() => _run(HapticFeedback.mediumImpact);

  static void _run(Future<void> Function() feedback) {
    unawaited(feedback().catchError((Object _) {}));
  }
}
