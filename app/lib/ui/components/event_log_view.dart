/// Lista compacta de eventos (GEUI).
library;

import 'package:flutter/material.dart';

import '../../services/event_log.dart';
import '../semantic_colors.dart';
import '../tokens.dart';

/// Lista compacta de eventos.
class EventLogView extends StatelessWidget {
  const EventLogView({super.key, required this.log, this.max = 6});

  final EventLog log;
  final int max;

  @override
  Widget build(BuildContext context) {
    final entries = log.entries.take(max).toList();
    if (entries.isEmpty) {
      return Text('Sin eventos todavía.', style: Theme.of(context).textTheme.bodySmall);
    }
    final theme = Theme.of(context);
    final sem = SemanticColors.of(context);
    return Column(
      children: [
        for (final e in entries)
          Padding(
            padding: const EdgeInsets.only(bottom: 6),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Padding(
                  padding: const EdgeInsets.only(top: 2),
                  child: Icon(
                    switch (e.level) {
                      LogLevel.success => Icons.check_circle,
                      LogLevel.warn => Icons.warning_amber,
                      LogLevel.error => Icons.error,
                      LogLevel.info => Icons.info_outline,
                    },
                    size: 14,
                    color: switch (e.level) {
                      LogLevel.success => sem.success.color,
                      LogLevel.warn => sem.warning.color,
                      LogLevel.error => theme.colorScheme.error,
                      LogLevel.info => theme.colorScheme.primary,
                    },
                  ),
                ),
                const SizedBox(width: NSpace.sm),
                Expanded(child: Text(e.message, style: theme.textTheme.bodySmall)),
                Text(
                  '${e.time.hour.toString().padLeft(2, '0')}:${e.time.minute.toString().padLeft(2, '0')}:${e.time.second.toString().padLeft(2, '0')}',
                  style: theme.textTheme.labelSmall?.copyWith(
                    color: theme.colorScheme.onSurfaceVariant,
                  ),
                ),
              ],
            ),
          ),
      ],
    );
  }
}
