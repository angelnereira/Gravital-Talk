import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../core/glass.dart';
import '../core/states.dart';
import '../core/tokens.dart';
import '../models/session.dart';
import '../services/engine.dart';
import '../services/session_controller.dart';
import '../widgets/common.dart';

/// Pantalla de hablar.
///
/// Una sola cosa en el centro: el botón. Lo que rodea al botón responde a una
/// única pregunta —¿quién habla ahora?— y las métricas del protocolo quedan
/// plegadas en "Detalles técnicos". RTT, jitter y MOS son para diagnosticar,
/// no para conversar.
class SessionScreen extends StatelessWidget {
  const SessionScreen({super.key});

  String _formatDuration(Duration d) {
    final h = d.inHours;
    final m = d.inMinutes.remainder(60).toString().padLeft(2, '0');
    final s = d.inSeconds.remainder(60).toString().padLeft(2, '0');
    return h > 0 ? '$h:$m:$s' : '$m:$s';
  }

  /// Confirma antes de cortar: un toque accidental en plena conversación no
  /// debe descolgar la sesión.
  Future<void> _confirmEndSession(
    BuildContext context,
    SessionController c,
  ) async {
    final ok = await ConfirmDialog.show(
      context,
      title: 'Salir de la sala',
      message: 'Se cortará la conversación. Para volver a hablar habrá que '
          'emparejar de nuevo.',
      confirmLabel: 'Salir',
      destructive: true,
    );
    if (!ok || !context.mounted) return;
    await c.reset();
    if (context.mounted) Navigator.pop(context);
  }

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);

    return Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(c.roomCode != null ? 'Sala ${c.roomCode}' : 'Conversación'),
            if (c.isLive)
              Text(
                _formatDuration(c.elapsed),
                style: theme.textTheme.labelSmall
                    ?.copyWith(color: glass.secondary),
              ),
          ],
        ),
        actions: [
          if (c.isLive)
            Padding(
              padding: const EdgeInsets.only(right: Spacing.lg),
              child: Center(child: _QualityChip(metrics: c.metrics)),
            ),
        ],
      ),
      body: !c.isLive
          ? _IdlePlaceholder(onBack: () => Navigator.pop(context))
          : SafeArea(
              child: Center(
                child: ConstrainedBox(
                  constraints: const BoxConstraints(maxWidth: 520),
                  child: ListView(
                    padding: const EdgeInsets.fromLTRB(
                        Spacing.xl, Spacing.md, Spacing.xl, Spacing.xxl),
                    children: [
                      if (c.state == SessionState.reconnecting) ...[
                        const ReconnectingBanner(),
                        const SizedBox(height: Spacing.md),
                      ],
                      if (c.engineKind == EngineKind.demo) ...[
                        const WarningBanner(
                          message: 'Modo demostración: no hay audio real.',
                        ),
                        const SizedBox(height: Spacing.md),
                      ],
                      if (c.notice != null) ...[
                        WarningBanner(
                          message: c.notice!,
                          actionLabel: 'Entendido',
                          onAction: c.clearNotice,
                        ),
                        const SizedBox(height: Spacing.md),
                      ],
                      const SizedBox(height: Spacing.xl),
                      _TalkPanel(controller: c),
                      const SizedBox(height: Spacing.xxxl),
                      _TechnicalDetails(controller: c),
                      const SizedBox(height: Spacing.lg),
                      OutlinedButton.icon(
                        onPressed: () => _confirmEndSession(context, c),
                        icon: Icon(Icons.call_end,
                            color: theme.colorScheme.error),
                        label: Text(
                          'Salir de la sala',
                          style: TextStyle(color: theme.colorScheme.error),
                        ),
                      ),
                    ],
                  ),
                ),
              ),
            ),
    );
  }
}

/// El botón y, encima, quién tiene la palabra.
///
/// Los niveles llegan por `controller.levels`, que cambia varias veces por
/// segundo: sólo este panel se repinta con él, no la pantalla.
class _TalkPanel extends StatelessWidget {
  const _TalkPanel({required this.controller});

  final SessionController controller;

  @override
  Widget build(BuildContext context) {
    final c = controller;
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    final talking = c.pttActive;
    final listening = c.peerPttActive && !talking;

    return ValueListenableBuilder<AudioLevels>(
      valueListenable: c.levels,
      builder: (context, levels, _) {
        // El aviso de "te hablan" no depende sólo de la señalización: si
        // llega audio, alguien está hablando, se haya recibido o no su PTT.
        final hearing = listening || (!talking && levels.rx > 0.02);
        final (headline, detail, color) = talking
            ? (
                'Estás hablando',
                'Suelta para escuchar',
                GravitalColors.transmit,
              )
            : hearing
                ? (
                    'Te están hablando',
                    'Espera a que termine para contestar',
                    theme.colorScheme.primary,
                  )
                : (
                    'Mantén pulsado para hablar',
                    'El otro te oye mientras no sueltes',
                    glass.primary,
                  );
        final level = talking ? levels.mic : (hearing ? levels.rx : 0.0);

        return Column(
          children: [
            Semantics(
              liveRegion: true,
              child: Text(
                headline,
                textAlign: TextAlign.center,
                style: theme.textTheme.headlineSmall?.copyWith(
                  fontWeight: FontWeight.w800,
                  color: color,
                ),
              ),
            ),
            const SizedBox(height: Spacing.xs),
            Text(
              detail,
              textAlign: TextAlign.center,
              style:
                  theme.textTheme.bodyMedium?.copyWith(color: glass.secondary),
            ),
            const SizedBox(height: Spacing.xxxl),
            PttButton(
              pressed: talking,
              enabled: c.isLive,
              peerSpeaking: hearing,
              level: level,
              onDown: c.pttDown,
              onUp: c.pttUp,
            ),
            const SizedBox(height: Spacing.xxxl),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: Spacing.xxxl),
              child: LevelMeter(level: level, transmitting: talking),
            ),
          ],
        );
      },
    );
  }
}

