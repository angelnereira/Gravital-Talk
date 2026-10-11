import 'package:flutter/material.dart';
import 'package:flutter_animate/flutter_animate.dart';
import 'package:provider/provider.dart';

import '../core/glass.dart';
import '../core/routes.dart';
import '../core/states.dart';
import '../core/tokens.dart';
import '../models/session.dart';
import '../services/session_controller.dart';
import '../widgets/active_session_card.dart';
import '../widgets/common.dart';
import '../widgets/contacts_section.dart';
import '../services/contacts.dart';
import 'about_screen.dart';
import 'invite_screen.dart';
import 'pairing_screen.dart';
import 'settings_screen.dart';

/// Pantalla de inicio: dos acciones y nada que configurar.
///
/// Antes la home mostraba el perfil del servidor, el peer P2P, el diagnóstico
/// de red y el registro de eventos al mismo nivel que "conectar". Todo eso
/// sigue existiendo, pero plegado en "Detalles técnicos": es información para
/// quien depura, no un paso para quien quiere hablar.
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

  /// Con contactos disponibles, invitar es un enlace que lo hace todo. Sin
  /// motor nativo (modo demostración) queda la sala de siempre.
  void _invite() {
    final contacts = context.read<ContactsService>();
    if (!contacts.available) return _open(PairingMode.host);
    Navigator.push(
      context,
      ForwardRoute(builder: (_) => const InviteScreen()),
    );
  }

  void _open(PairingMode mode) {
    Navigator.push(
      context,
      ForwardRoute(builder: (_) => PairingScreen(mode: mode)),
    );
  }

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    // Con una sesión viva lo único que tiene sentido es volver a ella.
    final canPair = !c.isLive && !c.busy;

    return Scaffold(
      appBar: AppBar(
        title: const Text('Gravital Talk'),
        actions: [
          IconButton(
            tooltip: 'Ajustes',
            onPressed: () => Navigator.push(context,
                QuickRoute(builder: (_) => const SettingsScreen())),
            icon: const Icon(Icons.settings_outlined),
          ),
          IconButton(
            tooltip: 'Acerca de',
            onPressed: () => Navigator.push(
                context, QuickRoute(builder: (_) => const AboutScreen())),
            icon: const Icon(Icons.info_outline),
          ),
        ],
      ),
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 520),
          child: ListView(
            padding: const EdgeInsets.fromLTRB(
                Spacing.xl, Spacing.md, Spacing.xl, Spacing.xxl),
            children: [
              Padding(
                padding: const EdgeInsets.symmetric(vertical: Spacing.xl),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      'Pulsa y habla',
                      style: theme.textTheme.displaySmall?.copyWith(
                        fontWeight: FontWeight.w800,
                        color: glass.primary,
                        height: 1.05,
                      ),
                    ),
                    const SizedBox(height: Spacing.sm),
                    Text(
                      'Voz directa entre dos dispositivos, cifrada de extremo '
                      'a extremo. Sin cuentas ni servidores.',
                      style: theme.textTheme.bodyLarge
                          ?.copyWith(color: glass.secondary),
                    ),
                  ],
                ),
              ).animate().fadeIn(duration: 350.ms).slideY(begin: 0.06, end: 0),
              if (c.engineKind == EngineKind.demo) ...[
                const WarningBanner(
                  message: 'Modo demostración: falta el motor de audio, así '
                      'que nada de lo que veas es una llamada real.',
                ),
                const SizedBox(height: Spacing.lg),
              ],
              if (c.isLive) ...[
                const ActiveSessionCard(),
                const SizedBox(height: Spacing.lg),
              ],
              _ActionTile(
                icon: Icons.qr_code_2,
                title: 'Invitar',
                subtitle: 'Envía un enlace o enseña un código',
                primary: true,
                onTap: canPair ? _invite : null,
              )
                  .animate(delay: 100.ms)
                  .fadeIn(duration: 300.ms)
                  .slideY(begin: 0.08, end: 0),
              const SizedBox(height: Spacing.md),
              _ActionTile(
                icon: Icons.qr_code_scanner,
                title: 'Escanear',
                subtitle: 'Lee el código de otra pantalla o pega un enlace',
                onTap: canPair ? () => _open(PairingMode.join) : null,
              )
                  .animate(delay: 180.ms)
                  .fadeIn(duration: 300.ms)
                  .slideY(begin: 0.08, end: 0),
              const SizedBox(height: Spacing.xxl),
              ContactsSection(enabled: canPair),
              const SizedBox(height: Spacing.xxl),
              _TechnicalDetails(
                controller: c,
                localIps: _localIps,
                publicIp: _publicIp,
                diagnosing: _diagnosing,
                onDiagnose: () => _diagnose(c),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Acción principal de la home: una tarjeta de vidrio entera pulsable.
class _ActionTile extends StatelessWidget {
  const _ActionTile({
    required this.icon,
    required this.title,
    required this.subtitle,
    required this.onTap,
    this.primary = false,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final VoidCallback? onTap;

  /// La acción destacada lleva el resplandor de marca.
  final bool primary;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    final scheme = theme.colorScheme;
    final enabled = onTap != null;

    return Semantics(
      button: true,
      enabled: enabled,
      label: '$title. $subtitle',
      child: Opacity(
        opacity: enabled ? 1 : 0.45,
        child: GlassSurface(
          glow: primary ? scheme.primary.withValues(alpha: 0.3) : null,
          padding: EdgeInsets.zero,
          child: Material(
            type: MaterialType.transparency,
            child: InkWell(
              onTap: onTap,
              borderRadius: BorderRadius.circular(Radii.glass),
              child: Padding(
                padding: const EdgeInsets.all(Spacing.xl),
                child: Row(
                  children: [
                    Container(
                      width: 56,
                      height: 56,
                      decoration: BoxDecoration(
                        shape: BoxShape.circle,
                        color: primary
                            ? scheme.primary
                            : scheme.primary.withValues(alpha: 0.16),
                      ),
                      child: Icon(
                        icon,
                        size: 28,
                        // Oscuro fijo: sobre el disco de marca, `onPrimary` del tema no
                        // llega a contraste suficiente.
                        color: primary ? const Color(0xFF04201C) : scheme.primary,
                      ),
                    ),
                    const SizedBox(width: Spacing.lg),
                    Expanded(
                      child: ExcludeSemantics(
                        child: Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            Text(
                              title,
                              style: theme.textTheme.titleLarge?.copyWith(
                                fontWeight: FontWeight.w800,
                                color: glass.primary,
                              ),
                            ),
                            const SizedBox(height: Spacing.xxs),
                            Text(
                              subtitle,
                              style: theme.textTheme.bodyMedium
                                  ?.copyWith(color: glass.secondary),
                            ),
                          ],
                        ),
                      ),
                    ),
                    Icon(Icons.arrow_forward, color: glass.secondary),
                  ],
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

/// Información de depuración, plegada por defecto.
class _TechnicalDetails extends StatelessWidget {
  const _TechnicalDetails({
    required this.controller,
    required this.localIps,
    required this.publicIp,
    required this.diagnosing,
    required this.onDiagnose,
  });

  final SessionController controller;
  final List<String> localIps;
  final String? publicIp;
  final bool diagnosing;
  final VoidCallback onDiagnose;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    final c = controller;

    return GlassSurface(
      padding: EdgeInsets.zero,
      child: Theme(
        // Sin las líneas divisorias que `ExpansionTile` pinta al abrirse:
        // dentro de una tarjeta de vidrio son ruido.
        data: theme.copyWith(dividerColor: Colors.transparent),
        child: ExpansionTile(
          shape: const Border(),
          collapsedShape: const Border(),
          iconColor: glass.secondary,
          collapsedIconColor: glass.secondary,
          trailing: const Icon(Icons.keyboard_arrow_down_rounded),
          tilePadding: const EdgeInsets.symmetric(
              horizontal: Spacing.xl, vertical: Spacing.xs),
          childrenPadding: const EdgeInsets.fromLTRB(
              Spacing.xl, 0, Spacing.xl, Spacing.xl),
          expandedCrossAxisAlignment: CrossAxisAlignment.start,
          title: Text(
            'Detalles técnicos',
            style: theme.textTheme.titleSmall?.copyWith(color: glass.primary),
          ),
          subtitle: Text(
            'Red, motor y registro de eventos',
            style: theme.textTheme.bodySmall?.copyWith(color: glass.secondary),
          ),
          children: [
            _InfoRow(
              icon: c.engineKind == EngineKind.native
                  ? Icons.memory
                  : Icons.science_outlined,
              label: 'Motor',
              value: '${c.engineKind.label} · ${c.engineDetail}',
            ),
            _InfoRow(
              icon: Icons.wifi,
              label: 'IP local',
              value: localIps.isEmpty ? '—' : localIps.join(', '),
            ),
            _InfoRow(
              icon: Icons.public,
              label: 'IP pública',
              value: publicIp ?? '—',
            ),
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton.icon(
                onPressed: diagnosing ? null : onDiagnose,
                icon: diagnosing
                    ? const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      )
                    : const Icon(Icons.refresh, size: 18),
                label: const Text('Comprobar la red'),
              ),
            ),
            const SizedBox(height: Spacing.md),
            Row(
              children: [
                Expanded(
                  child: Text(
                    'Eventos',
                    style: theme.textTheme.titleSmall
                        ?.copyWith(color: glass.primary),
                  ),
                ),
                IconButton(
                  tooltip: 'Limpiar eventos',
                  onPressed: c.log.clear,
                  icon: const Icon(Icons.delete_sweep_outlined),
                ),
              ],
            ),
            EventLogView(log: c.log, max: 8),
          ],
        ),
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
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: Spacing.sm),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, size: 16, color: glass.secondary),
          const SizedBox(width: Spacing.sm),
          SizedBox(
            width: 84,
            child: Text(label,
                style: theme.textTheme.bodySmall
                    ?.copyWith(color: glass.secondary)),
          ),
          Expanded(
            child: Text(value,
                style: theme.textTheme.bodyMedium?.copyWith(
                    fontWeight: FontWeight.w600, color: glass.primary)),
          ),
        ],
      ),
    );
  }
}
