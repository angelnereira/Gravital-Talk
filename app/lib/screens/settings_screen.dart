import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:permission_handler/permission_handler.dart';
import 'package:provider/provider.dart';

import '../core/constants.dart';
import '../core/glass.dart';
import '../core/tokens.dart';
import '../models/connection.dart';
import '../services/engine.dart';
import '../services/session_controller.dart';
import '../widgets/common.dart';

/// Ajustes: permisos, conexión, apariencia y datos de la app.
///
/// El orden es el de lo que más probablemente trae al usuario aquí. Lo
/// primero, los permisos: sin micrófono la app conecta pero no se oye nada, y
/// hasta ahora la única forma de arreglarlo era salir a los ajustes del
/// sistema sin que la app dijera cuál faltaba.
class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

/// Un permiso del sistema que la app usa, con el porqué en lenguaje llano.
class _AppPermission {
  const _AppPermission({
    required this.permission,
    required this.icon,
    required this.title,
    required this.why,
    required this.essential,
  });

  final Permission permission;
  final IconData icon;
  final String title;
  final String why;

  /// Sin él la app no cumple su función (frente a "sólo una comodidad").
  final bool essential;
}

const _permissions = <_AppPermission>[
  _AppPermission(
    permission: Permission.microphone,
    icon: Icons.mic_none,
    title: 'Micrófono',
    why: 'Para que te oigan cuando pulsas para hablar.',
    essential: true,
  ),
  _AppPermission(
    permission: Permission.camera,
    icon: Icons.photo_camera_outlined,
    title: 'Cámara',
    why: 'Para escanear el código de una sala. Sin ella puedes pegar la '
        'invitación.',
    essential: false,
  ),
];

