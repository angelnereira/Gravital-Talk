import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:url_launcher/url_launcher.dart';

import '../core/constants.dart';
import '../services/session_controller.dart';
import '../widgets/common.dart';

/// Información del proyecto y del stack.
class AboutScreen extends StatelessWidget {
  const AboutScreen({super.key});

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();
    return Scaffold(
      appBar: AppBar(title: const Text('Acerca de')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          Card(
            child: Padding(
              padding: const EdgeInsets.all(20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Icon(Icons.graphic_eq,
                          size: 34,
                          color: Theme.of(context).colorScheme.primary),
                      const SizedBox(width: 12),
                      Text('Gravital Talk',
                          style: Theme.of(context)
                              .textTheme
                              .headlineSmall
                              ?.copyWith(fontWeight: FontWeight.w800)),
                    ],
                  ),
                  const SizedBox(height: 10),
                  Text(
                    'Protocolo de audio en tiempo real sobre internet: UDP P2P '
                    'directo o sala con relay central. Handshake X25519, cifrado '
                    'ChaCha20-Poly1305 por paquete, FEC XOR y jitter buffer '
                    'explícito.',
                    style: Theme.of(context).textTheme.bodyMedium,
                  ),
                ],
              ),
            ),
          ),
          const SizedBox(height: 16),
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
          const SizedBox(height: 16),
          SectionCard(
            title: 'Modos de conexión',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _mode(context, Icons.dns_outlined, 'Servidor (sala)',
                    'Un terminal central actúa como relay: enruta el handshake y '
                    'el audio cifrado entre los peers de la misma sala.'),
                const SizedBox(height: 10),
                _mode(context, Icons.device_hub_outlined, 'P2P directo',
                    'Dos dispositivos negocian claves y se hablan sin '
                    'intermediarios. Requiere conectividad UDP mutua.'),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCard(
            title: 'Enlaces',
            child: Column(
              children: [
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  leading: const Icon(Icons.code),
                  title: const Text('Repositorio'),
                  subtitle: const Text('github.com/angelnereira/gravital-talk'),
                  onTap: () => _open(
                      context, 'https://github.com/angelnereira/gravital-talk'),
                ),
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  leading: const Icon(Icons.description_outlined),
                  title: const Text('Especificación del protocolo'),
                  subtitle: const Text('docs/protocol-spec.md'),
                  onTap: () => _open(
                      context,
                      'https://github.com/angelnereira/gravital-talk/blob/main/docs/protocol-spec.md'),
                ),
                ListTile(
                  contentPadding: EdgeInsets.zero,
                  leading: const Icon(Icons.balance),
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
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Row(
        children: [
          SizedBox(
            width: 130,
            child: Text(label,
                style: Theme.of(context).textTheme.bodySmall?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant)),
          ),
          Expanded(
            child: SelectableText(value,
                style: const TextStyle(fontWeight: FontWeight.w600)),
          ),
        ],
      ),
    );
  }

  Widget _mode(BuildContext context, IconData icon, String title, String body) {
    return Row(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Icon(icon, size: 18, color: Theme.of(context).colorScheme.primary),
        const SizedBox(width: 10),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(title, style: const TextStyle(fontWeight: FontWeight.w700)),
              Text(body, style: Theme.of(context).textTheme.bodySmall),
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
