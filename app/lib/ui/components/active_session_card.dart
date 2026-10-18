/// Tarjeta de sesión activa en la home (GEUI).
library;

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../../models/connection.dart';
import '../../screens/session_screen.dart';
import '../../services/session_controller.dart';
import '../semantic_colors.dart';
import '../tokens.dart';
import 'metric_tile.dart';
import 'status_badge.dart';

/// Tarjeta de sesión activa en la home.
class ActiveSessionCard extends StatelessWidget {
  const ActiveSessionCard({super.key});

  String _formatDuration(Duration d) {
    final m = d.inMinutes.remainder(60).toString().padLeft(2, '0');
    final s = d.inSeconds.remainder(60).toString().padLeft(2, '0');
    return '$m:$s';
  }

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();
    if (!c.isLive) return const SizedBox.shrink();

    final theme = Theme.of(context);
    final sem = SemanticColors.of(context);
    final m = c.metrics;
    final label = c.roomCode ??
        (c.mode == ConnectionMode.p2p ? 'P2P directo' : 'Sesión');

    return Card(
      color: theme.colorScheme.primaryContainer,
      shape: NShape.rounded(NShape.extraLarge),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: () => Navigator.of(context).push(
          MaterialPageRoute(builder: (_) => const SessionScreen()),
        ),
        child: Padding(
          padding: const EdgeInsets.all(NSpace.lg),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Container(
                    width: 10,
                    height: 10,
                    decoration: BoxDecoration(
                      color: sem.success.color,
                      shape: BoxShape.circle,
                    ),
                  ),
                  const SizedBox(width: NSpace.sm),
                  Expanded(
                    child: Text(
                      'Sesión activa',
                      style: theme.textTheme.titleMedium?.copyWith(
                        color: theme.colorScheme.onPrimaryContainer,
                      ),
                    ),
                  ),
                  StatusBadge(c.state),
                ],
              ),
              const SizedBox(height: NSpace.xs),
              Text(
                label,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onPrimaryContainer.withValues(alpha: 0.85),
                ),
              ),
              const SizedBox(height: NSpace.md),
              Row(
                children: [
                  Expanded(
                    child: MetricTile(
                      label: 'Duración',
                      value: _formatDuration(c.elapsed),
                      icon: Icons.timer_outlined,
                      color: theme.colorScheme.onPrimaryContainer,
                    ),
                  ),
                  const SizedBox(width: NSpace.sm),
                  Expanded(
                    child: MetricTile(
                      label: 'MOS',
                      value: m.estimatedMos.toStringAsFixed(2),
                      icon: Icons.star_outline,
                      color: m.estimatedMos >= 4
                          ? sem.success.color
                          : (m.estimatedMos >= 3
                              ? sem.warning.color
                              : theme.colorScheme.error),
                    ),
                  ),
                  const SizedBox(width: NSpace.sm),
                  Expanded(
                    child: MetricTile(
                      label: 'RTT',
                      value: m.rttMs.toStringAsFixed(0),
                      unit: 'ms',
                      icon: Icons.speed,
                      color: m.rttMs < 60
                          ? theme.colorScheme.onPrimaryContainer
                          : sem.pttActive.color,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: NSpace.md),
              Align(
                alignment: Alignment.centerRight,
                child: TextButton.icon(
                  onPressed: () => Navigator.of(context).push(
                    MaterialPageRoute(builder: (_) => const SessionScreen()),
                  ),
                  icon: const Icon(Icons.open_in_new, size: 16),
                  label: const Text('Abrir sesión'),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
