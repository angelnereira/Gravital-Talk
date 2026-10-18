/// Banner de reconexión (GEUI).
library;

import 'package:flutter/material.dart';

import '../semantic_colors.dart';
import '../tokens.dart';

/// Banner de reconexión: la sesión se está recuperando sola.
class ReconnectingBanner extends StatelessWidget {
  const ReconnectingBanner({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final sem = SemanticColors.of(context);
    return Container(
      padding: const EdgeInsets.all(NSpace.md),
      decoration: BoxDecoration(
        color: sem.warning.container,
        borderRadius: BorderRadius.circular(NShape.medium),
      ),
      child: Row(
        children: [
          SizedBox(
            width: 16,
            height: 16,
            child: CircularProgressIndicator(
              strokeWidth: 2,
              valueColor: AlwaysStoppedAnimation<Color>(sem.warning.color),
            ),
          ),
          const SizedBox(width: NSpace.md),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'Reconectando…',
                  style: theme.textTheme.labelLarge?.copyWith(color: sem.warning.color),
                ),
                Text(
                  'La sesión se recupera sola tras un corte de red',
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: sem.warning.onContainer,
                  ),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
