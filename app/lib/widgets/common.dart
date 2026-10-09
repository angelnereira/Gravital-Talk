import 'package:animate_do/animate_do.dart';
import 'package:flutter/material.dart';
import 'package:percent_indicator/linear_percent_indicator.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:provider/provider.dart';

import '../core/tokens.dart';
import '../models/session.dart';
import '../services/event_log.dart';
import '../services/room_events.dart';
import '../services/session_controller.dart';

/// Badge de estado de la sesión.
class StatusBadge extends StatelessWidget {
  const StatusBadge(this.state, {super.key});

  final SessionState state;

  Color _color(ColorScheme scheme) {
    switch (state) {
      case SessionState.active:
        return const Color(0xFF00C853);
      case SessionState.handshaking:
      case SessionState.reconnecting:
        return Colors.orange;
      case SessionState.error:
        return scheme.error;
      case SessionState.closed:
        return scheme.outline;
      default:
        return scheme.secondary;
    }
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final color = _color(scheme);
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 5),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.14),
        borderRadius: BorderRadius.circular(999),
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
          const SizedBox(width: 8),
          Text(state.label,
              style: TextStyle(
                  color: color, fontWeight: FontWeight.w600, fontSize: 12)),
        ],
      ),
    );
  }
}

/// Tarjeta contenedora con título.
class SectionCard extends StatelessWidget {
  const SectionCard({
    super.key,
    required this.title,
    this.subtitle,
    this.trailing,
    required this.child,
  });

  final String title;
  final String? subtitle;
  final Widget? trailing;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              children: [
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text(title,
                          style: theme.textTheme.titleMedium
                              ?.copyWith(fontWeight: FontWeight.w700)),
                      if (subtitle != null)
                        Padding(
                          padding: const EdgeInsets.only(top: 2),
                          child: Text(subtitle!,
                              style: theme.textTheme.bodySmall?.copyWith(
                                  color: theme.colorScheme.onSurfaceVariant)),
                        ),
                    ],
                  ),
                ),
                if (trailing != null) ?trailing,
              ],
            ),
            const SizedBox(height: 14),
            child,
          ],
        ),
      ),
    );
  }
}

/// Campo de texto con etiqueta y estilo uniforme.
class LabeledField extends StatelessWidget {
  const LabeledField({
    super.key,
    required this.label,
    required this.controller,
    this.hint,
    this.keyboardType,
    this.obscure = false,
    this.suffix,
    this.validator,
  });

  final String label;
  final TextEditingController controller;
  final String? hint;
  final TextInputType? keyboardType;
  final bool obscure;
  final Widget? suffix;
  final String? Function(String?)? validator;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: TextFormField(
        controller: controller,
        keyboardType: keyboardType,
        obscureText: obscure,
        validator: validator,
        decoration: InputDecoration(labelText: label, hintText: hint, suffix: suffix),
      ),
    );
  }
}

/// Tile de una métrica.
class MetricTile extends StatelessWidget {
  const MetricTile({
    super.key,
    required this.label,
    required this.value,
    this.unit = '',
    this.icon,
    this.color,
  });

  final String label;
  final String value;
  final String unit;
  final IconData? icon;
  final Color? color;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final tint = color ?? theme.colorScheme.primary;
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: tint.withValues(alpha: 0.07),
        borderRadius: BorderRadius.circular(14),
        border: Border.all(color: tint.withValues(alpha: 0.25)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              if (icon != null) ...[
                Icon(icon, size: 14, color: tint),
                const SizedBox(width: 5),
              ],
              Expanded(
                child: Text(label,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: theme.textTheme.labelSmall
                        ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
              ),
            ],
          ),
          const SizedBox(height: 6),
          Row(
            crossAxisAlignment: CrossAxisAlignment.baseline,
            textBaseline: TextBaseline.alphabetic,
            children: [
              Text(value,
                  style: theme.textTheme.titleMedium
                      ?.copyWith(fontWeight: FontWeight.w800)),
              if (unit.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.only(left: 2),
                  child: Text(unit,
                      style: theme.textTheme.labelSmall
                          ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
                ),
            ],
          ),
        ],
      ),
    );
  }
}

/// Tarjeta grande de selección de modo.
class ModeCard extends StatelessWidget {
  const ModeCard({
    super.key,
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.color,
    required this.onTap,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final Color color;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Card(
      clipBehavior: Clip.antiAlias,
      child: InkWell(
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.all(18),
          child: Row(
            children: [
              Container(
                width: 52,
                height: 52,
                decoration: BoxDecoration(
                  color: color.withValues(alpha: 0.14),
                  borderRadius: BorderRadius.circular(16),
                ),
                child: Icon(icon, color: color, size: 28),
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(title,
                        style: theme.textTheme.titleMedium
                            ?.copyWith(fontWeight: FontWeight.w700)),
                    const SizedBox(height: 4),
                    Text(subtitle,
                        style: theme.textTheme.bodySmall?.copyWith(
                            color: theme.colorScheme.onSurfaceVariant)),
                  ],
                ),
              ),
              const Icon(Icons.chevron_right),
            ],
          ),
        ),
      ),
    );
  }
}

