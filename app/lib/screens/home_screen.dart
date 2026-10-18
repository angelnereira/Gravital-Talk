import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';

import '../core/routes.dart';
import '../models/session.dart';
import '../services/session_controller.dart';
import '../widgets/contacts_section.dart';
import '../services/contacts.dart';
import '../services/launch_links.dart';
import '../ui/components/index.dart';
import '../ui/patterns.dart';
import '../ui/semantic_colors.dart';
import '../ui/tokens.dart';
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
    Navigator.push(context, ForwardRoute(builder: (_) => const InviteScreen()));
  }

  /// Lee el portapapeles sólo cuando se pide: en Android leerlo por sistema
  /// enseña un aviso cada vez, y no hay por qué mirar lo que el usuario copia.
  Future<void> _pasteInvite() async {
    final messenger = ScaffoldMessenger.of(context);
    final navigator = Navigator.of(context);
    final text = (await Clipboard.getData(Clipboard.kTextPlain))?.text ?? '';
    final link = LaunchLinks.extract(text);
    if (link == null) {
      messenger.showSnackBar(
        const SnackBar(
          content: Text('No hay ninguna invitación de Gravital Talk copiada.'),
        ),
      );
      return;
    }
    navigator.push(
      ForwardRoute(
        builder: (_) => PairingScreen(mode: PairingMode.join, invite: link),
      ),
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
    final scheme = theme.colorScheme;
    final semanticColors = SemanticColors.of(context);
    final windowClass = WindowClass.of(context);
    // Con una sesión viva lo único que tiene sentido es volver a ella.
    final canPair = !c.isLive && !c.busy;

    return Scaffold(
      appBar: AppBar(
        title: const Text('Gravital Talk'),
        actions: [
          // El `tooltip` se ve al pasar el ratón o al mantener pulsado, pero no
          // llega al árbol de semántica: el botón se anunciaría sin nombre.
          // Por eso la etiqueta va además en `semanticLabel` (GEUI §9.1).
          IconButton(
            tooltip: 'Ajustes',
            onPressed: () => Navigator.push(
              context,
              QuickRoute(builder: (_) => const SettingsScreen()),
            ),
            icon: const Icon(Icons.settings_outlined, semanticLabel: 'Ajustes'),
          ),
          IconButton(
            tooltip: 'Acerca de',
            onPressed: () => Navigator.push(
              context,
              QuickRoute(builder: (_) => const AboutScreen()),
            ),
            icon: const Icon(Icons.info_outline, semanticLabel: 'Acerca de'),
          ),
        ],
      ),
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 520),
          child: ListView(
            // Margen lateral adaptativo (GEUI §8.1): 16 dp en compact, 24 dp
            // desde medium. Con 24 dp fijos las dos acciones principales quedaban
            // fuera de un viewport de 600 dp de alto.
            padding: EdgeInsets.fromLTRB(
              windowClass.isCompact ? NSpace.lg : NSpace.xl,
              NSpace.md,
              windowClass.isCompact ? NSpace.lg : NSpace.xl,
              NSpace.xxl,
            ),
            children: [
              Padding(
                padding: const EdgeInsets.symmetric(vertical: NSpace.lg),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Container(
                          width: 10,
                          height: 10,
                          decoration: BoxDecoration(
                            shape: BoxShape.circle,
                            color: semanticColors.success.color,
                          ),
                        ),
                        const SizedBox(width: NSpace.sm),
                        Expanded(
                          child: Text(
                            'Pulsa y habla',
                            style: theme.textTheme.displaySmall?.copyWith(
                              fontWeight: FontWeight.w800,
                              color: scheme.onSurface,
                              height: 1.05,
                            ),
                          ),
                        ),
                      ],
                    ),
                    const SizedBox(height: NSpace.sm),
                    Text(
                      'Voz directa entre dos dispositivos, cifrada de extremo '
                      'a extremo. Sin cuentas ni servidores.',
                      style: theme.textTheme.bodyLarge?.copyWith(
                        color: scheme.onSurfaceVariant,
                      ),
                    ),
                  ],
                ),
              ),
              if (c.engineKind == EngineKind.demo) ...[
                const WarningBanner(
                  message:
                      'Modo demostración: falta el motor de audio, así '
                      'que nada de lo que veas es una llamada real.',
                ),
                const SizedBox(height: NSpace.lg),
              ],
              if (c.isLive) ...[
                const ActiveSessionCard(),
                const SizedBox(height: NSpace.lg),
              ],
              _ActionTile(
                icon: Icons.qr_code_2,
                title: 'Invitar',
                subtitle: 'Envía un enlace o enseña un código',
                primary: true,
                onTap: canPair ? _invite : null,
              ),
              const SizedBox(height: NSpace.md),
              _ActionTile(
                icon: Icons.qr_code_scanner,
                title: 'Escanear',
                subtitle: 'Lee el código de otra pantalla o pega un enlace',
                onTap: canPair ? () => _open(PairingMode.join) : null,
              ),
              const SizedBox(height: NSpace.xxl),
              ContactsSection(
                enabled: canPair,
                onPaste: canPair ? _pasteInvite : null,
              ),
              const SizedBox(height: NSpace.xxl),
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

