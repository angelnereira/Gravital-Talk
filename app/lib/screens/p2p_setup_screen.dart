import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../core/constants.dart';
import '../core/states.dart';
import '../core/routes.dart';
import '../core/tokens.dart';
import '../models/connection.dart';
import '../services/session_controller.dart';
import '../services/validation.dart';
import '../widgets/common.dart';
import 'session_screen.dart';

/// Configuración del modo P2P directo (sin relay).
class P2pSetupScreen extends StatefulWidget {
  const P2pSetupScreen({super.key});

  @override
  State<P2pSetupScreen> createState() => _P2pSetupScreenState();
}

class _P2pSetupScreenState extends State<P2pSetupScreen> {
  late final TextEditingController _host;
  late final TextEditingController _port;
  late final TextEditingController _localPort;
  ConnectionRole _role = ConnectionRole.join;
  List<String> _localIps = const [];

  @override
  void initState() {
    super.initState();
    final p = context.read<SessionController>().p2p;
    _host = TextEditingController(text: p.host);
    _port = TextEditingController(text: '${p.port}');
    _localPort =
        TextEditingController(text: p.localPort == 0 ? '' : '${p.localPort}');
    _loadIps();
  }

  Future<void> _loadIps() async {
    final ips = await context.read<SessionController>().localAddresses();
    if (mounted) setState(() => _localIps = ips);
  }

  @override
  void dispose() {
    _host.dispose();
    _port.dispose();
    _localPort.dispose();
    super.dispose();
  }

  /// Error del campo del peer, mostrado en línea.
  String? _peerError;

  void _persist(SessionController c) {
    c.updateP2pProfile(P2pProfile(
      host: _host.text.trim(),
      port: int.tryParse(_port.text) ?? DefaultPorts.udp,
      localPort: int.tryParse(_localPort.text) ?? 0,
    ));
  }

  Future<void> _connect(SessionController c) async {
    // El peer sólo hace falta al unirse: al crear sala se espera a cualquiera.
    if (_role == ConnectionRole.join) {
      final peerError = validateHost(_host.text, fieldName: 'el peer');
      if (peerError != null) {
        setState(() => _peerError = peerError.message);
        c.reportError(peerError.message);
        return;
      }
    }

    _persist(c);
    final ok =
        _role == ConnectionRole.host ? await c.hostP2p() : await c.joinP2p();
    if (!mounted) return;
    if (ok && c.isLive) {
      Navigator.of(context).pushReplacement(
          ForwardRoute(builder: (_) => const SessionScreen()));
    }
  }

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();
    final hosting = _role == ConnectionRole.host;
    final scheme = Theme.of(context).colorScheme;

    return Scaffold(
      appBar: AppBar(title: const Text('P2P directo')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          SegmentedButton<ConnectionRole>(
            segments: const [
              ButtonSegment(
                value: ConnectionRole.host,
                icon: Icon(Icons.wifi_tethering),
                label: Text('Esperar peer'),
              ),
              ButtonSegment(
                value: ConnectionRole.join,
                icon: Icon(Icons.link),
                label: Text('Conectar'),
              ),
            ],
            selected: {_role},
            onSelectionChanged: (s) => setState(() => _role = s.first),
          ),
          const SizedBox(height: 16),
          SectionCard(
            title: hosting ? 'Tu dirección de escucha' : 'Peer destino',
            subtitle: hosting
                ? 'Acepta la primera conexión entrante (handshake abierto)'
                : 'Debe ser alcanzable por UDP desde este dispositivo',
            child: Column(
              children: [
                if (hosting) ...[
                  LabeledField(
                    label: 'Puerto local (vacío = efímero)',
                    controller: _localPort,
                    hint: '9000',
                    keyboardType: TextInputType.number,
                  ),
                  if (c.localPort != 0)
                    Container(
                      width: double.infinity,
                      padding: const EdgeInsets.all(12),
                      decoration: BoxDecoration(
                        color: scheme.primary.withValues(alpha: 0.08),
                        borderRadius: BorderRadius.circular(12),
                      ),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text('Comparte esta dirección con el peer:',
                              style: Theme.of(context).textTheme.bodySmall),
                          const SizedBox(height: 6),
                          for (final ip in _localIps.ifEmpty(['<tu-ip-local>']))
                            SelectableText('$ip:${c.localPort}',
                                style: const TextStyle(
                                    fontWeight: FontWeight.w700,
                                    fontSize: 16)),
                        ],
                      ),
                    ),
                ] else ...[
                  LabeledField(
                    label: 'IP o host del peer',
                    errorText: _peerError,
                    onChanged: (_) {
                      if (_peerError != null) {
                        setState(() => _peerError = null);
                      }
                    },
                    controller: _host,
                    hint: '192.168.1.42',
                    keyboardType: TextInputType.url,
                  ),
                  LabeledField(
                    label: 'Puerto UDP',
                    controller: _port,
                    hint: '9000',
                    keyboardType: TextInputType.number,
                  ),
                ],
              ],
            ),
          ),
          if (c.error != null) ...[
            const SizedBox(height: Spacing.md),
            ErrorBanner(
              message: c.error!,
              onRetry: c.busy ? null : () => _connect(c),
              onDismiss: () => c.clearError(),
            ),
          ],
          const SizedBox(height: 20),
          FilledButton.icon(
            onPressed: c.busy ? null : () => _connect(c),
            icon: c.busy
                ? const SizedBox(
                    width: 18,
                    height: 18,
                    child: CircularProgressIndicator(strokeWidth: 2))
                : Icon(hosting ? Icons.podcasts : Icons.link),
            label: Text(
              c.busy
                  ? 'Conectando…'
                  : hosting
                      ? 'Escuchar y esperar peer'
                      : 'Conectar al peer',
            ),
          ),
          const SizedBox(height: 8),
          Card(
            child: Padding(
              padding: const EdgeInsets.all(14),
              child: Row(
                children: [
                  Icon(Icons.shield_outlined, size: 18, color: scheme.primary),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Text(
                      'El P2P usa el mismo handshake X25519 + ChaCha20-Poly1305 '
                      'que la sala; sin relay no hay NAT traversal (usa la sala '
                      'si estás detrás de NAT simétrico).',
                      style: Theme.of(context).textTheme.bodySmall,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ],
      ),
    );
  }
}

extension _IfEmpty<T> on List<T> {
  List<T> ifEmpty(List<T> fallback) => isEmpty ? fallback : this;
}
