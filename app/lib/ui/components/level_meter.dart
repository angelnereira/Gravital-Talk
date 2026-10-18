/// Barra de nivel de micrófono animada (GEUI).
library;

import 'package:flutter/material.dart';
import 'package:percent_indicator/linear_percent_indicator.dart';

import '../semantic_colors.dart';

/// Barra de nivel de micrófono animada.
class LevelMeter extends StatelessWidget {
  const LevelMeter({super.key, required this.level, this.transmitting = false});

  final double level;
  final bool transmitting;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final sem = SemanticColors.of(context);
    return Semantics(
      label: 'Nivel de micrófono',
      value: '${(level.clamp(0.0, 1.0) * 100).round()}%',
      child: ExcludeSemantics(
        child: LinearPercentIndicator(
          percent: level.clamp(0.0, 1.0),
          animation: true,
          animationDuration: 120,
          lineHeight: 8,
          barRadius: const Radius.circular(8),
          backgroundColor: scheme.onSurface.withValues(alpha: 0.12),
          progressColor: transmitting ? sem.pttActive.color : scheme.primary,
        ),
      ),
    );
  }
}