class _SettingsScreenState extends State<SettingsScreen>
    with WidgetsBindingObserver {
  /// Los permisos en tiempo de ejecución existen en móvil; en escritorio los
  /// gestiona el sistema y no hay nada que pedir desde la app.
  static bool get _hasRuntimePermissions =>
      !kIsWeb && (Platform.isAndroid || Platform.isIOS);

  final Map<Permission, PermissionStatus> _status = {};
  String? _publicIp;
  List<String> _localIps = const [];
  bool _checkingNetwork = false;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    _refreshPermissions();
  }

  @override
  void dispose() {
    WidgetsBinding.instance.removeObserver(this);
    super.dispose();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    // Al volver de los ajustes del sistema el estado puede haber cambiado.
    if (state == AppLifecycleState.resumed) _refreshPermissions();
  }

  Future<void> _refreshPermissions() async {
    if (!_hasRuntimePermissions) return;
    final fresh = <Permission, PermissionStatus>{};
    for (final p in _permissions) {
      try {
        fresh[p.permission] = await p.permission.status;
      } catch (_) {
        // Plataforma sin ese permiso: se deja sin estado y la fila no sale.
      }
    }
    if (mounted) setState(() => _status..clear()..addAll(fresh));
  }

  /// Pide el permiso; si el sistema ya no deja preguntar, abre sus ajustes.
  Future<void> _grant(_AppPermission p) async {
    final current = _status[p.permission];
    if (current == PermissionStatus.permanentlyDenied ||
        current == PermissionStatus.restricted) {
      await openAppSettings();
      return;
    }
    final result = await p.permission.request();
    if (!mounted) return;
    setState(() => _status[p.permission] = result);
    // Denegado "para siempre" (segunda negativa en Android): la única vía
    // que queda son los ajustes del sistema, así que se lleva allí.
    if (result == PermissionStatus.permanentlyDenied) await openAppSettings();
  }

  Future<void> _checkNetwork(SessionController c) async {
    setState(() => _checkingNetwork = true);
    final ips = await c.localAddresses();
    final pub = await c.discoverPublicAddress();
    if (!mounted) return;
    setState(() {
      _localIps = ips;
      _publicIp = pub;
      _checkingNetwork = false;
    });
  }

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();
    final s = c.settings;

    return Scaffold(
      appBar: AppBar(title: const Text('Ajustes')),
      body: Center(
        child: ConstrainedBox(
          constraints: const BoxConstraints(maxWidth: 560),
          child: ListView(
            padding: const EdgeInsets.fromLTRB(
                Spacing.xl, Spacing.md, Spacing.xl, Spacing.xxxl),
            children: [
              _Section(
                title: 'Permisos',
                subtitle: _hasRuntimePermissions
                    ? 'Lo que la app necesita del teléfono, y para qué.'
                    : 'En este equipo los concede el sistema, no la app.',
                children: _hasRuntimePermissions
                    ? [
                        for (final p in _permissions)
                          if (_status.containsKey(p.permission))
                            _PermissionRow(
                              permission: p,
                              status: _status[p.permission]!,
                              onGrant: () => _grant(p),
                            ),
                        _Row(
                          icon: Icons.tune,
                          title: 'Todos los permisos',
                          subtitle: 'Abre los ajustes del sistema para esta '
                              'app.',
                          trailing: const Icon(Icons.open_in_new, size: 18),
                          onTap: openAppSettings,
                        ),
                      ]
                    : const [
                        _Row(
                          icon: Icons.mic_none,
                          title: 'Micrófono y altavoz',
                          subtitle: 'Se usan los dispositivos de audio por '
                              'defecto del sistema. Cámbialos en sus ajustes '
                              'de sonido.',
                        ),
                      ],
              ),
              _Section(
                title: 'Conexión',
                subtitle: 'Las llamadas van directas entre los dos '
                    'dispositivos. No hay nada que configurar.',
                children: [
                  _Row(
                    icon: Icons.wifi_tethering,
                    title: 'Comprobar mi red',
                    subtitle: _networkSummary(),
                    trailing: _checkingNetwork
                        ? const SizedBox(
                            width: 18,
                            height: 18,
                            child: CircularProgressIndicator(strokeWidth: 2),
                          )
                        : const Icon(Icons.refresh, size: 20),
                    onTap: _checkingNetwork ? null : () => _checkNetwork(c),
                  ),
                  _Row(
                    icon: Icons.hub_outlined,
                    title: 'Servidor de respaldo',
                    subtitle: s.rendezvous.isEmpty
                        ? 'Opcional y sin configurar. Sólo hace falta si dos '
                            'redes móviles no logran conectar directo.'
                        : '${s.rendezvous} · se usa sólo si no hay camino '
                            'directo.',
                    trailing: const Icon(Icons.chevron_right),
                    onTap: () => _editRendezvous(c),
                  ),
                ],
              ),
              _Section(
                title: 'Apariencia',
                children: [
                  Padding(
                    padding: const EdgeInsets.symmetric(vertical: Spacing.sm),
                    child: SegmentedButton<AppThemeMode>(
                      segments: [
                        for (final mode in AppThemeMode.values)
                          ButtonSegment(value: mode, label: Text(mode.label)),
                      ],
                      selected: {s.themeMode},
                      onSelectionChanged: (sel) =>
                          c.updateSettings(s.copyWith(themeMode: sel.first)),
                    ),
                  ),
                ],
              ),
              _Section(
                title: 'Acerca de',
                children: [
                  const _Row(
                    icon: Icons.info_outline,
                    title: 'Versión',
                    subtitle: appVersion,
                  ),
                  _Row(
                    icon: Icons.memory,
                    title: 'Motor de audio',
                    subtitle: '${c.engineKind.label} · ${c.engineDetail}',
                  ),
                  const _Row(
                    icon: Icons.lock_outline,
                    title: 'Cifrado',
                    subtitle: 'De extremo a extremo (X25519 + '
                        'ChaCha20-Poly1305). Nadie en medio puede escuchar.',
                  ),
                ],
              ),
            ],
          ),
        ),
      ),
    );
  }

  String _networkSummary() {
    if (_localIps.isEmpty && _publicIp == null) {
      return 'Muestra tu dirección local y la pública.';
    }
    final local = _localIps.isEmpty ? '—' : _localIps.join(', ');
    return 'Local: $local\nPública: ${_publicIp ?? 'no disponible'}';
  }

  /// Pide la dirección del servidor de respaldo.
  Future<void> _editRendezvous(SessionController c) async {
    final ctl = TextEditingController(text: c.settings.rendezvous);
    String? error;
    final value = await showDialog<String>(
      context: context,
      builder: (dialogCtx) => StatefulBuilder(
        builder: (dialogCtx, setDialog) => AlertDialog(
          title: const Text('Servidor de respaldo'),
          content: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Dirección de un gs-relay propio. Presenta a los dos '
                'dispositivos y, sólo si no hay camino directo, reenvía el '
                'audio cifrado. Déjalo vacío para no usar ninguno.',
              ),
              const SizedBox(height: Spacing.md),
              TextField(
                controller: ctl,
                autofocus: true,
                autocorrect: false,
                keyboardType: TextInputType.url,
                decoration: InputDecoration(
                  hintText: 'relay.ejemplo.com:9000',
                  errorText: error,
                ),
              ),
            ],
          ),
          actions: [
            TextButton(
              onPressed: () => Navigator.pop(dialogCtx),
              child: const Text('Cancelar'),
            ),
            FilledButton(
              onPressed: () {
                final text = ctl.text.trim();
                if (text.isNotEmpty && splitHostPort(text) == null) {
                  setDialog(() => error = 'Usa la forma host:puerto');
                  return;
                }
                Navigator.pop(dialogCtx, text);
              },
              child: const Text('Guardar'),
            ),
          ],
        ),
      ),
    );
    if (value == null) return;
    c.updateSettings(c.settings.copyWith(rendezvous: value));
  }
}

