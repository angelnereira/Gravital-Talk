import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../models/session.dart';
import '../services/contacts.dart';
import '../services/engine.dart';
import '../services/session_controller.dart';
import '../services/share.dart';
import '../ui/components/index.dart';
import '../ui/patterns.dart';
import '../ui/semantic_colors.dart';
import '../ui/tokens.dart';

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
      message:
          'Se cortará la conversación. Para volver a hablar habrá que '
          'emparejar de nuevo.',
      confirmLabel: 'Salir',
      destructive: true,
    );
    if (!ok || !context.mounted) return;
    await c.reset();
    if (context.mounted) Navigator.pop(context);
  }

  /// Enseña la invitación de la sala sin salir de la conversación: es la
  /// misma que abrió la sala, así que quien la escanee entra directo.
  void _showInvite(BuildContext context, SessionController c) {
    final invite = c.pairingUri?.toUri();
    if (invite == null) return;
    showModalBottomSheet<void>(
      context: context,
      showDragHandle: true,
      isScrollControlled: true,
      builder: (_) => _InviteSheet(invite: invite, roomCode: c.roomCode ?? ''),
    );
  }

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;

    return Scaffold(
      appBar: AppBar(
        title: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            // Con quién, no en qué sala: el código de sala es un detalle
            // interno que ya nadie tiene que dictar.
            Text(switch (context.watch<ContactsService>().talkingWith) {
              final contact? => contact.name,
              null => 'Conversación',
            }),
            if (c.isLive)
              Text(
                // Sólo el anfitrión conoce el total: un invitado está
                // conectado a él y a nadie más.
                c.canInvite && c.roomSize > 2
                    ? '${_formatDuration(c.elapsed)} · ${c.roomSize} en la sala'
                    : _formatDuration(c.elapsed),
                style: theme.textTheme.labelSmall?.copyWith(
                  color: scheme.onSurfaceVariant,
                ),
              ),
          ],
        ),
        actions: [
          if (c.isLive)
            Padding(
              padding: const EdgeInsets.only(right: NSpace.lg),
              child: Center(
                child: _QualityChip(metrics: c.metrics, relayed: c.relayed),
              ),
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
                      NSpace.xl,
                      NSpace.md,
                      NSpace.xl,
                      NSpace.xxl,
                    ),
                    children: [
                      if (c.state == SessionState.reconnecting ||
                          c.connectionStalled) ...[
                        const ReconnectingBanner(),
                        const SizedBox(height: NSpace.md),
                      ],
                      if (c.engineKind == EngineKind.demo) ...[
                        const WarningBanner(
                          message: 'Modo demostración: no hay audio real.',
                        ),
                        const SizedBox(height: NSpace.md),
                      ],
                      if (c.paused) ...[
                        WarningBanner(
                          message:
                              'Sala en pausa: no oyes ni te oyen. '
                              'Sigues dentro.',
                          actionLabel: 'Reanudar',
                          onAction: () => c.setPaused(false),
                        ),
                        const SizedBox(height: NSpace.md),
                      ],
                      if (c.notice != null) ...[
                        WarningBanner(
                          message: c.notice!,
                          actionLabel: 'Entendido',
                          onAction: c.clearNotice,
                        ),
                        const SizedBox(height: NSpace.md),
                      ],
                      const SizedBox(height: NSpace.xl),
                      _TalkPanel(controller: c),
                      const SizedBox(height: NSpace.xxxl),
                      _TechnicalDetails(controller: c),
                      const SizedBox(height: NSpace.lg),
                      if (!c.paused) ...[
                        OutlinedButton.icon(
                          onPressed: () => c.setPaused(true),
                          icon: const Icon(Icons.pause_circle_outline),
                          label: const Text('Pausar'),
                        ),
                        const SizedBox(height: NSpace.md),
                      ],
                      if (c.canInvite) ...[
                        FilledButton.tonalIcon(
                          onPressed: () => _showInvite(context, c),
                          icon: const Icon(Icons.person_add_alt_1),
                          label: const Text('Invitar a alguien más'),
                        ),
                        const SizedBox(height: NSpace.md),
                      ],
                      OutlinedButton.icon(
                        onPressed: () => _confirmEndSession(context, c),
                        icon: Icon(Icons.call_end, color: scheme.error),
                        label: Text(
                          'Salir de la sala',
                          style: TextStyle(color: scheme.error),
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

/// Invitación de la sala en curso: código QR y enlace para copiar.
class _InviteSheet extends StatefulWidget {
  const _InviteSheet({required this.invite, required this.roomCode});

  final String invite;
  final String roomCode;

  @override
  State<_InviteSheet> createState() => _InviteSheetState();
}

class _InviteSheetState extends State<_InviteSheet> {
  bool _copied = false;

  /// Contactos ya avisados desde esta hoja.
  final _rung = <String>{};

  Future<void> _copy() async {
    await Clipboard.setData(ClipboardData(text: widget.invite));
    if (mounted) setState(() => _copied = true);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final contacts = context.watch<ContactsService>();
    return SafeArea(
      child: SingleChildScrollView(
        padding: const EdgeInsets.fromLTRB(NSpace.xl, 0, NSpace.xl, NSpace.xl),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Text(
              'Que escaneen este código',
              style: theme.textTheme.titleLarge?.copyWith(
                fontWeight: FontWeight.w800,
              ),
            ),
            const SizedBox(height: NSpace.xs),
            Text(
              'Entran en esta misma conversación, sin cortarla.',
              textAlign: TextAlign.center,
              style: theme.textTheme.bodyMedium,
            ),
            const SizedBox(height: NSpace.lg),
            Semantics(
              label: 'Código QR de invitación a la sala',
              child: Container(
                padding: const EdgeInsets.all(NSpace.md),
                decoration: BoxDecoration(
                  color: Colors.white,
                  borderRadius: BorderRadius.circular(NShape.extraLarge),
                ),
                child: QrImageView(
                  data: widget.invite,
                  size: 220,
                  backgroundColor: Colors.white,
                ),
              ),
            ),
            const SizedBox(height: NSpace.md),
            Text(
              widget.roomCode,
              style: theme.textTheme.titleLarge?.copyWith(
                fontWeight: FontWeight.w800,
                letterSpacing: 4,
              ),
            ),
            const SizedBox(height: NSpace.lg),
            Row(
              children: [
                Expanded(
                  child: FilledButton.tonalIcon(
                    onPressed: () async {
                      final shared = await shareText(
                        'Únete a mi sala de Gravital Talk:\n${widget.invite}',
                      );
                      if (!shared && mounted) setState(() => _copied = true);
                    },
                    icon: const Icon(Icons.ios_share),
                    label: const Text('Compartir'),
                  ),
                ),
                const SizedBox(width: NSpace.md),
                Expanded(
                  child: OutlinedButton.icon(
                    onPressed: _copy,
                    icon: Icon(_copied ? Icons.check : Icons.link),
                    label: Text(_copied ? 'Copiada' : 'Copiar'),
                  ),
                ),
              ],
            ),
            if (contacts.available && contacts.contacts.isNotEmpty) ...[
              const SizedBox(height: NSpace.xl),
              Align(
                alignment: Alignment.centerLeft,
                child: Text(
                  'O avisa a un contacto',
                  style: theme.textTheme.titleSmall?.copyWith(
                    fontWeight: FontWeight.w800,
                  ),
                ),
              ),
              for (final contact in contacts.contacts)
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  leading: const Icon(Icons.person_outline),
                  title: Text(contact.name),
                  trailing: _rung.contains(contact.id)
                      ? const Text('Avisado')
                      : TextButton(
                          onPressed: () {
                            if (contacts.ring(contact)) {
                              setState(() => _rung.add(contact.id));
                            }
                          },
                          child: const Text('Invitar'),
                        ),
                ),
            ],
          ],
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
    final scheme = theme.colorScheme;
    final sem = SemanticColors.of(context);
    final talking = c.pttActive;
    final listening = c.peerPttActive && !talking;

    return ValueListenableBuilder<AudioLevels>(
      valueListenable: c.levels,
      builder: (context, levels, _) {
        // El aviso de "te hablan" no depende sólo de la señalización: si
        // llega audio, alguien está hablando, se haya recibido o no su PTT.
        final hearing = listening || (!talking && levels.rx > 0.02);
        final (headline, detail, color) = talking
            ? ('Estás hablando', 'Suelta para escuchar', sem.pttActive.color)
            : hearing
            ? (
                'Te están hablando',
                'Espera a que termine para contestar',
                scheme.primary,
              )
            : (
                'Mantén pulsado para hablar',
                'El otro te oye mientras no sueltes',
                scheme.onSurface,
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
            const SizedBox(height: NSpace.xs),
            Text(
              detail,
              textAlign: TextAlign.center,
              style: theme.textTheme.bodyMedium?.copyWith(
                color: scheme.onSurfaceVariant,
              ),
            ),
            const SizedBox(height: NSpace.xxxl),
            PttButton(
              pressed: talking,
              enabled: c.isLive && !c.paused,
              peerSpeaking: hearing,
              level: level,
              onDown: c.pttDown,
              onUp: c.pttUp,
            ),
            const SizedBox(height: NSpace.xxxl),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: NSpace.xxxl),
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
  const _QualityChip({required this.metrics, required this.relayed});

  final SessionMetrics metrics;

  /// El audio va por un servidor intermedio: se dice, porque añade latencia
  /// y porque el usuario espera una conexión directa.
  final bool relayed;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final sem = SemanticColors.of(context);
    // Sin tráfico en los dos sentidos aún no hay nada que medir, y el MOS
    // que se calcula sobre nada sale bajo: no se afirma ni que va bien ni que
    // va mal, sólo que hay conexión.
    final measured = metrics.packetsReceived > 20 && metrics.rttMs > 0;
    final mos = measured ? metrics.estimatedMos : 0.0;
    final (label, color, icon) = mos <= 0
        ? (
            relayed ? 'Vía servidor' : 'Conectado',
            sem.success.color,
            relayed ? Icons.cloud_outlined : Icons.check_circle_outline,
          )
        : mos >= 4
        ? ('Buena señal', sem.success.color, Icons.signal_cellular_alt)
        : mos >= 3
        ? (
            'Señal regular',
            sem.pttActive.color,
            Icons.signal_cellular_alt_2_bar,
          )
        : ('Señal mala', scheme.error, Icons.signal_cellular_alt_1_bar);
    return Semantics(
      label: 'Calidad de la conexión: $label',
      child: ExcludeSemantics(
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 18, color: color),
            const SizedBox(width: NSpace.xs),
            Text(
              label,
              style: theme.textTheme.labelMedium?.copyWith(
                color: color,
                fontWeight: FontWeight.w700,
              ),
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
    final scheme = theme.colorScheme;
    final c = controller;
    final m = c.metrics;

    return Card(
      color: scheme.surfaceContainerLow,
      shape: NShape.rounded(NShape.medium),
      margin: EdgeInsets.zero,
      child: Theme(
        // Sin las líneas divisorias que `ExpansionTile` pinta al abrirse:
        // dentro de una tarjeta son ruido.
        data: theme.copyWith(dividerColor: Colors.transparent),
        child: ExpansionTile(
          shape: const Border(),
          collapsedShape: const Border(),
          iconColor: scheme.onSurfaceVariant,
          collapsedIconColor: scheme.onSurfaceVariant,
          trailing: const Icon(Icons.keyboard_arrow_down_rounded),
          tilePadding: const EdgeInsets.symmetric(
            horizontal: NSpace.xl,
            vertical: NSpace.xs,
          ),
          childrenPadding: const EdgeInsets.fromLTRB(
            NSpace.xl,
            0,
            NSpace.xl,
            NSpace.xl,
          ),
          title: Text(
            'Detalles técnicos',
            style: theme.textTheme.titleSmall?.copyWith(
              color: scheme.onSurface,
            ),
          ),
          subtitle: Text(
            'Latencia, pérdida y registro de la sesión',
            style: theme.textTheme.bodySmall?.copyWith(
              color: scheme.onSurfaceVariant,
            ),
          ),
          children: [
            GridView.count(
              crossAxisCount: 2,
              shrinkWrap: true,
              physics: const NeverScrollableScrollPhysics(),
              mainAxisSpacing: NSpace.sm,
              crossAxisSpacing: NSpace.sm,
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
                  label: 'Audio',
                  value: c.audioBitrate > 0
                      ? '${(c.audioBitrate / 1000).round()}'
                      : '—',
                  unit: c.audioBitrate > 0 ? 'kbps' : '',
                  icon: Icons.graphic_eq,
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
            const SizedBox(height: NSpace.lg),
            const ParticipantsCard(),
            const SizedBox(height: NSpace.lg),
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
    final scheme = theme.colorScheme;
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(NSpace.xxxl),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(Icons.call_end, size: 56, color: scheme.onSurfaceVariant),
            const SizedBox(height: NSpace.lg),
            Text(
              'La conversación terminó',
              style: theme.textTheme.titleLarge?.copyWith(
                color: scheme.onSurface,
              ),
            ),
            if (c.error != null) ...[
              const SizedBox(height: NSpace.sm),
              Text(
                c.error!,
                textAlign: TextAlign.center,
                style: theme.textTheme.bodyMedium?.copyWith(
                  color: scheme.onSurfaceVariant,
                ),
              ),
            ],
            const SizedBox(height: NSpace.xl),
            FilledButton(onPressed: onBack, child: const Text('Volver')),
          ],
        ),
      ),
    );
  }
}
