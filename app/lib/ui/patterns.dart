/// Patrones de pantalla reutilizables (GEUI).
///
/// Estados de carga, vacío, error, confirmación y avisos: la misma forma en
/// toda la app.
library;

import 'package:flutter/material.dart';

import 'tokens.dart';

/// Estados de carga reutilizables.
abstract final class UILoading {
  const UILoading._();

  /// Indicador centrado con mensaje. Para cargas que ocupan la pantalla.
  static Widget centered({required String message, double size = 32}) {
    return Center(
      child: Column(
        mainAxisSize: MainAxisSize.min,
        children: [
          SizedBox(
            width: size,
            height: size,
            child: const CircularProgressIndicator(strokeWidth: 3),
          ),
          const SizedBox(height: NSpace.lg),
          Text(message, textAlign: TextAlign.center),
        ],
      ),
    );
  }

  /// Botón en estado de carga. Conserva el ancho para no mover el layout.
  static Widget button({required String label, double height = NHitSize.minTouch}) {
    return SizedBox(
      height: height,
      child: FilledButton(
        onPressed: null,
        child: Row(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            const SizedBox(
              width: 16,
              height: 16,
              child: CircularProgressIndicator(strokeWidth: 2),
            ),
            const SizedBox(width: NSpace.md),
            Text(label),
          ],
        ),
      ),
    );
  }
}

/// Estado vacío: qué significa y qué hacer a continuación.
class EmptyState extends StatelessWidget {
  const EmptyState({
    super.key,
    required this.icon,
    required this.title,
    this.message,
    this.actionLabel,
    this.onAction,
  });

  final IconData icon;
  final String title;
  final String? message;
  final String? actionLabel;
  final VoidCallback? onAction;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(NSpace.xxl),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Container(
              width: 72,
              height: 72,
              decoration: BoxDecoration(
                color: theme.colorScheme.primary.withValues(alpha: 0.1),
                shape: BoxShape.circle,
              ),
              child: Icon(icon, size: 32, color: theme.colorScheme.primary),
            ),
            const SizedBox(height: NSpace.lg),
            Text(
              title,
              textAlign: TextAlign.center,
              style: theme.textTheme.titleMedium,
            ),
            if (message != null) ...[
              const SizedBox(height: NSpace.sm),
              Text(
                message!,
                textAlign: TextAlign.center,
                style: theme.textTheme.bodySmall?.copyWith(
                  color: theme.colorScheme.onSurfaceVariant,
                ),
              ),
            ],
            if (actionLabel != null && onAction != null) ...[
              const SizedBox(height: NSpace.xl),
              FilledButton(onPressed: onAction, child: Text(actionLabel!)),
            ],
          ],
        ),
      ),
    );
  }
}

/// Banner de error con acción de reintento.
class ErrorBanner extends StatelessWidget {
  const ErrorBanner({
    super.key,
    required this.message,
    this.onRetry,
    this.onDismiss,
  });

