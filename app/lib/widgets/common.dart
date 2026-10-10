import 'package:flutter/material.dart';
import 'package:percent_indicator/linear_percent_indicator.dart';
import 'package:qr_flutter/qr_flutter.dart';
import 'package:provider/provider.dart';

import '../core/palette.dart';
import '../core/glass.dart';
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
    final glassText = GlassTextTheme.of(context);
    return GlassSurface(
      padding: const EdgeInsets.all(Spacing.xl),
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
                        style: theme.textTheme.titleMedium?.copyWith(
                            fontWeight: FontWeight.w700,
                            color: glassText.primary)),
                    if (subtitle != null)
                      Padding(
                        padding: const EdgeInsets.only(top: 2),
                        child: Text(subtitle!,
                            style: theme.textTheme.bodySmall?.copyWith(
                                color: glassText.secondary)),
                      ),
                  ],
                ),
              ),
              if (trailing != null) ?trailing,
            ],
          ),
          const SizedBox(height: Spacing.lg),
          child,
        ],
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
    this.errorText,
    this.onChanged,
  });

  final String label;
  final TextEditingController controller;
  final String? hint;
  final TextInputType? keyboardType;
  final bool obscure;
  final Widget? suffix;
  final String? Function(String?)? validator;

  /// Mensaje de error del campo, para mostrarlo en línea.
  final String? errorText;

  /// Se invoca al cambiar el texto, para poder limpiar el error anterior.
  final ValueChanged<String>? onChanged;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: TextFormField(
        controller: controller,
        keyboardType: keyboardType,
        obscureText: obscure,
        validator: validator,
        onChanged: onChanged,
        autovalidateMode: AutovalidateMode.disabled,
        decoration: InputDecoration(
          labelText: label,
          hintText: hint,
          suffix: suffix,
          errorText: errorText,
          errorMaxLines: 2,
        ),
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

  /// Tono del icono cuando tiene que distinguirse (p. ej. estado activo).
  ///
  /// Antes se usaba como relleno de un cuadrado al 14 %, que era lo que
  /// cargaba la pantalla. Ahora sólo tiñe el icono.
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
              // Icono monócromo sobre la superficie, sin cuadrado de color
              // detrás. Es lo que hace la referencia de Pixel, y es lo que
              // quita el bloque de teal saturado que dominaba cada fila.
              //
              // El `color` del widget se respeta sólo como tono del icono;
              // nunca como relleno. Si todas las tarjetas pintaran su icono a
              // todo color, ninguna destacaría.
              Icon(icon, color: theme.colorScheme.onSurfaceVariant, size: 26),
              const SizedBox(width: Spacing.lg),
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

/// Botón PTT: el anillo de señal de la marca.
///
/// Es la pieza más distintiva de la app, así que es donde se concentra la
/// identidad. Antes era un círculo con gradiente radial —que es lo que hace
/// cualquier botón circular de cualquier tutorial—. Ahora es un **anillo que se
/// llena con el nivel de audio**, como un indicador de "en el aire" de equipo de
/// radiodifusión.
///
/// El color también significa algo: ámbar al transmitir (convención real de
/// broadcast), no naranja por elección estética.
class PttButton extends StatefulWidget {
  const PttButton({
    super.key,
    required this.pressed,
    required this.enabled,
    required this.onDown,
    required this.onUp,
    this.peerSpeaking = false,
    this.level = 0,
  });

  final bool pressed;
  final bool enabled;
  final VoidCallback onDown;
  final VoidCallback onUp;

  /// El otro participante está transmitiendo.
  final bool peerSpeaking;

  /// Nivel de audio actual, 0..1. Rellena el anillo.
  final double level;

  @override
  State<PttButton> createState() => _PttButtonState();
}

class _PttButtonState extends State<PttButton> {
  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final pressed = widget.pressed;
    final enabled = widget.enabled;
    final level = widget.level.clamp(0.0, 1.0);

    // Ámbar al transmitir; el resto del tiempo, el color de marca.
    final signalColor = pressed
        ? GlassAccent.transmit
        : (widget.peerSpeaking ? scheme.tertiary : scheme.primary);

    final disc = GestureDetector(
      onTapDown: enabled ? (_) => widget.onDown() : null,
      onTapUp: enabled ? (_) => widget.onUp() : null,
      onTapCancel: enabled ? widget.onUp : null,
      child: Semantics(
        button: true,
        enabled: enabled,
        label: pressed
            ? 'Transmitiendo. Suelta para dejar de hablar'
            : 'Mantén pulsado para hablar',
        hint: widget.peerSpeaking
            ? 'El otro participante está transmitiendo'
            : null,
        child: SizedBox(
          width: HitSizes.pttButton,
          height: HitSizes.pttButton,
          child: CustomPaint(
            painter: _PttRingPainter(
              level: level,
              color: signalColor,
              trackColor: scheme.outlineVariant,
              glow: pressed,
            ),
            child: Center(
              child: Padding(
                // El contenido respeta el grosor del anillo.
                padding: const EdgeInsets.all(Spacing.xl),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Icon(
                      widget.peerSpeaking && !pressed
                          ? Icons.hearing
                          : Icons.mic,
                      color: signalColor,
                      size: 34,
                    ),
                    const SizedBox(height: Spacing.sm),
                    Text(
                      pressed ? 'TRANSMITIENDO' : 'PULSA PARA HABLAR',
                      textAlign: TextAlign.center,
                      style: TextStyle(
                        color: enabled ? signalColor : scheme.outline,
                        fontWeight: FontWeight.w800,
                        fontSize: 10,
                        letterSpacing: 0.8,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );

    // El anillo va sobre un disco de vidrio: sin el volumen del cristal, el
    // botón más importante de la app quedaba como un aro plano sobre el fondo.
    // El resplandor sólo aparece al transmitir, cuando el vidrio debe parecer
    // que emite luz propia.
    final button = GlassSurface(
      borderRadius: Radii.pill,
      width: HitSizes.pttButton,
      height: HitSizes.pttButton,
      blur: 14,
      padding: EdgeInsets.zero,
      glow: pressed ? signalColor.withValues(alpha: 0.55) : null,
      child: disc,
    );

    return AnimatedScale(
      scale: pressed ? 1.04 : 1.0,
      duration: GravitalMotion.instant,
      curve: Curves.easeOut,
      child: button,
    );
  }
}

/// Pinta el anillo de señal del botón PTT.
///
/// Tres capas, en este orden:
/// 1. la pista (círculo vacío),
/// 2. el arco que representa el nivel de audio,
/// 3. el resplandor, sólo al transmitir.
///
/// Se hace con `CustomPaint` en vez de apilar widgets porque un arco
/// parcial no es composable con formas estándar, y porque pintar un arco es
/// considerablemente más barato que animar la visibilidad de veinte
/// segmentos.
class _PttRingPainter extends CustomPainter {
  const _PttRingPainter({
    required this.level,
    required this.color,
    required this.trackColor,
    required this.glow,
  });

  final double level;
  final Color color;
  final Color trackColor;
  final bool glow;

  /// Grosor del anillo.
  static const _stroke = 10.0;

  @override
  void paint(Canvas canvas, Size size) {
    final center = Offset(size.width / 2, size.height / 2);
    final radius = (size.shortestSide - _stroke) / 2;

    // 1. Pista.
    final track = Paint()
      ..color = trackColor
      ..style = PaintingStyle.stroke
      ..strokeWidth = _stroke
      ..strokeCap = StrokeCap.round;
    canvas.drawCircle(center, radius, track);

    // 2. Arco de nivel. Empieza arriba (-90 grados) y gira en sentido horario.
    if (level > 0.01) {
      final arc = Paint()
        ..color = color
        ..style = PaintingStyle.stroke
        ..strokeWidth = _stroke
        ..strokeCap = StrokeCap.round;
      canvas.drawArc(
        Rect.fromCircle(center: center, radius: radius),
        -90 * 3.141592653589793 / 180,
        2 * 3.141592653589793 * level,
        false,
        arc,
      );
    }

    // 3. Resplandor al transmitir. Se pinta ANTES del contenido: si fuera
    // después lo taparía.
    if (glow) {
      final halo = Paint()
        ..color = color.withValues(alpha: 0.18)
        ..maskFilter = const MaskFilter.blur(BlurStyle.normal, 18);
      canvas.drawCircle(center, radius, halo);
    }
  }

  @override
  bool shouldRepaint(_PttRingPainter old) =>
      old.level != level || old.color != color || old.glow != glow;
}

/// Barra de nivel de micrófono animada (percent_indicator).
class LevelMeter extends StatelessWidget {
  const LevelMeter({super.key, required this.level, this.transmitting = false});

  final double level;
  final bool transmitting;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Semantics(
      // El nivel es informativo, no un control: se anuncia su valor para que
      // un usuario sin vista sepa si el micrófono está captando.
      label: 'Nivel de micrófono',
      value: '${(level.clamp(0.0, 1.0) * 100).round()}%',
      child: ExcludeSemantics(
        child: LinearPercentIndicator(
          percent: level.clamp(0.0, 1.0),
          animation: true,
          animationDuration: 120,
          lineHeight: 8,
          barRadius: const Radius.circular(8),
          backgroundColor: scheme.surfaceContainerHighest,
          progressColor:
              transmitting ? GravitalColors.transmit : scheme.primary,
        ),
      ),
    );
  }
}

/// Colores semánticos usados por los widgets.
///
/// Apuntan a `GravitalPalette` en vez de repetir los valores: antes cada widget
/// tenía su propia copia del "verde de conectado", y cada copia se había
/// desviado un poco.
class GravitalColors {
  static const transmit = GravitalPalette.transmit;
  static const online = GravitalPalette.online;

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
                          ? GravitalColors.transmit.withValues(alpha: 0.15)
                          : theme.colorScheme.surfaceContainerHighest,
                      shape: BoxShape.circle,
                    ),
                    child: Icon(
                      p.hasFloor
                          ? Icons.graphic_eq
                          : (p.isLocal ? Icons.person : Icons.people),
                      size: 16,
                      color: p.hasFloor
                          ? GravitalColors.transmit
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
                                ? GravitalColors.transmit
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
                        color: GravitalColors.transmit,
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