/// Calidad de la conexión en una palabra.
///
/// Resume el MOS, que nadie fuera del gremio sabe leer, en tres estados.
class _QualityChip extends StatelessWidget {
  const _QualityChip({required this.metrics});

  final SessionMetrics metrics;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    // Sin tráfico en los dos sentidos aún no hay nada que medir, y el MOS
    // que se calcula sobre nada sale bajo: no se afirma ni que va bien ni que
    // va mal, sólo que hay conexión.
    final measured = metrics.packetsReceived > 20 && metrics.rttMs > 0;
    final mos = measured ? metrics.estimatedMos : 0.0;
    final (label, color, icon) = mos <= 0
        ? ('Conectado', GravitalColors.online, Icons.check_circle_outline)
        : mos >= 4
            ? ('Buena señal', GravitalColors.online, Icons.signal_cellular_alt)
            : mos >= 3
                ? (
                    'Señal regular',
                    GravitalColors.transmit,
                    Icons.signal_cellular_alt_2_bar,
                  )
                : (
                    'Señal mala',
                    theme.colorScheme.error,
                    Icons.signal_cellular_alt_1_bar,
                  );
    return Semantics(
      label: 'Calidad de la conexión: $label',
      child: ExcludeSemantics(
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 18, color: color),
            const SizedBox(width: Spacing.xs),
            Text(
              label,
              style: theme.textTheme.labelMedium
                  ?.copyWith(color: color, fontWeight: FontWeight.w700),
            ),
          ],
        ),
      ),
    );
  }
}

/// Métricas del protocolo y registro, plegados por defecto.
class _TechnicalDetails extends StatelessWidget {
  const _TechnicalDetails({required this.controller});

  final SessionController controller;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    final scheme = theme.colorScheme;
    final c = controller;
    final m = c.metrics;

    return GlassSurface(
      padding: EdgeInsets.zero,
      child: Theme(
        data: theme.copyWith(dividerColor: Colors.transparent),
        child: ExpansionTile(
          shape: const Border(),
          collapsedShape: const Border(),
          iconColor: glass.secondary,
          collapsedIconColor: glass.secondary,
          trailing: const Icon(Icons.keyboard_arrow_down_rounded),
          tilePadding: const EdgeInsets.symmetric(
              horizontal: Spacing.xl, vertical: Spacing.xs),
          childrenPadding: const EdgeInsets.fromLTRB(
              Spacing.xl, 0, Spacing.xl, Spacing.xl),
          title: Text(
            'Detalles técnicos',
            style: theme.textTheme.titleSmall?.copyWith(color: glass.primary),
          ),
          subtitle: Text(
            'Latencia, pérdida y registro de la sesión',
            style: theme.textTheme.bodySmall?.copyWith(color: glass.secondary),
          ),
          children: [
            GridView.count(
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
                ValueListenableBuilder<AudioLevels>(
                  valueListenable: c.levels,
                  builder: (context, l, _) => MetricTile(
                    label: 'Audio RX',
                    value: '${l.rxFrames}',
                    unit: 'frames',
                    icon: Icons.hearing,
                  ),
                ),
                ValueListenableBuilder<AudioLevels>(
                  valueListenable: c.levels,
                  builder: (context, l, _) => MetricTile(
                    label: 'Audio TX',
                    value: '${l.txFrames}',
                    unit: 'frames',
                    icon: Icons.mic_none,
                  ),
                ),
              ],
            ),
            const SizedBox(height: Spacing.lg),
            const ParticipantsCard(),
            const SizedBox(height: Spacing.lg),
            EventLogView(log: c.log, max: 6),
          ],
        ),
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
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(Spacing.xxxl),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.call_end, size: 56, color: glass.secondary),
            const SizedBox(height: Spacing.lg),
            Text(
              'La conversación terminó',
              style:
                  theme.textTheme.titleLarge?.copyWith(color: glass.primary),
            ),
            if (c.error != null) ...[
              const SizedBox(height: Spacing.sm),
              Text(
                c.error!,
                textAlign: TextAlign.center,
                style: theme.textTheme.bodyMedium
                    ?.copyWith(color: glass.secondary),
              ),
            ],
            const SizedBox(height: Spacing.xl),
            FilledButton(onPressed: onBack, child: const Text('Volver')),
          ],
        ),
      ),
    );
  }
}