/// Panel con QR del room code.
class QrPanel extends StatelessWidget {
  const QrPanel({super.key, required this.code, this.caption});

  final String code;
  final String? caption;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Container(
          padding: const EdgeInsets.all(10),
          decoration: BoxDecoration(
            color: Colors.white,
            borderRadius: BorderRadius.circular(16),
          ),
          child: QrImageView(
            data: code,
            size: 150,
            backgroundColor: Colors.white,
          ),
        ),
        const SizedBox(height: 10),
        SelectableText(
          code,
          style: Theme.of(context)
              .textTheme
              .titleLarge
              ?.copyWith(fontWeight: FontWeight.w800, letterSpacing: 2),
        ),
        if (caption != null)
          Padding(
            padding: const EdgeInsets.only(top: 4),
            child: Text(caption!,
                style: Theme.of(context).textTheme.bodySmall),
          ),
      ],
    );
  }
}

/// Lista compacta de eventos.
class EventLogView extends StatelessWidget {
  const EventLogView({super.key, required this.log, this.max = 6});

  final EventLog log;
  final int max;

  @override
  Widget build(BuildContext context) {
    final entries = log.entries.take(max).toList();
    if (entries.isEmpty) {
      return Text('Sin eventos todavía.',
          style: Theme.of(context).textTheme.bodySmall);
    }
    final theme = Theme.of(context);
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
                      LogLevel.success => const Color(0xFF00C853),
                      LogLevel.warn => Colors.orange,
                      LogLevel.error => theme.colorScheme.error,
                      LogLevel.info => theme.colorScheme.primary,
                    },
                  ),
                ),
                const SizedBox(width: 8),
                Expanded(
                  child: Text(e.message, style: theme.textTheme.bodySmall),
                ),
                Text(
                  '${e.time.hour.toString().padLeft(2, '0')}:${e.time.minute.toString().padLeft(2, '0')}:${e.time.second.toString().padLeft(2, '0')}',
                  style: theme.textTheme.labelSmall
                      ?.copyWith(color: theme.colorScheme.onSurfaceVariant),
                ),
              ],
            ),
          ),
      ],
    );
  }
}

/// Botón PTT grande con estados (pulso animado al transmitir).
class PttButton extends StatelessWidget {
  const PttButton({
    super.key,
    required this.pressed,
    required this.enabled,
    required this.onDown,
    required this.onUp,
    this.peerSpeaking = false,
  });

  final bool pressed;
  final bool enabled;
  final VoidCallback onDown;
  final VoidCallback onUp;
  final bool peerSpeaking;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final base = pressed ? GravitalColors.pttActive : scheme.primary;
    final button = GestureDetector(
      onTapDown: enabled ? (_) => onDown() : null,
      onTapUp: enabled ? (_) => onUp() : null,
      onTapCancel: enabled ? onUp : null,
      child: AnimatedContainer(
        duration: const Duration(milliseconds: 150),
        height: 132,
        decoration: BoxDecoration(
          shape: BoxShape.circle,
          gradient: RadialGradient(
            colors: [
              base.withValues(alpha: pressed ? 1 : 0.85),
              base.withValues(alpha: pressed ? 0.75 : 0.55),
            ],
          ),
          boxShadow: [
            BoxShadow(
              color: base.withValues(alpha: 0.45),
              blurRadius: pressed ? 42 : 18,
              spreadRadius: pressed ? 6 : 0,
            ),
          ],
        ),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Icon(
              peerSpeaking ? Icons.hearing : Icons.mic,
              color: Colors.white,
              size: 34,
            ),
            const SizedBox(height: 6),
            Text(
              pressed ? 'TRANSMITIENDO' : 'MANTÉN PARA HABLAR',
              textAlign: TextAlign.center,
              style: const TextStyle(
                color: Colors.white,
                fontWeight: FontWeight.w800,
                fontSize: 11,
                letterSpacing: 1.1,
              ),
            ),
          ],
        ),
      ),
    );

    // Pulso continuo mientras se transmite (animate_do).
    return Pulse(
      animate: pressed,
      infinite: true,
      duration: const Duration(milliseconds: 900),
      from: 1.0,
      to: 1.04,
      child: button,
    );
  }
}

/// Barra de nivel de micrófono animada (percent_indicator).
class LevelMeter extends StatelessWidget {
  const LevelMeter({super.key, required this.level, this.transmitting = false});

  final double level;
  final bool transmitting;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return LinearPercentIndicator(
      percent: level.clamp(0.0, 1.0),
      animation: true,
      animationDuration: 120,
      lineHeight: 8,
      barRadius: const Radius.circular(8),
      backgroundColor: scheme.surfaceContainerHighest,
      progressColor:
          transmitting ? GravitalColors.pttActive : scheme.primary,
    );
  }
}

