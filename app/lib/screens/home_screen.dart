import 'package:flutter/material.dart';
import 'package:flutter_animate/flutter_animate.dart';
import 'package:provider/provider.dart';

import '../models/connection.dart';
import '../models/session.dart';
import '../services/session_controller.dart';
import '../widgets/common.dart';
import 'about_screen.dart';
import 'p2p_setup_screen.dart';
import 'server_setup_screen.dart';
import 'session_screen.dart';
import 'settings_screen.dart';

class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

class _HomeScreenState extends State<HomeScreen> {
  String? _publicIp;
  List<String> _localIps = const [];
  bool _diagnosing = false;

  Future<void> _diagnose(SessionController c) async {
    setState(() => _diagnosing = true);
    final ips = await c.localAddresses();
    final pub = await c.discoverPublicAddress();
    if (!mounted) return;
    setState(() {
      _localIps = ips;
      _publicIp = pub;
      _diagnosing = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();
    final scheme = Theme.of(context).colorScheme;

    return Scaffold(
      appBar: AppBar(
        title: const Text('Gravital Talk'),
        actions: [
          IconButton(
            tooltip: 'Ajustes',
            onPressed: () => Navigator.push(context,
                MaterialPageRoute(builder: (_) => const SettingsScreen())),
            icon: const Icon(Icons.settings_outlined),
          ),
          IconButton(
            tooltip: 'Acerca de',
            onPressed: () => Navigator.push(context,
                MaterialPageRoute(builder: (_) => const AboutScreen())),
            icon: const Icon(Icons.info_outline),
          ),
        ],
      ),
      body: ListView(
        padding: const EdgeInsets.fromLTRB(16, 8, 16, 24),
        children: [
          _Hero(controller: c)
              .animate()
              .fadeIn(duration: 350.ms)
              .slideY(begin: 0.08, end: 0),
          const SizedBox(height: 16),
          if (c.isLive) ...[
            FilledButton.icon(
              onPressed: () => Navigator.push(context,
                  MaterialPageRoute(builder: (_) => const SessionScreen())),
              icon: const Icon(Icons.mic),
              label: Text(
                  'Ir a la sesión activa · ${c.roomCode ?? (c.mode == ConnectionMode.p2p ? 'P2P' : '')}'),
            ),
            const SizedBox(height: 16),
          ],
          ModeCard(
            icon: Icons.dns_outlined,
            color: scheme.primary,
            title: 'Servidor (sala)',
            subtitle:
                'Terminal central: crea o únete a una sala vía relay con código',
            onTap: () => Navigator.push(context,
                MaterialPageRoute(builder: (_) => const ServerSetupScreen())),
          )
              .animate(delay: 120.ms)
              .fadeIn(duration: 300.ms)
              .slideX(begin: -0.05, end: 0),
          const SizedBox(height: 10),
          ModeCard(
            icon: Icons.device_hub_outlined,
            color: const Color(0xFF00A97F),
            title: 'P2P directo',
            subtitle: 'Conexión directa entre dos dispositivos, sin relay',
            onTap: () => Navigator.push(context,
                MaterialPageRoute(builder: (_) => const P2pSetupScreen())),
          )
              .animate(delay: 220.ms)
              .fadeIn(duration: 300.ms)
              .slideX(begin: -0.05, end: 0),
          const SizedBox(height: 16),
          SectionCard(
            title: 'Última conexión',
            subtitle: 'Perfiles guardados en este dispositivo',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _InfoRow(
                  icon: Icons.dns_outlined,
                  label: 'Servidor',
                  value: c.server.host.isEmpty
                      ? 'sin configurar'
                      : '${c.server.host}:${c.server.udpPort}',
                ),
                _InfoRow(
                  icon: Icons.meeting_room_outlined,
                  label: 'Sala',
                  value: c.server.roomCode.isEmpty
                      ? '—'
                      : c.server.roomCode,
                ),
                _InfoRow(
                  icon: Icons.device_hub_outlined,
                  label: 'Peer P2P',
                  value: c.p2p.host.isEmpty
                      ? 'sin configurar'
                      : '${c.p2p.host}:${c.p2p.port}',
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCard(
            title: 'Diagnóstico de red',
            subtitle: 'Direcciones locales y pública (STUN)',
            trailing: IconButton(
              onPressed: _diagnosing ? null : () => _diagnose(c),
              icon: _diagnosing
                  ? const SizedBox(
                      width: 18,
                      height: 18,
                      child: CircularProgressIndicator(strokeWidth: 2))
                  : const Icon(Icons.refresh),
            ),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _InfoRow(
                  icon: Icons.wifi,
                  label: 'IP local',
                  value: _localIps.isEmpty ? 'pulsa ↻ para detectar' : _localIps.join(', '),
                ),
                _InfoRow(
                  icon: Icons.public,
                  label: 'IP pública',
                  value: _publicIp ?? '—',
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCard(
            title: 'Eventos',
            subtitle: 'Actividad reciente',
            trailing: IconButton(
              tooltip: 'Limpiar',
              onPressed: c.log.clear,
              icon: const Icon(Icons.delete_sweep_outlined),
            ),
            child: EventLogView(log: c.log, max: 8),
          ),
          const SizedBox(height: 12),
          Center(
            child: Text(
              'RFC-style · wire v1 · X25519 + ChaCha20-Poly1305',
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                  color: Theme.of(context).colorScheme.onSurfaceVariant),
            ),
          ),
        ],
      ),
    );
  }
}

class _Hero extends StatelessWidget {
  const _Hero({required this.controller});

  final SessionController controller;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.all(18),
      decoration: BoxDecoration(
        borderRadius: BorderRadius.circular(22),
        gradient: LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [
            scheme.primary.withValues(alpha: 0.16),
            scheme.primary.withValues(alpha: 0.04),
          ],
        ),
        border: Border.all(color: scheme.primary.withValues(alpha: 0.25)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Icon(Icons.graphic_eq, color: scheme.primary, size: 30),
              const SizedBox(width: 10),
              Expanded(
                child: Text('Audio en tiempo real',
                    style: Theme.of(context)
                        .textTheme
                        .titleLarge
                        ?.copyWith(fontWeight: FontWeight.w800)),
              ),
              StatusBadge(controller.state),
            ],
          ),
          const SizedBox(height: 12),
          Row(
            children: [
              Icon(
                controller.engineKind == EngineKind.native
                    ? Icons.memory
                    : Icons.science_outlined,
                size: 15,
                color: scheme.onSurfaceVariant,
              ),
              const SizedBox(width: 6),
              Expanded(
                child: Text(
                  'Motor: ${controller.engineKind.label} · ${controller.engineDetail}',
                  style: Theme.of(context).textTheme.bodySmall?.copyWith(
                      color: scheme.onSurfaceVariant),
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }
}

class _InfoRow extends StatelessWidget {
  const _InfoRow({required this.icon, required this.label, required this.value});

  final IconData icon;
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.only(bottom: 8),
      child: Row(
        children: [
          Icon(icon, size: 16, color: scheme.onSurfaceVariant),
          const SizedBox(width: 8),
          SizedBox(
            width: 84,
            child: Text(label,
                style: Theme.of(context)
                    .textTheme
                    .bodySmall
                    ?.copyWith(color: scheme.onSurfaceVariant)),
          ),
          Expanded(
            child: Text(value,
                style: Theme.of(context)
                    .textTheme
                    .bodyMedium
                    ?.copyWith(fontWeight: FontWeight.w600)),
          ),
        ],
      ),
    );
  }
}
