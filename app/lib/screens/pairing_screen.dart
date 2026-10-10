import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:provider/provider.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../core/glass.dart';
import '../core/routes.dart';
import '../core/states.dart';
import '../core/tokens.dart';
import '../models/session.dart';
import '../services/session_controller.dart';
import 'session_screen.dart';

/// Qué viene a hacer el usuario a la pantalla de emparejamiento.
enum PairingMode {
  /// Crear una sala y enseñar la invitación.
  host,

  /// Entrar en la sala de otro.
  join,
}

/// Emparejamiento en un paso.
///
/// Cada modo hace una sola cosa y la hace nada más entrar:
///
/// - **Crear** abre la sala al instante y enseña el QR. No hay botón de
///   "crear": quien ha llegado aquí ya lo ha pedido.
/// - **Unirse** enciende la cámara. Leer el QR conecta; no rellena un
///   formulario que luego haya que enviar.
///
/// En ambos, conectar lleva solo a la pantalla de hablar. No hay campos de red
/// en ningún sitio: la dirección viaja dentro de la invitación.
class PairingScreen extends StatelessWidget {
  const PairingScreen({super.key, required this.mode});

  final PairingMode mode;

  @override
  Widget build(BuildContext context) {
    return switch (mode) {
      PairingMode.host => const _HostView(),
      PairingMode.join => const _JoinView(),
    };
  }
}

/// Sustituye esta pantalla por la de hablar.
void _goToSession(BuildContext context) {
  Navigator.of(context).pushReplacement(
    ForwardRoute(builder: (_) => const SessionScreen()),
  );
}

// ── Crear ──────────────────────────────────────────────────────────────────

class _HostView extends StatefulWidget {
  const _HostView();

  @override
  State<_HostView> createState() => _HostViewState();
}

class _HostViewState extends State<_HostView> {
  late final SessionController _controller;
  bool _copied = false;

  /// `true` cuando la sala ya tiene invitado y pasa a la pantalla de hablar.
  bool _handedOff = false;

  @override
  void initState() {
    super.initState();
    _controller = context.read<SessionController>();
    // Tras el primer frame: crear la sala notifica a los oyentes, y eso no
    // puede ocurrir mientras el árbol se está construyendo.
    WidgetsBinding.instance.addPostFrameCallback((_) => _open());
  }

  Future<void> _open() async {
    final ok = await _controller.createRoom();
    if (!mounted) return;
    if (ok && _controller.isLive) {
      _handedOff = true;
      _goToSession(context);
    }
  }

  @override
  void dispose() {
    // Salir sin que haya entrado nadie cierra la sala: dejarla abierta en
    // segundo plano sería un puerto escuchando sin que el usuario lo sepa.
    if (!_handedOff) _controller.reset();
    super.dispose();
  }

  Future<void> _copyInvite(String invite) async {
    await Clipboard.setData(ClipboardData(text: invite));
    if (!mounted) return;
    setState(() => _copied = true);
    await Future<void>.delayed(const Duration(seconds: 2));
    if (mounted) setState(() => _copied = false);
  }

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    final invite = c.pairingUri?.toUri();

