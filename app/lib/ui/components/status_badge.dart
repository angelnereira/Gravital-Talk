/// Badge de estado de la sesión (GEUI).
library;

import 'package:flutter/material.dart';

import '../../models/session.dart';
import '../semantic_colors.dart';
import '../tokens.dart';

/// Badge de estado de la sesión.
class StatusBadge extends StatelessWidget {
  const StatusBadge(this.state, {super.key});

  final SessionState state;

  Color _color(BuildContext context) {
    final sem = Theme.of(context).extension<SemanticColors>()!;
    final scheme = Theme.of(context).colorScheme;
    return switch (state) {
      SessionState.active => sem.success.color,
      SessionState.handshaking => sem.warning.color,
      SessionState.reconnecting => sem.warning.color,
      SessionState.error => scheme.error,
      SessionState.closed => scheme.outline,
      _ => scheme.secondary,
    };
  }

  @override
  Widget build(BuildContext context) {
    final color = _color(context);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.14),
        borderRadius: BorderRadius.circular(NShape.full),
        border: Border.all(color: color.withValues(alpha: 0.5)),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          Container(
            width: 8,
            height: 8,
            decoration: BoxDecoration(color: color, shape: BoxShape.circle),
          ),
          const SizedBox(width: NSpace.sm),
          Text(
            state.label,
            style: TextStyle(
              color: color,
              fontWeight: FontWeight.w600,
              fontSize: 12,
            ),
          ),
        ],
      ),
    );
  }
}