  final String message;
  final VoidCallback? onRetry;
  final VoidCallback? onDismiss;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    return Container(
      margin: const EdgeInsets.only(bottom: NSpace.md),
      padding: const EdgeInsets.all(NSpace.md),
      decoration: BoxDecoration(
        color: scheme.errorContainer,
        borderRadius: BorderRadius.circular(NShape.medium),
        border: Border.all(color: scheme.error.withValues(alpha: 0.4)),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(Icons.error_outline, color: scheme.error, size: 20),
          const SizedBox(width: NSpace.md),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  message,
                  style: theme.textTheme.bodySmall?.copyWith(color: scheme.onErrorContainer),
                ),
                if (onRetry != null || onDismiss != null) ...[
                  const SizedBox(height: NSpace.sm),
                  Row(
                    children: [
                      if (onRetry != null)
                        TextButton(
                          onPressed: onRetry,
                          style: TextButton.styleFrom(
                            padding: const EdgeInsets.symmetric(horizontal: NSpace.md),
                            minimumSize: const Size(0, 32),
                          ),
                          child: const Text('Reintentar'),
                        ),
                      if (onDismiss != null)
                        TextButton(
                          onPressed: onDismiss,
                          style: TextButton.styleFrom(
                            padding: const EdgeInsets.symmetric(horizontal: NSpace.md),
                            minimumSize: const Size(0, 32),
                          ),
                          child: const Text('Descartar'),
                        ),
                    ],
                  ),
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// Banner de aviso no bloqueante.
class WarningBanner extends StatelessWidget {
  const WarningBanner({
    super.key,
    required this.message,
    this.actionLabel,
    this.onAction,
  });

  final String message;
  final String? actionLabel;
  final VoidCallback? onAction;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    return Container(
      margin: const EdgeInsets.only(bottom: NSpace.md),
      padding: const EdgeInsets.all(NSpace.md),
      decoration: BoxDecoration(
        color: scheme.tertiaryContainer,
        borderRadius: BorderRadius.circular(NShape.medium),
        border: Border.all(color: scheme.tertiary.withValues(alpha: 0.4)),
      ),
      child: Row(
        children: [
          Icon(Icons.warning_amber_rounded, color: scheme.tertiary, size: 20),
          const SizedBox(width: NSpace.md),
          Expanded(
            child: Text(
              message,
              style: theme.textTheme.bodyMedium?.copyWith(color: scheme.onTertiaryContainer),
            ),
          ),
          if (actionLabel != null && onAction != null)
            TextButton(
              onPressed: onAction,
              style: TextButton.styleFrom(foregroundColor: scheme.onTertiaryContainer),
              child: Text(actionLabel!),
            ),
        ],
      ),
    );
  }
}

/// Confirmación de acción destructiva.
class ConfirmDialog extends StatelessWidget {
  const ConfirmDialog({
    super.key,
    required this.title,
    required this.message,
    this.confirmLabel = 'Confirmar',
    this.cancelLabel = 'Cancelar',
    this.destructive = false,
  });

  final String title;
  final String message;
  final String confirmLabel;
  final String cancelLabel;
  final bool destructive;

  static Future<bool> show(
    BuildContext context, {
    required String title,
    required String message,
    String confirmLabel = 'Confirmar',
    String cancelLabel = 'Cancelar',
    bool destructive = false,
  }) async {
    final result = await showDialog<bool>(
      context: context,
      builder: (_) => ConfirmDialog(
        title: title,
        message: message,
        confirmLabel: confirmLabel,
        cancelLabel: cancelLabel,
        destructive: destructive,
      ),
    );
    return result ?? false;
  }

  @override
  Widget build(BuildContext context) {
    return AlertDialog(
      title: Text(title),
      content: Text(message),
      actions: [
        UIDialogActions(
          cancelLabel: cancelLabel,
          onCancel: () => Navigator.pop(context, false),
          confirmLabel: confirmLabel,
          onConfirm: () => Navigator.pop(context, true),
          destructive: destructive,
        ),
      ],
    );
  }
}

/// Los dos botones de un diálogo: cancelar y confirmar.
class UIDialogActions extends StatelessWidget {
  const UIDialogActions({
    super.key,
    required this.cancelLabel,
    required this.onCancel,
    required this.confirmLabel,
    required this.onConfirm,
    this.destructive = false,
  });

  final String cancelLabel;
  final VoidCallback onCancel;
  final String confirmLabel;
  final VoidCallback onConfirm;
  final bool destructive;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    const size = Size(0, NHitSize.minTouch);
    return Row(
      children: [
        Expanded(
          child: OutlinedButton(
            style: OutlinedButton.styleFrom(minimumSize: size),
            onPressed: onCancel,
            child: Text(cancelLabel, maxLines: 1, overflow: TextOverflow.ellipsis),
          ),
        ),
        const SizedBox(width: NSpace.md),
        Expanded(
          child: FilledButton(
            style: FilledButton.styleFrom(
              minimumSize: size,
              backgroundColor: destructive ? scheme.error : null,
              foregroundColor: destructive ? scheme.onError : null,
            ),
            onPressed: onConfirm,
            child: Text(confirmLabel, maxLines: 1, overflow: TextOverflow.ellipsis),
          ),
        ),
      ],
    );
  }
}