/// Bloque de ajustes: título, explicación opcional y sus filas sobre vidrio.
class _Section extends StatelessWidget {
  const _Section({required this.title, this.subtitle, required this.children});

  final String title;
  final String? subtitle;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    return Padding(
      padding: const EdgeInsets.only(bottom: Spacing.xl),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: const EdgeInsets.only(left: Spacing.xs, bottom: Spacing.sm),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: theme.textTheme.titleMedium?.copyWith(
                    fontWeight: FontWeight.w800,
                    color: glass.primary,
                  ),
                ),
                if (subtitle != null)
                  Text(
                    subtitle!,
                    style: theme.textTheme.bodySmall
                        ?.copyWith(color: glass.secondary),
                  ),
              ],
            ),
          ),
          GlassSurface(
            padding: const EdgeInsets.symmetric(
                horizontal: Spacing.lg, vertical: Spacing.sm),
            child: Column(
              children: [
                for (var i = 0; i < children.length; i++) ...[
                  if (i > 0)
                    Divider(
                      height: 1,
                      color: glass.secondary.withValues(alpha: 0.18),
                    ),
                  children[i],
                ],
              ],
            ),
          ),
        ],
      ),
    );
  }
}

/// Fila de ajustes: icono, título, explicación y una acción opcional.
class _Row extends StatelessWidget {
  const _Row({
    required this.icon,
    required this.title,
    required this.subtitle,
    this.trailing,
    this.onTap,
  });

  final IconData icon;
  final String title;
  final String subtitle;
  final Widget? trailing;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    final row = Padding(
      padding: const EdgeInsets.symmetric(vertical: Spacing.md),
      child: Row(
        children: [
          Icon(icon, color: glass.secondary),
          const SizedBox(width: Spacing.lg),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  title,
                  style: theme.textTheme.bodyLarge?.copyWith(
                    fontWeight: FontWeight.w600,
                    color: glass.primary,
                  ),
                ),
                const SizedBox(height: Spacing.xxs),
                Text(
                  subtitle,
                  style: theme.textTheme.bodySmall
                      ?.copyWith(color: glass.secondary),
                ),
              ],
            ),
          ),
          if (trailing != null) ...[
            const SizedBox(width: Spacing.md),
            IconTheme.merge(
              data: IconThemeData(color: glass.secondary),
              child: trailing!,
            ),
          ],
        ],
      ),
    );
    if (onTap == null) return row;
    return InkWell(
      onTap: onTap,
      borderRadius: BorderRadius.circular(Radii.md),
      child: row,
    );
  }
}

/// Fila de un permiso: qué es, para qué, cómo está y el botón para darlo.
class _PermissionRow extends StatelessWidget {
  const _PermissionRow({
    required this.permission,
    required this.status,
    required this.onGrant,
  });

  final _AppPermission permission;
  final PermissionStatus status;
  final VoidCallback onGrant;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final granted = status.isGranted || status.isLimited;
    // Denegado sin posibilidad de volver a preguntar: el botón lleva a los
    // ajustes del sistema, y lo dice.
    final blocked = status.isPermanentlyDenied || status.isRestricted;

    final Widget trailing = granted
        ? Semantics(
            label: 'Concedido',
            child: const Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(Icons.check_circle, color: GravitalColors.online, size: 20),
                SizedBox(width: Spacing.xs),
                Text(
                  'Concedido',
                  style: TextStyle(
                    color: GravitalColors.online,
                    fontWeight: FontWeight.w700,
                  ),
                ),
              ],
            ),
          )
        : FilledButton(
            onPressed: onGrant,
            style: permission.essential
                ? null
                : FilledButton.styleFrom(
                    backgroundColor: theme.colorScheme.secondaryContainer,
                    foregroundColor: theme.colorScheme.onSecondaryContainer,
                  ),
            child: Text(blocked ? 'Abrir ajustes' : 'Permitir'),
          );

    return _Row(
      icon: permission.icon,
      title: permission.title,
      subtitle: granted
          ? permission.why
          : permission.essential
              ? '${permission.why} Sin él puedes escuchar pero no hablar.'
              : permission.why,
      trailing: trailing,
    );
  }
}