    return Scaffold(
      appBar: AppBar(title: const Text('Tu sala')),
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(Spacing.xl),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 420),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  if (c.engineKind == EngineKind.demo) ...[
                    const WarningBanner(
                      message: 'Modo demostración: esta sala no acepta '
                          'conexiones reales.',
                    ),
                    const SizedBox(height: Spacing.lg),
                  ],
                  if (c.error != null)
                    ErrorBanner(
                      message: c.error!,
                      onRetry: c.busy ? null : _open,
                      onDismiss: c.clearError,
                    )
                  else if (invite == null)
                    LoadingStates.centered(message: 'Abriendo la sala…')
                  else
                    GlassSurface(
                      glow: theme.colorScheme.primary.withValues(alpha: 0.25),
                      padding: const EdgeInsets.all(Spacing.xxl),
                      child: Column(
                        children: [
                          Text(
                            'Que escaneen este código',
                            textAlign: TextAlign.center,
                            style: theme.textTheme.titleLarge?.copyWith(
                              fontWeight: FontWeight.w800,
                              color: glass.primary,
                            ),
                          ),
                          const SizedBox(height: Spacing.xs),
                          Text(
                            c.reachableFromOtherNetworks
                                ? 'Con Gravital Talk, desde cualquier red.'
                                : 'Con Gravital Talk, desde la misma red wifi.',
                            textAlign: TextAlign.center,
                            style: theme.textTheme.bodyMedium
                                ?.copyWith(color: glass.secondary),
                          ),
                          const SizedBox(height: Spacing.xl),
                          Semantics(
                            label: 'Código QR de invitación a la sala',
                            child: Container(
                              padding: const EdgeInsets.all(Spacing.md),
                              decoration: BoxDecoration(
                                color: Colors.white,
                                borderRadius: BorderRadius.circular(Radii.glass),
                              ),
                              child: QrImageView(
                                data: invite,
                                size: 232,
                                backgroundColor: Colors.white,
                              ),
                            ),
                          ),
                          const SizedBox(height: Spacing.xl),
                          Text(
                            c.roomCode ?? '',
                            style: theme.textTheme.headlineSmall?.copyWith(
                              fontWeight: FontWeight.w800,
                              letterSpacing: 4,
                              color: glass.primary,
                            ),
                          ),
                          const SizedBox(height: Spacing.lg),
                          Row(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              const SizedBox(
                                width: 16,
                                height: 16,
                                child: CircularProgressIndicator(strokeWidth: 2),
                              ),
                              const SizedBox(width: Spacing.md),
                              Flexible(
                                child: Text(
                                  'Esperando a que alguien se una…',
                                  style: theme.textTheme.bodyMedium
                                      ?.copyWith(color: glass.secondary),
                                ),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                  if (invite != null && c.error == null) ...[
                    const SizedBox(height: Spacing.lg),
                    OutlinedButton.icon(
                      onPressed: () => _copyInvite(invite),
                      icon: Icon(_copied ? Icons.check : Icons.link),
                      label: Text(
                        _copied ? 'Invitación copiada' : 'Copiar invitación',
                      ),
                    ),
                  ],
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

// ── Unirse ─────────────────────────────────────────────────────────────────

class _JoinView extends StatefulWidget {
  const _JoinView();

  @override
  State<_JoinView> createState() => _JoinViewState();
}

class _JoinViewState extends State<_JoinView> {
  final _inviteCtl = TextEditingController();
  MobileScannerController? _scanner;
  bool _joining = false;

  /// Sólo el móvil tiene cámara con la que escanear; en escritorio la
  /// invitación se pega.
  static bool get _canScan =>
      !kIsWeb && (Platform.isAndroid || Platform.isIOS);

  @override
  void initState() {
    super.initState();
    if (_canScan) {
      _scanner = MobileScannerController(formats: const [BarcodeFormat.qrCode]);
    }
  }

  @override
  void dispose() {
    _inviteCtl.dispose();
    _scanner?.dispose();
    super.dispose();
  }

  Future<void> _join(String raw) async {
    if (_joining) return;
    final invite = raw.trim();
    final c = context.read<SessionController>();
    if (invite.isEmpty) {
      c.reportError('Pega la invitación que te han pasado.');
      return;
    }
    setState(() => _joining = true);
    await _scanner?.stop();
    final ok = await c.joinFromUri(invite);
    if (!mounted) return;
    if (ok && c.isLive) {
      _goToSession(context);
      return;
    }
    setState(() => _joining = false);
    await _scanner?.start();
  }

  Future<void> _paste() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    final text = data?.text?.trim() ?? '';
    if (text.isEmpty) return;
    _inviteCtl.text = text;
    await _join(text);
  }

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);

    return Scaffold(
      appBar: AppBar(title: const Text('Unirse')),
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(Spacing.xl),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 420),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  if (c.error != null) ...[
                    ErrorBanner(message: c.error!, onDismiss: c.clearError),
                    const SizedBox(height: Spacing.lg),
                  ],
                  if (_joining)
                    Padding(
                      padding: const EdgeInsets.symmetric(vertical: 80),
                      child: LoadingStates.centered(message: 'Conectando…'),
                    )
                  else ...[
                    if (_scanner != null) ...[
                      Text(
                        'Apunta al código de la otra pantalla',
                        textAlign: TextAlign.center,
                        style: theme.textTheme.titleLarge?.copyWith(
                          fontWeight: FontWeight.w800,
                          color: glass.primary,
                        ),
                      ),
                      const SizedBox(height: Spacing.lg),
                      ClipRRect(
                        borderRadius: BorderRadius.circular(Radii.glass),
                        child: AspectRatio(
                          aspectRatio: 1,
                          child: MobileScanner(
                            controller: _scanner,
                            onDetect: (capture) {
                              final value = capture.barcodes.isEmpty
                                  ? null
                                  : capture.barcodes.first.rawValue;
                              if (value != null && value.trim().isNotEmpty) {
                                _join(value);
                              }
                            },
                          ),
                        ),
                      ),
                      const SizedBox(height: Spacing.xl),
                    ],
                    GlassSurface(
                      padding: const EdgeInsets.all(Spacing.xl),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.stretch,
                        children: [
                          Text(
                            _scanner != null
                                ? '¿Te la han enviado por mensaje?'
                                : 'Pega la invitación',
                            style: theme.textTheme.titleMedium?.copyWith(
                              fontWeight: FontWeight.w700,
                              color: glass.primary,
                            ),
                          ),
                          const SizedBox(height: Spacing.md),
                          TextField(
                            controller: _inviteCtl,
                            autocorrect: false,
                            enableSuggestions: false,
                            textInputAction: TextInputAction.go,
                            onSubmitted: _join,
                            decoration: InputDecoration(
                              hintText: 'gravital-talk://pair?…',
                              suffixIcon: IconButton(
                                tooltip: 'Pegar del portapapeles',
                                onPressed: _paste,
                                icon: const Icon(Icons.content_paste),
                              ),
                            ),
                          ),
                          const SizedBox(height: Spacing.md),
                          FilledButton.icon(
                            onPressed: () => _join(_inviteCtl.text),
                            icon: const Icon(Icons.login),
                            label: const Text('Unirse'),
                          ),
                        ],
                      ),
                    ),
                  ],
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
