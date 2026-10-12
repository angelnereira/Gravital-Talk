import 'dart:async';
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
import '../services/contacts.dart';
import '../services/haptics.dart';
import '../services/session_controller.dart';
import '../services/share.dart';
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
  const PairingScreen({
    super.key,
    required this.mode,
    this.invite,
    this.callee,
    this.caller,
  });

  /// Nombre del contacto cuya llamada se está aceptando (modo unirse): con
  /// él no hay nada que escanear, sólo esperar a que conecte.
  final String? caller;

  /// Contacto al que se llama (modo anfitrión): la sala se abre igual, pero
  /// la invitación se le envía sola en vez de esperar a que la escanee.
  final Contact? callee;

  final PairingMode mode;

  /// Invitación con la que entrar nada más abrir (modo unirse): la que llegó
  /// por un enlace. Así quien abre un enlace ve lo mismo que quien escanea,
  /// incluido el código de respuesta si están en redes distintas.
  final String? invite;

  @override
  Widget build(BuildContext context) {
    return switch (mode) {
      PairingMode.host => _HostView(callee: callee),
      PairingMode.join => _JoinView(invite: invite, caller: caller),
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
  const _HostView({this.callee});

  final Contact? callee;

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
    _contacts = context.read<ContactsService>();
    final callee = widget.callee;
    if (callee != null) _contacts.call(callee);
  }

  late final ContactsService _contacts;

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
    if (widget.callee != null) _contacts.cancelCall();
    if (!_handedOff) _controller.reset();
    super.dispose();
  }

  /// Sólo el móvil tiene cámara con la que escanear.
  static bool get _canScan =>
      !kIsWeb && (Platform.isAndroid || Platform.isIOS);

  Future<void> _useAnswer(String raw) async {
    final ok = await _controller.acceptAnswer(raw);
    if (!mounted || !ok) return;
    ScaffoldMessenger.of(context).showSnackBar(
      const SnackBar(content: Text('Respuesta recibida: conectando…')),
    );
  }

  Future<void> _pasteAnswer() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    final text = data?.text?.trim() ?? '';
    if (text.isEmpty) {
      _controller.reportError('No hay ninguna respuesta copiada.');
      return;
    }
    await _useAnswer(text);
  }

  Future<void> _scanAnswer() async {
    final raw = await showModalBottomSheet<String>(
      context: context,
      isScrollControlled: true,
      builder: (sheetCtx) => SizedBox(
        height: 440,
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.all(Spacing.lg),
              child: Text(
                'Escanea el código de respuesta',
                style: Theme.of(sheetCtx).textTheme.titleMedium,
              ),
            ),
            Expanded(
              child: MobileScanner(
                controller: MobileScannerController(
                  formats: const [BarcodeFormat.qrCode],
                ),
                onDetect: (capture) {
                  final value = capture.barcodes.isEmpty
                      ? null
                      : capture.barcodes.first.rawValue;
                  if (value != null && value.trim().isNotEmpty) {
                    Haptics.scanned();
                    Navigator.of(sheetCtx).pop(value.trim());
                  }
                },
              ),
            ),
          ],
        ),
      ),
    );
    if (raw != null) await _useAnswer(raw);
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

    final callee = widget.callee;
    if (callee != null) {
      return _CallingView(
        name: callee.name,
        error: c.error,
        onCancel: () => Navigator.of(context).maybePop(),
      );
    }

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
                  if (c.error != null && invite == null)
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
                  if (invite != null) ...[
                    if (c.error != null) ...[
                      const SizedBox(height: Spacing.lg),
                      ErrorBanner(message: c.error!, onDismiss: c.clearError),
                    ],
                    const SizedBox(height: Spacing.lg),
                    FilledButton.tonalIcon(
                      onPressed: () async {
                        final shared = await shareText(
                            'Únete a mi sala de Gravital Talk:\n$invite');
                        if (!shared && mounted) setState(() => _copied = true);
                      },
                      icon: const Icon(Icons.ios_share),
                      label: const Text('Compartir enlace de la sala'),
                    ),
                    const SizedBox(height: Spacing.md),
                    OutlinedButton.icon(
                      onPressed: () => _copyInvite(invite),
                      icon: Icon(_copied ? Icons.check : Icons.link),
                      label: Text(
                        _copied ? 'Invitación copiada' : 'Copiar invitación',
                      ),
                    ),
                    const SizedBox(height: Spacing.xl),
                    // Para cuando los dos están en redes distintas: el
                    // invitado enseña un código con su dirección y aquí se lee.
                    Text(
                      c.reachableFromOtherNetworks
                          ? '¿No conecta desde otra red?'
                          : '¿Está en otra red (datos móviles, otra wifi)?',
                      textAlign: TextAlign.center,
                      style: theme.textTheme.titleSmall
                          ?.copyWith(color: glass.primary),
                    ),
                    const SizedBox(height: Spacing.xs),
                    Text(
                      'Al unirse le aparece un código de respuesta. Léelo '
                      'aquí y os conectáis directo.',
                      textAlign: TextAlign.center,
                      style: theme.textTheme.bodySmall
                          ?.copyWith(color: glass.secondary),
                    ),
                    const SizedBox(height: Spacing.md),
                    if (_canScan)
                      FilledButton.tonalIcon(
                        onPressed: _scanAnswer,
                        icon: const Icon(Icons.qr_code_scanner),
                        label: const Text('Escanear su respuesta'),
                      ),
                    if (_canScan) const SizedBox(height: Spacing.sm),
                    OutlinedButton.icon(
                      onPressed: _pasteAnswer,
                      icon: const Icon(Icons.content_paste),
                      label: const Text('Pegar su respuesta'),
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
  const _JoinView({this.invite, this.caller});

  final String? invite;
  final String? caller;

  @override
  State<_JoinView> createState() => _JoinViewState();
}

class _JoinViewState extends State<_JoinView> {
  final _inviteCtl = TextEditingController();
  MobileScannerController? _scanner;
  bool _joining = false;
  bool _showAnswer = false;

  /// `true` cuando ya se conectó y se pasa a la pantalla de hablar.
  bool _handedOff = false;

  /// Sólo el móvil tiene cámara con la que escanear; en escritorio la
  /// invitación se pega.
  static bool get _canScan =>
      !kIsWeb && (Platform.isAndroid || Platform.isIOS);

  late final SessionController _controller;

  @override
  void initState() {
    super.initState();
    _controller = context.read<SessionController>();
    final invite = widget.invite;
    if (invite != null) {
      // Con invitación ya en mano no hace falta la cámara.
      _joining = true;
      WidgetsBinding.instance.addPostFrameCallback((_) {
        _joining = false;
        _join(invite);
      });
    } else if (_canScan) {
      _scanner = MobileScannerController(formats: const [BarcodeFormat.qrCode]);
    }
  }

  @override
  void dispose() {
    // Salir a medio conectar cancela el intento: si no, la sesión seguiría
    // sondando en segundo plano (hasta minuto y medio sin servidor).
    if (_joining && !_handedOff) _controller.reset();
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
    // Escanear o pegar un enlace de contacto aquí también vale: se guarda y
    // se vuelve, no hay sala en la que entrar.
    if (ContactLink.tryParse(invite) != null) {
      final contacts = context.read<ContactsService>();
      final navigator = Navigator.of(context);
      final messenger = ScaffoldMessenger.of(context);
      final contact = await contacts.addFromLink(invite);
      messenger.showSnackBar(SnackBar(
        content: Text(contact == null
            ? 'Ese enlace de contacto no es válido.'
            : '${contact.name} añadido a tus contactos'),
      ));
      if (contact != null) navigator.pop();
      return;
    }
    setState(() {
      _joining = true;
      _showAnswer = false;
    });
    await _scanner?.stop();
    // En la misma red se conecta en décimas, y entre redes la respuesta le
    // llega sola al anfitrión (red local o buzón) y conecta en unos segundos.
    // El código para enseñársela a mano es el último recurso: sólo aparece
    // si ni así se ha conectado.
    final reveal = Timer(const Duration(seconds: 12), () {
      if (mounted) setState(() => _showAnswer = true);
    });
    final ok = await c.joinFromUri(invite);
    reveal.cancel();
    if (!mounted) return;
    if (ok && c.isLive) {
      // Marcado antes de navegar: al sustituir esta pantalla se destruye, y
      // sin la marca su `dispose` tomaría la sesión recién conectada por un
      // intento a medias y la cerraría.
      _handedOff = true;
      _goToSession(context);
      return;
    }
    setState(() => _joining = false);
    // Falló una invitación abierta por enlace: se ofrece la cámara para
    // reintentar con otra.
    if (_scanner == null && _canScan) {
      setState(() {
        _scanner =
            MobileScannerController(formats: const [BarcodeFormat.qrCode]);
      });
    } else {
      await _scanner?.start();
    }
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
    final caller = widget.caller;
    if (caller != null) {
      return _CallingView(
        name: caller,
        title: 'Conectando',
        status: 'Entrando en la conversación…',
        error: context.watch<SessionController>().error,
        onCancel: () => Navigator.of(context).maybePop(),
      );
    }
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
                    _JoiningPanel(
                      answer: _showAnswer ? c.answerUri : null,
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
                                if (!_joining) Haptics.scanned();
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

/// Lo que ve el invitado mientras conecta.
///
/// Empieza siendo sólo "Conectando…". Si la conexión tarda, aparece el código
/// de respuesta: la dirección de este dispositivo, para que el anfitrión la
/// lea y abra su red hacia aquí.
class _JoiningPanel extends StatefulWidget {
  const _JoiningPanel({required this.answer});

  /// Respuesta a enseñar, o `null` si aún no toca.
  final String? answer;

  @override
  State<_JoiningPanel> createState() => _JoiningPanelState();
}

class _JoiningPanelState extends State<_JoiningPanel> {
  bool _copied = false;

  Future<void> _copy(String answer) async {
    await Clipboard.setData(ClipboardData(text: answer));
    if (!mounted) return;
    setState(() => _copied = true);
    await Future<void>.delayed(const Duration(seconds: 2));
    if (mounted) setState(() => _copied = false);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    final answer = widget.answer;

    if (answer == null) {
      return Padding(
        padding: const EdgeInsets.symmetric(vertical: 80),
        child: LoadingStates.centered(message: 'Conectando…'),
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        GlassSurface(
          padding: const EdgeInsets.all(Spacing.xxl),
          child: Column(
            children: [
              Text(
                'Enséñale este código',
                textAlign: TextAlign.center,
                style: theme.textTheme.titleLarge?.copyWith(
                  fontWeight: FontWeight.w800,
                  color: glass.primary,
                ),
              ),
              const SizedBox(height: Spacing.xs),
              Text(
                'Estáis en redes distintas. Quien creó la sala tiene que '
                'leerlo con "Escanear su respuesta".',
                textAlign: TextAlign.center,
                style: theme.textTheme.bodyMedium
                    ?.copyWith(color: glass.secondary),
              ),
              const SizedBox(height: Spacing.xl),
              Semantics(
                label: 'Código QR de respuesta para el anfitrión',
                child: Container(
                  padding: const EdgeInsets.all(Spacing.md),
                  decoration: BoxDecoration(
                    color: Colors.white,
                    borderRadius: BorderRadius.circular(Radii.glass),
                  ),
                  child: QrImageView(
                    data: answer,
                    size: 220,
                    backgroundColor: Colors.white,
                  ),
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
                      'Sigo intentando conectar…',
                      style: theme.textTheme.bodyMedium
                          ?.copyWith(color: glass.secondary),
                    ),
                  ),
                ],
              ),
            ],
          ),
        ),
        const SizedBox(height: Spacing.lg),
        OutlinedButton.icon(
          onPressed: () => _copy(answer),
          icon: Icon(_copied ? Icons.check : Icons.link),
          label: Text(_copied ? 'Respuesta copiada' : 'Copiar respuesta'),
        ),
      ],
    );
  }
}

/// Llamada a un contacto: sólo se espera a que conteste.
///
/// Con un contacto no hay nada que escanear ni que pegar: la confianza se
/// estableció al añadirlo (el secreto compartido) y la llamada viaja sellada
/// con él. Por eso aquí no hay códigos, sólo el estado de la llamada.
class _CallingView extends StatefulWidget {
  const _CallingView({
    required this.name,
    required this.error,
    required this.onCancel,
    this.title = 'Llamando',
    this.status = 'Esperando a que conteste…',
  });

  final String title;
  final String status;
  final String name;
  final String? error;
  final VoidCallback onCancel;

  @override
  State<_CallingView> createState() => _CallingViewState();
}

class _CallingViewState extends State<_CallingView> {
  Timer? _timer;
  bool _slow = false;

  @override
  void initState() {
    super.initState();
    _timer = Timer(const Duration(seconds: 15), () {
      if (mounted) setState(() => _slow = true);
    });
  }

  @override
  void dispose() {
    _timer?.cancel();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    final name = widget.name;
    final initial = name.isEmpty ? '?' : name.characters.first.toUpperCase();

    return Scaffold(
      appBar: AppBar(title: Text(widget.title)),
      body: SafeArea(
        child: Center(
          child: ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 420),
            child: Padding(
              padding: const EdgeInsets.all(Spacing.xl),
              child: Column(
                mainAxisSize: MainAxisSize.min,
                children: [
                  CircleAvatar(
                    radius: 48,
                    backgroundColor:
                        theme.colorScheme.primary.withValues(alpha: 0.2),
                    child: Text(
                      initial,
                      style: theme.textTheme.displaySmall?.copyWith(
                        fontWeight: FontWeight.w800,
                        color: glass.primary,
                      ),
                    ),
                  ),
                  const SizedBox(height: Spacing.xl),
                  Text(
                    name,
                    textAlign: TextAlign.center,
                    style: theme.textTheme.headlineSmall?.copyWith(
                      fontWeight: FontWeight.w800,
                      color: glass.primary,
                    ),
                  ),
                  const SizedBox(height: Spacing.sm),
                  Row(
                    mainAxisAlignment: MainAxisAlignment.center,
                    children: [
                      const SizedBox(
                        width: 16,
                        height: 16,
                        child: CircularProgressIndicator(strokeWidth: 2),
                      ),
                      const SizedBox(width: Spacing.md),
                      Text(
                        widget.status,
                        style: theme.textTheme.bodyLarge
                            ?.copyWith(color: glass.secondary),
                      ),
                    ],
                  ),
                  if (_slow) ...[
                    const SizedBox(height: Spacing.lg),
                    Text(
                      'Está tardando. Puede que no tenga la app abierta o '
                      'que no tenga conexión ahora mismo.',
                      textAlign: TextAlign.center,
                      style: theme.textTheme.bodyMedium
                          ?.copyWith(color: glass.secondary),
                    ),
                  ],
                  if (widget.error != null) ...[
                    const SizedBox(height: Spacing.lg),
                    Text(
                      widget.error!,
                      textAlign: TextAlign.center,
                      style: theme.textTheme.bodyMedium
                          ?.copyWith(color: theme.colorScheme.error),
                    ),
                  ],
                  const SizedBox(height: Spacing.xxxl),
                  OutlinedButton.icon(
                    onPressed: widget.onCancel,
                    icon: Icon(Icons.call_end, color: theme.colorScheme.error),
                    label: Text(
                      'Cancelar',
                      style: TextStyle(color: theme.colorScheme.error),
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}