class GravitalColors {
  static const pttActive = Color(0xFFFF3D00);
  static const online = Color(0xFF00C853);

  const GravitalColors._();
}

/// Banner de reconexión: la sesión se está recuperando sola.
///
/// Sin esto, un corte de red parece una sesión congelada: el PTT no responde y
/// no hay explicación. El backoff vive en el controller (`2s -> 30s`).
class ReconnectingBanner extends StatelessWidget {
  const ReconnectingBanner({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Container(
      padding: const EdgeInsets.all(Spacing.md),
      decoration: BoxDecoration(
        color: theme.colorScheme.tertiaryContainer,
        borderRadius: BorderRadius.circular(Radii.md),
      ),
      child: Row(
        children: [
          SizedBox(
            width: 16,
            height: 16,
            child: CircularProgressIndicator(
              strokeWidth: 2,
              valueColor: AlwaysStoppedAnimation<Color>(
                theme.colorScheme.onTertiaryContainer,
              ),
            ),
          ),
          const SizedBox(width: Spacing.md),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'Reconectando…',
                  style: theme.textTheme.labelLarge?.copyWith(
                    color: theme.colorScheme.onTertiaryContainer,
                  ),
                ),
                Text(
                  'La sesión se recupera sola tras un corte de red',
                  style: theme.textTheme.bodySmall?.copyWith(
                    color: theme.colorScheme.onTertiaryContainer
                        .withValues(alpha: 0.8),
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

/// Lista de participantes de la sala.
///
/// Consume el stream `WatchRoom` del relay (`PEER_JOINED`, `PEER_LEFT`,
/// `FLOOR_GRANTED`, `FLOOR_RELEASED`), que la app no usaba: la pantalla de
/// sesión mostraba métricas pero no quién estaba en la sala ni quién tenía el
/// turno. El floor es lo que hace útil el PTT, así que mostrarlo es mostrar la
/// característica, no un adorno.
class ParticipantsCard extends StatelessWidget {
  const ParticipantsCard({super.key});

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final events = context.watch<RoomEvents>();
    final controller = context.watch<SessionController>();

    final participants = <Participant>[
      Participant(
        ssrc: controller.sessionId,
        isLocal: true,
        hasFloor: events.floorHolder == controller.sessionId,
      ),
      for (final ssrc in events.peers)
        Participant(
          ssrc: ssrc,
          hasFloor: events.floorHolder == ssrc,
        ),
    ];

    return SectionCard(
      title: 'Participantes',
      subtitle: switch (events.isConnected) {
        true => '${participants.length} en la sala',
        false when events.roomCode == null => 'Sin observación de sala',
        false => 'Observación de sala perdida',
      },
      trailing: events.isConnected
          ? Icon(Icons.circle, size: 10, color: GravitalColors.online)
          : Icon(Icons.cloud_off, size: 18, color: theme.colorScheme.outline),
      child: Column(
        children: [
          for (final p in participants)
            Padding(
              padding: const EdgeInsets.only(bottom: Spacing.sm),
              child: Row(
                children: [
                  Container(
                    width: 32,
                    height: 32,
                    decoration: BoxDecoration(
                      color: p.hasFloor
                          ? GravitalColors.pttActive.withValues(alpha: 0.15)
                          : theme.colorScheme.surfaceContainerHighest,
                      shape: BoxShape.circle,
                    ),
                    child: Icon(
                      p.hasFloor
                          ? Icons.graphic_eq
                          : (p.isLocal ? Icons.person : Icons.people),
                      size: 16,
                      color: p.hasFloor
                          ? GravitalColors.pttActive
                          : theme.colorScheme.onSurfaceVariant,
                    ),
                  ),
                  const SizedBox(width: Spacing.md),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          p.isLocal ? '${p.label} (tú)' : p.label,
                          style: theme.textTheme.bodyMedium?.copyWith(
                            fontWeight:
                                p.hasFloor ? FontWeight.w700 : FontWeight.w500,
                          ),
                        ),
                        Text(
                          p.statusLabel,
                          style: theme.textTheme.labelSmall?.copyWith(
                            color: p.hasFloor
                                ? GravitalColors.pttActive
                                : theme.colorScheme.onSurfaceVariant,
                          ),
                        ),
                      ],
                    ),
                  ),
                  if (p.hasFloor)
                    Container(
                      padding: const EdgeInsets.symmetric(
                        horizontal: Spacing.sm,
                        vertical: Spacing.xxs,
                      ),
                      decoration: BoxDecoration(
                        color: GravitalColors.pttActive,
                        borderRadius: BorderRadius.circular(Radii.pill),
                      ),
                      child: Text(
                        'TURNO',
                        style: theme.textTheme.labelSmall?.copyWith(
                          color: Colors.white,
                          fontWeight: FontWeight.w800,
                          letterSpacing: 0.6,
                        ),
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
