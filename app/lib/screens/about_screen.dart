import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

import '../core/constants.dart';
import '../services/session_controller.dart';
import '../ui/components/index.dart';
import '../ui/semantic_colors.dart';
import '../ui/tokens.dart';

/// Información del proyecto y del stack.
class AboutScreen extends StatelessWidget {
  const AboutScreen({super.key});

  /// Ancho de la columna de etiquetas de la tabla de versiones: los valores
  /// son más largos y sin columna el bloque se lee como texto suelto.
  static const double _labelWidth = 130;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    final c = context.watch<SessionController>();
    // Los modos de conexión son estados, no adornos: cada icono lleva el tono
    // que significa. Los enlaces, en cambio, son iconos auxiliares y van en
    // neutro, porque si todos tuvieran color no significaría nada ninguno.
    final semantic = SemanticColors.of(context);

    return Scaffold(
      appBar: AppBar(title: const Text('Acerca de')),
      body: ListView(
        padding: const EdgeInsets.all(NSpace.lg),
        children: [
          SectionCard(
            title: 'Gravital Talk',
            trailing: Icon(Icons.graphic_eq, size: 28, color: scheme.primary),
            child: Text(
              'Protocolo de audio en tiempo real sobre internet: UDP P2P '
              'directo o sala con relay central. Handshake X25519, cifrado '
              'ChaCha20-Poly1305 por paquete, FEC XOR y jitter buffer '
              'explícito.',
              style: theme.textTheme.bodyMedium
                  ?.copyWith(color: scheme.onSurfaceVariant),
            ),
          ),
          const SizedBox(height: NSpace.lg),
          SectionCard(
            title: 'Versiones',
            child: Column(
              children: [
                _row(context, 'App', appVersion),
                _row(context, 'Motor', c.engineKind.label),
                _row(context, 'Detalle motor', c.engineDetail),
                _row(context, 'ABI objetivo', 'v$supportedAbiVersion'),
                _row(context, 'Protocolo wire', 'v1'),
              ],
            ),
          ),
          const SizedBox(height: NSpace.lg),
          SectionCard(
            title: 'Modos de conexión',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _mode(
                  context,
                  Icons.dns_outlined,
                  'Servidor (sala)',
                  'Un terminal central actúa como relay: enruta el handshake y '
                  'el audio cifrado entre los peers de la misma sala.',
                  semantic.info,
                ),
                const SizedBox(height: NSpace.md),
                _mode(
                  context,
                  Icons.device_hub_outlined,
                  'P2P directo',
                  'Dos dispositivos negocian claves y se hablan sin '
                  'intermediarios. Requiere conectividad UDP mutua.',
                  semantic.success,
                ),
              ],
            ),
          ),
          const SizedBox(height: NSpace.lg),
          SectionCard(
            title: 'Enlaces',
            child: Column(
              children: [
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  leading:
                      Icon(Icons.code, color: scheme.onSurfaceVariant),
                  title: const Text('Repositorio'),
                  subtitle: const Text('github.com/angelnereira/gravital-talk'),
                  onTap: () => _open(
                      context, 'https://github.com/angelnereira/gravital-talk'),
                ),
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  leading: Icon(Icons.description_outlined,
                      color: scheme.onSurfaceVariant),
                  title: const Text('Especificación del protocolo'),
                  subtitle: const Text('docs/protocol-spec.md'),
                  onTap: () => _open(
                      context,
                      'https://github.com/angelnereira/gravital-talk/blob/main/docs/protocol-spec.md'),
                ),
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  leading:
                      Icon(Icons.balance, color: scheme.onSurfaceVariant),
                  title: const Text('Licencia'),
                  subtitle: const Text('MIT OR Apache-2.0'),
                  onTap: () => _snack(context, 'MIT OR Apache-2.0'),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }

  Widget _row(BuildContext context, String label, String value) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: NSpace.sm),
      child: Row(
        children: [
          SizedBox(
            width: _labelWidth,
            child: Text(label,
                style: theme.textTheme.bodySmall
                    ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
          ),
          Expanded(
            child: SelectableText(value,
                style: theme.textTheme.bodyMedium
                    ?.copyWith(fontWeight: FontWeight.w600)),
          ),
        ],
      ),
    );
  }

  Widget _mode(BuildContext context, IconData icon, String title, String body,
      SemanticTone tone) {
    final theme = Theme.of(context);
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(icon, size: 20, color: tone.color),
        const SizedBox(width: NSpace.sm),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(title,
                  style: theme.textTheme.titleSmall
                      ?.copyWith(fontWeight: FontWeight.w700)),
              Text(body,
                  style: theme.textTheme.bodySmall
                      ?.copyWith(color: theme.colorScheme.onSurfaceVariant)),
            ],
          ),
        ),
      ],
    );
  }

  Future<void> _open(BuildContext context, String url) async {
    final uri = Uri.parse(url);
    final ok = await launchUrl(uri, mode: LaunchMode.externalApplication);
    if (!ok && context.mounted) {
      _snack(context, url);
    }
  }

  void _snack(BuildContext context, String msg) {
    ScaffoldMessenger.of(context)
        .showSnackBar(SnackBar(content: Text(msg)));
  }
}