/// Acción principal de la home: una tarjeta entera pulsable.
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
    final scheme = theme.colorScheme;
    final enabled = onTap != null;

    // Un solo nodo semántico para toda la tarjeta.
    //
    // Con `label` y `onTap` aquí, y `excludeSemantics` sobre el contenido, el
    // botón se anuncia como una unidad. Lo contrario —la etiqueta en un
    // `Semantics` y el `tap` en el `InkWell`— deja dos nodos hermanos: el lector
    // de pantalla dice «botón» sin decir de qué, que es justo lo que
    // `labeledTapTargetGuideline` rechaza.
    return Semantics(
      button: true,
      enabled: enabled,
      label: '$title. $subtitle',
      excludeSemantics: true,
      onTap: enabled ? onTap : null,
      child: Opacity(
        opacity: enabled ? 1 : 0.45,
        child: Card(
          color: scheme.surfaceContainerLow,
          shape: NShape.rounded(NShape.medium),
          margin: EdgeInsets.zero,
          child: InkWell(
            onTap: onTap,
            borderRadius: BorderRadius.circular(NShape.medium),
            child: Padding(
              padding: const EdgeInsets.all(NSpace.lg),
              child: Row(
                children: [
                  Container(
                    width: 48,
                    height: 48,
                    decoration: BoxDecoration(
                      shape: BoxShape.circle,
                      color: primary
                          ? scheme.primary
                          : scheme.primary.withValues(alpha: 0.16),
                    ),
                    child: Icon(
                      icon,
                      size: 24,
                      // `onPrimary` del tema, no un color fijo: la paleta tonal
                      // lo genera con contraste AA por construcción (8.95:1 en
                      // claro, 19.1:1 en oscuro — ver test/theme_test.dart).
                      color: primary ? scheme.onPrimary : scheme.primary,
                    ),
                  ),
                  const SizedBox(width: NSpace.lg),
                  Expanded(
                    child: ExcludeSemantics(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            title,
                            style: theme.textTheme.titleLarge?.copyWith(
                              fontWeight: FontWeight.w800,
                              color: scheme.onSurface,
                            ),
                          ),
                          const SizedBox(height: NSpace.xs),
                          Text(
                            subtitle,
                            style: theme.textTheme.bodyMedium?.copyWith(
                              color: scheme.onSurfaceVariant,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                  Icon(Icons.arrow_forward, color: scheme.onSurfaceVariant),
                ],
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
    final scheme = theme.colorScheme;
    final semanticColors = SemanticColors.of(context);
    final c = controller;

    return Card(
      color: scheme.surfaceContainerLow,
      shape: NShape.rounded(NShape.medium),
      margin: EdgeInsets.zero,
      child: Theme(
        // Sin las líneas divisorias que `ExpansionTile` pinta al abrirse:
        // dentro de una tarjeta son ruido.
        data: theme.copyWith(dividerColor: Colors.transparent),
        child: ExpansionTile(
          shape: const Border(),
          collapsedShape: const Border(),
          iconColor: scheme.onSurfaceVariant,
          collapsedIconColor: scheme.onSurfaceVariant,
          trailing: const Icon(Icons.keyboard_arrow_down_rounded),
          tilePadding: const EdgeInsets.symmetric(
            horizontal: NSpace.xl,
            vertical: NSpace.xs,
          ),
          childrenPadding: const EdgeInsets.fromLTRB(
            NSpace.xl,
            0,
            NSpace.xl,
            NSpace.xl,
          ),
          expandedCrossAxisAlignment: CrossAxisAlignment.start,
          title: Text(
            'Detalles técnicos',
            style: theme.textTheme.titleSmall?.copyWith(
              color: scheme.onSurface,
            ),
          ),
          subtitle: Text(
            'Red, motor y registro de eventos',
            style: theme.textTheme.bodySmall?.copyWith(
              color: scheme.onSurfaceVariant,
            ),
          ),
          children: [
            _InfoRow(
              icon: c.engineKind == EngineKind.native
                  ? Icons.memory
                  : Icons.science_outlined,
              label: 'Motor',
              value: '${c.engineKind.label} · ${c.engineDetail}',
              iconColor: c.engineKind == EngineKind.native
                  ? semanticColors.success.color
                  : semanticColors.warning.color,
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
            const SizedBox(height: NSpace.md),
            Row(
              children: [
                Expanded(
                  child: Text(
                    'Eventos',
                    style: theme.textTheme.titleSmall?.copyWith(
                      color: scheme.onSurface,
                    ),
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
  const _InfoRow({
    required this.icon,
    required this.label,
    required this.value,
    this.iconColor,
  });

  final IconData icon;
  final String label;
  final String value;
  final Color? iconColor;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    return Padding(
      padding: const EdgeInsets.only(bottom: NSpace.sm),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, size: 16, color: iconColor ?? scheme.onSurfaceVariant),
          const SizedBox(width: NSpace.sm),
          SizedBox(
            width: 84,
            child: Text(
              label,
              style: theme.textTheme.bodySmall?.copyWith(
                color: scheme.onSurfaceVariant,
              ),
            ),
          ),
          Expanded(
            child: Text(
              value,
              style: theme.textTheme.bodyMedium?.copyWith(
                fontWeight: FontWeight.w600,
                color: scheme.onSurface,
              ),
            ),
          ),
        ],
      ),
    );
  }
}
