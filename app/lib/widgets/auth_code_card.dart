import 'dart:async';

import 'package:flutter/material.dart';

import '../core/tokens.dart';

/// Tarjeta del código de autorización rotativo.
///
/// ## Para qué existe
///
/// El token de sala de Gravital Talk es estático: sirve para siempre. Cuando la
/// sala está protegida, el usuario tiene que copiar y pegar un secreto largo
/// desde un chat, que a su vez queda en el historial para siempre.
///
/// Un código rotativo corto cambia eso: lo lees de la pantalla de quien crea la
/// sala y lo tecleas. Caduca solo, así que filtrarlo sólo da una ventana de 30 s.
///
/// Este widget asume que **el otro extremo muestra el código**; no genera TOTP
/// él mismo. La generación vive en el servicio Band-All
/// (https://github.com/angelnereira/Band-All), y la verificación en el relay
/// (`crates/gravital-talk-relay/src/band_all.rs`). Duplicar TOTP en Dart
/// significaría dos implementaciones que nadie compara entre sí.
class AuthCodeCard extends StatefulWidget {
  const AuthCodeCard({
    super.key,
    required this.code,
    this.onCodeChanged,
    this.stepSeconds = 30,
    this.showTimer = true,
  });

  /// Código actual de 6 dígitos.
  final String code;

  /// Se invoca cuando el usuario quiere pedir uno nuevo.
  final VoidCallback? onCodeChanged;

  /// Duración de cada código, en segundos.
  ///
  /// 30 es el paso de RFC 6238. Se parametriza para que un cambio en Band-All
  /// sea un cambio de configuración, no de código.
  final int stepSeconds;

  /// Muestra el tiempo restante del código.
  final bool showTimer;

  @override
  State<AuthCodeCard> createState() => _AuthCodeCardState();
}

class _AuthCodeCardState extends State<AuthCodeCard> {
  Timer? _timer;
  int _remaining = 0;

  @override
  void initState() {
    super.initState();
    _syncToStep();
    // Un tick por segundo basta: la barra no necesita más resolución y un
    // temporizador más rápido gasta batería sin que se note.
    _timer = Timer.periodic(const Duration(seconds: 1), (_) => _syncToStep());
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  void _syncToStep() {
    if (!mounted) return;
    final now = DateTime.now().millisecondsSinceEpoch ~/ 1000;
    final step = widget.stepSeconds;
    final left = step - (now % step);
    if (left != _remaining) {
      setState(() => _remaining = left);
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final fraction = _remaining / widget.stepSeconds;
    // Rojo cuando queda poco: es la señal de "date prisa" sin texto.
    final urgent = _remaining <= 5;

    return Container(
      padding: const EdgeInsets.all(Spacing.lg),
      decoration: BoxDecoration(
        color: theme.colorScheme.surfaceContainerHighest.withValues(alpha: 0.5),
        borderRadius: BorderRadius.circular(Radii.lg),
        border: Border.all(color: theme.colorScheme.outlineVariant),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(
                urgent ? Icons.timer_off_outlined : Icons.lock_clock,
                size: 18,
                color: urgent ? theme.colorScheme.error : theme.colorScheme.primary,
              ),
              const SizedBox(width: Spacing.sm),
              Expanded(
                child: Text(
                  'Código de autorización',
                  style: theme.textTheme.titleSmall,
                ),
              ),
              if (widget.showTimer)
                Text(
                  '${_remaining}s',
                  style: theme.textTheme.labelMedium?.copyWith(
                    color: urgent
                        ? theme.colorScheme.error
                        : theme.colorScheme.onSurfaceVariant,
                    fontFeatures: const [FontFeature.tabularFigures()],
                  ),
                ),
            ],
          ),
          const SizedBox(height: Spacing.md),
          // El código se muestra grande y monoespaciado: se lee y se teclea.
          SelectableText(
            _formatCode(widget.code),
            style: theme.textTheme.headlineMedium?.copyWith(
              fontWeight: FontWeight.w800,
              letterSpacing: 6,
              fontFeatures: const [FontFeature.tabularFigures()],
            ),
          ),
          const SizedBox(height: Spacing.md),
          ClipRRect(
            borderRadius: BorderRadius.circular(Radii.pill),
            child: LinearProgressIndicator(
              value: fraction.clamp(0.0, 1.0),
              minHeight: 4,
              backgroundColor: theme.colorScheme.surfaceContainerHighest,
              valueColor: AlwaysStoppedAnimation<Color>(
                urgent ? theme.colorScheme.error : theme.colorScheme.primary,
              ),
            ),
          ),
          if (widget.onCodeChanged != null) ...[
            const SizedBox(height: Spacing.sm),
            Align(
              alignment: Alignment.centerRight,
              child: TextButton.icon(
                onPressed: widget.onCodeChanged,
                icon: const Icon(Icons.refresh, size: 16),
                label: const Text('Pedir otro'),
              ),
            ),
          ],
        ],
      ),
    );
  }

  /// Agrupa el código en dos bloques: `123 456` se lee mejor que `123456`.
  String _formatCode(String code) {
    final clean = code.replaceAll(RegExp(r'[^0-9]'), '');
    if (clean.length <= 3) return clean;
    final half = clean.length ~/ 2;
    return '${clean.substring(0, half)} ${clean.substring(half)}';
  }
}
