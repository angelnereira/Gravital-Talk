import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../models/connection.dart';
import '../services/session_controller.dart';
import '../widgets/common.dart';

/// Pantalla de sesión activa: PTT, medidores y métricas en vivo.
class SessionScreen extends StatelessWidget {
  const SessionScreen({super.key});

  String _formatDuration(Duration d) {
    final h = d.inHours;
    final m = d.inMinutes.remainder(60).toString().padLeft(2, '0');
    final s = d.inSeconds.remainder(60).toString().padLeft(2, '0');
    return h > 0 ? '$h:$m:$s' : '$m:$s';
  }

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();
    final scheme = Theme.of(context).colorScheme;
    final m = c.metrics;

    final modeLabel = switch (c.mode) {
      ConnectionMode.server =>
        c.role == ConnectionRole.host ? 'Sala (host)' : 'Sala (join)',
      ConnectionMode.p2p =>
        c.role == ConnectionRole.host ? 'P2P (esperando)' : 'P2P (directo)',
      null => 'Sesión',
    };

    return Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(modeLabel),
            Text(
              c.isLive
                  ? 'session 0x${c.sessionId.toRadixString(16).padLeft(8, '0').toUpperCase()}'
                  : 'sin conexión',
              style: Theme.of(context).textTheme.labelSmall,
            ),
          ],
        ),
        actions: [
          Padding(
            padding: const EdgeInsets.only(right: 12),
            child: Center(child: StatusBadge(c.state)),
          ),
        ],
      ),
      body: !c.isLive
          ? _IdlePlaceholder(onBack: () => Navigator.pop(context))
          : ListView(
              padding: const EdgeInsets.fromLTRB(16, 12, 16, 24),
              children: [
                Row(
                  children: [
                    Expanded(
                      child: MetricTile(
                        label: 'Duración',
                        value: _formatDuration(c.elapsed),
                        icon: Icons.timer_outlined,
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: MetricTile(
                        label: 'Room',
                        value: c.roomCode ?? 'P2P',
                        icon: Icons.meeting_room_outlined,
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: MetricTile(
                        label: 'Puerto local',
                        value: '${c.localPort}',
                        icon: Icons.settings_ethernet,
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 22),
                if (c.peerPttActive && !c.pttActive)
                  Container(
                    padding: const EdgeInsets.all(12),
                    decoration: BoxDecoration(
                      color: scheme.primary.withValues(alpha: 0.1),
                      borderRadius: BorderRadius.circular(14),
                    ),
                    child: Row(
                      children: [
                        Icon(Icons.hearing, color: scheme.primary),
                        const SizedBox(width: 10),
                        const Text('El peer está transmitiendo…',
                            style: TextStyle(fontWeight: FontWeight.w600)),
                      ],
                    ),
                  ),
                if (c.peerPttActive && !c.pttActive) const SizedBox(height: 14),
                Center(
                  child: PttButton(
                    pressed: c.pttActive,
                    enabled: c.isLive,
                    peerSpeaking: c.peerPttActive,
                    onDown: c.pttDown,
                    onUp: c.pttUp,
                  ),
                ),
                const SizedBox(height: 16),
                ClipRRect(
                  borderRadius: BorderRadius.circular(8),
                  child: LinearProgressIndicator(
                    value: c.micLevel,
                    minHeight: 8,
                    backgroundColor: scheme.surfaceContainerHighest,
                    color: c.pttActive
                        ? GravitalColors.pttActive
                        : scheme.primary,
                  ),
                ),
                const SizedBox(height: 6),
                Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    Text('Nivel de micrófono',
                        style: Theme.of(context).textTheme.labelSmall),
                    Text(
                      c.pttActive ? 'TX' : '—',
                      style: Theme.of(context).textTheme.labelSmall?.copyWith(
                          fontWeight: FontWeight.w700,
                          color: c.pttActive
                              ? GravitalColors.pttActive
                              : scheme.onSurfaceVariant),
                    ),
                  ],
                ),
                const SizedBox(height: 22),
                SectionCard(
                  title: 'Calidad de red',
                  subtitle: 'Métricas en vivo del protocolo',
                  child: GridView.count(
                    crossAxisCount: 2,
                    shrinkWrap: true,
                    physics: const NeverScrollableScrollPhysics(),
                    mainAxisSpacing: 10,
                    crossAxisSpacing: 10,
                    childAspectRatio: 2.1,
                    children: [
                      MetricTile(
                        label: 'RTT',
                        value: m.rttMs.toStringAsFixed(1),
                        unit: 'ms',
                        icon: Icons.speed,
                      ),
                      MetricTile(
                        label: 'Jitter',
                        value: m.jitterMs.toStringAsFixed(1),
                        unit: 'ms',
                        icon: Icons.waves,
                      ),
                      MetricTile(
                        label: 'Pérdida',
                        value: m.lossPercent.toStringAsFixed(1),
                        unit: '%',
                        icon: Icons.trending_down,
                        color: m.lossPercent > 5 ? scheme.error : null,
                      ),
                      MetricTile(
                        label: 'MOS',
                        value: m.estimatedMos.toStringAsFixed(2),
                        icon: Icons.star_outline,
                        color: m.estimatedMos >= 4
                            ? GravitalColors.online
                            : (m.estimatedMos >= 3 ? Colors.orange : scheme.error),
                      ),
                      MetricTile(
                        label: 'Buffer',
                        value: m.bufferFillPercent.toStringAsFixed(0),
                        unit: '%',
                        icon: Icons.storage,
                      ),
                      MetricTile(
                        label: 'Reorden.',
                        value: m.reorderPercent.toStringAsFixed(1),
                        unit: '%',
                        icon: Icons.swap_vert,
                      ),
                      MetricTile(
                        label: 'Paquetes RX',
                        value: '${m.packetsReceived}',
                        icon: Icons.download,
                      ),
                      MetricTile(
                        label: 'Paquetes TX',
                        value: '${m.packetsSent}',
                        icon: Icons.upload,
                      ),
                    ],
                  ),
                ),
                const SizedBox(height: 16),
                SectionCard(
                  title: 'Eventos de sesión',
                  child: EventLogView(log: c.log, max: 6),
                ),
                const SizedBox(height: 20),
                OutlinedButton.icon(
                  onPressed: () async {
                    await c.reset();
                    if (context.mounted) Navigator.pop(context);
                  },
                  icon: const Icon(Icons.call_end, color: Colors.red),
                  label: const Text('Finalizar sesión',
                      style: TextStyle(color: Colors.red)),
                ),
              ],
            ),
    );
  }
}

class _IdlePlaceholder extends StatelessWidget {
  const _IdlePlaceholder({required this.onBack});

  final VoidCallback onBack;

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.power_off,
                size: 56,
                color: Theme.of(context).colorScheme.onSurfaceVariant),
            const SizedBox(height: 16),
            Text('No hay sesión activa',
                style: Theme.of(context).textTheme.titleMedium),
            if (c.error != null) ...[
              const SizedBox(height: 8),
              Text(c.error!, textAlign: TextAlign.center),
            ],
            const SizedBox(height: 20),
            FilledButton(onPressed: onBack, child: const Text('Volver')),
          ],
        ),
      ),
    );
  }
}
