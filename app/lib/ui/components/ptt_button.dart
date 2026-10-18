/// Botón Push-To-Talk principal (GEUI).
///
/// Es la pieza más distintiva de la app. Un anillo que se llena con el nivel
/// de audio, como un indicador de "en el aire" de equipo de radiodifusión.
/// El color también significa algo: ámbar al transmitir.
library;

import 'package:flutter/material.dart';
import 'package:flutter/gestures.dart';

import '../../services/haptics.dart';
import '../semantic_colors.dart';
import '../tokens.dart';

/// Botón PTT: el anillo de señal de la marca.
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
  final bool peerSpeaking;
  final double level;

  @override
  State<PttButton> createState() => _PttButtonState();
}

/// Lo que hay que subir el dedo, hablando, para bloquear el turno.
const pttLockDistance = 72.0;

class _PttButtonState extends State<PttButton> {
  int? _pointer;
  double _downY = 0;
  bool _locked = false;
  bool _unlocking = false;

  void _down(PointerDownEvent event) {
    if (_pointer != null) return;
    if (_locked) {
      _pointer = event.pointer;
      _unlocking = true;
      setState(() => _locked = false);
      Haptics.pttUp();
      widget.onUp();
      return;
    }
    if (!widget.enabled) return;
    _pointer = event.pointer;
    _downY = event.position.dy;
    Haptics.pttDown();
    widget.onDown();
  }

  void _move(PointerMoveEvent event) {
    if (event.pointer != _pointer || _unlocking || _locked) return;
    if (_downY - event.position.dy > pttLockDistance) {
      Haptics.notice();
      setState(() => _locked = true);
    }
  }

  void _release(PointerEvent event) {
    if (event.pointer != _pointer) return;
    _pointer = null;
    if (_unlocking) {
      _unlocking = false;
      return;
    }
    if (_locked) return;
    Haptics.pttUp();
    widget.onUp();
  }

  @override
  void didUpdateWidget(PttButton old) {
    super.didUpdateWidget(old);
    if (_locked && old.pressed && !widget.pressed) _locked = false;
  }

  @override
  void dispose() {
    if ((_pointer != null && !_unlocking) || _locked) widget.onUp();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    final sem = SemanticColors.of(context);
    final pressed = widget.pressed;
    final enabled = widget.enabled;
    final level = widget.level.clamp(0.0, 1.0);

    final signalColor = pressed
        ? sem.pttActive.color
        : (widget.peerSpeaking ? scheme.tertiary : scheme.primary);

    final disc = Listener(
      onPointerDown: _down,
      onPointerMove: _move,
      onPointerUp: _release,
      onPointerCancel: _release,
      child: RawGestureDetector(
        gestures: {
          EagerGestureRecognizer:
              GestureRecognizerFactoryWithHandlers<EagerGestureRecognizer>(
            EagerGestureRecognizer.new,
            (_) {},
          ),
        },
        child: Semantics(
          button: true,
          enabled: enabled,
          label: _locked
              ? 'Transmitiendo con el turno bloqueado. Toca para dejar de hablar'
              : pressed
                  ? 'Transmitiendo. Suelta para dejar de hablar, o desliza hacia arriba para bloquear el turno'
                  : 'Mantén pulsado para hablar',
          hint: widget.peerSpeaking ? 'El otro participante está transmitiendo' : null,
          // El icono, el anillo y el texto "PULSA PARA HABLAR" son decoración de
          // un control que ya se anuncia completo arriba: sin esto, el lector
          // repite el texto tres veces.
          excludeSemantics: true,
          child: SizedBox(
            width: NHitSize.pttButton,
            height: NHitSize.pttButton,
            child: CustomPaint(
              painter: _PttRingPainter(
                level: level,
                color: signalColor,
                trackColor: scheme.outlineVariant,
                glow: pressed,
              ),
              child: Center(
                child: Padding(
                  padding: const EdgeInsets.all(NSpace.xl),
                  child: Column(
                    mainAxisSize: MainAxisSize.min,
                    children: [
                      Icon(
                        _locked
                            ? Icons.lock
                            : widget.peerSpeaking && !pressed
                                ? Icons.hearing
                                : Icons.mic,
                        color: signalColor,
                        size: 44,
                      ),
                      const SizedBox(height: NSpace.sm),
                      Text(
                        _locked
                            ? 'BLOQUEADO\nTOCA PARA SOLTAR'
                            : pressed
                                ? 'HABLANDO\n↑ PARA BLOQUEAR'
                                : 'PULSA PARA HABLAR',
                        textAlign: TextAlign.center,
                        style: TextStyle(
                          color: enabled ? signalColor : scheme.outline,
                          fontWeight: FontWeight.w800,
                          fontSize: 12,
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
      ),
    );

    return AnimatedScale(
      scale: pressed ? 1.04 : 1.0,
      duration: NDuration.short,
      curve: Curves.easeOut,
      child: Card(
        color: scheme.surfaceContainerLow,
        shape: NShape.rounded(NShape.full),
        elevation: pressed ? 2 : 0,
        child: disc,
      ),
    );
  }
}

/// Pinta el anillo de señal del botón PTT.
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

  static const _stroke = 10.0;

  @override
  void paint(Canvas canvas, Size size) {
    final center = Offset(size.width / 2, size.height / 2);
    final radius = (size.shortestSide - _stroke) / 2;

    final track = Paint()
      ..color = trackColor
      ..style = PaintingStyle.stroke
      ..strokeWidth = _stroke
      ..strokeCap = StrokeCap.round;
    canvas.drawCircle(center, radius, track);

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
