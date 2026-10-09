/// Tarjeta de sesión activa en la home.
///
/// Existe porque antes la sesión activa era un botón más dentro del `ListView`
/// de la home, con el mismo peso visual que los modos de conexión. Con una
/// llamada en curso, lo importante es volver a ella, no crear otra.
///
/// Muestra duración, estado y calidad para que la home sirva de resumen sin
/// entrar en la sesión.
library;

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../core/tokens.dart';
import '../models/connection.dart';
import '../screens/session_screen.dart';
import '../services/session_controller.dart';
import 'common.dart';

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
    final m = c.metrics;
    final label = c.roomCode ??
        (c.mode == ConnectionMode.p2p ? 'P2P directo' : 'Sesión');

    return Card(
      color: theme.colorScheme.primaryContainer,
      shape: RoundedRectangleBorder(
        borderRadius: BorderRadius.circular(Radii.xl),
      ),
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: () => Navigator.of(context).push(
          MaterialPageRoute(builder: (_) => const SessionScreen()),
        ),
        child: Padding(
          padding: const EdgeInsets.all(Spacing.lg),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  // Punto pulsante: indica llamada en vivo de un vistazo.
                  Container(
                    width: 10,
                    height: 10,
                    decoration: const BoxDecoration(
                      color: GravitalColors.online,
                      shape: BoxShape.circle,
                    ),
                  ),
                  const SizedBox(width: Spacing.sm),
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
              const SizedBox(height: Spacing.xs),
              Text(
                label,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onPrimaryContainer
                      .withValues(alpha: 0.85),
                ),
              ),
              const SizedBox(height: Spacing.md),
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
                  const SizedBox(width: Spacing.sm),
                  Expanded(
                    child: MetricTile(
                      label: 'MOS',
                      value: m.estimatedMos.toStringAsFixed(2),
                      icon: Icons.star_outline,
                      color: m.estimatedMos >= 4
                          ? GravitalColors.online
                          : (m.estimatedMos >= 3
                              ? Colors.orange
                              : theme.colorScheme.error),
                    ),
                  ),
                  const SizedBox(width: Spacing.sm),
                  Expanded(
                    child: MetricTile(
                      label: 'RTT',
                      value: m.rttMs.toStringAsFixed(0),
                      unit: 'ms',
                      icon: Icons.speed,
                      color: m.rttMs < 60
                          ? theme.colorScheme.onPrimaryContainer
                          : GravitalColors.transmit,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: Spacing.md),
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
