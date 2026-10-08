import 'package:flutter/foundation.dart';
import 'package:flutter/material.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:provider/provider.dart';
import 'package:toastification/toastification.dart';

import '../core/constants.dart';
import '../models/connection.dart';
import '../services/session_controller.dart';
import '../widgets/common.dart';
import 'session_screen.dart';

/// Configuración del modo servidor (sala con relay como terminal central).
class ServerSetupScreen extends StatefulWidget {
  const ServerSetupScreen({super.key});

  @override
  State<ServerSetupScreen> createState() => _ServerSetupScreenState();
}

class _ServerSetupScreenState extends State<ServerSetupScreen> {
  late final TextEditingController _host;
  late final TextEditingController _udpPort;
  late final TextEditingController _obsPort;
  late final TextEditingController _roomCode;
  ConnectionRole _role = ConnectionRole.host;

  @override
  void initState() {
    super.initState();
    final s = context.read<SessionController>().server;
    _host = TextEditingController(text: s.host);
    _udpPort = TextEditingController(text: '${s.udpPort}');
    _obsPort = TextEditingController(text: '${s.observabilityPort}');
    _roomCode = TextEditingController(text: s.roomCode);
  }

  @override
  void dispose() {
    _host.dispose();
    _udpPort.dispose();
    _obsPort.dispose();
    _roomCode.dispose();
    super.dispose();
  }

  void _persist(SessionController c) {
    c.updateServerProfile(ServerProfile(
      host: _host.text.trim(),
      udpPort: int.tryParse(_udpPort.text) ?? DefaultPorts.udp,
      observabilityPort:
          int.tryParse(_obsPort.text) ?? DefaultPorts.observability,
      roomCode: _roomCode.text.trim().toUpperCase(),
    ));
  }

  Future<void> _connect(SessionController c) async {
    _persist(c);
    final ok = _role == ConnectionRole.host
        ? await c.hostServer()
        : await c.joinServer();
    if (!mounted) return;
    if (ok && c.isLive) {
      Navigator.of(context).pushReplacement(
          MaterialPageRoute(builder: (_) => const SessionScreen()));
    }
  }

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();
    final hosting = _role == ConnectionRole.host;
    final scheme = Theme.of(context).colorScheme;

    return Scaffold(
      appBar: AppBar(title: const Text('Servidor · Sala')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          SegmentedButton<ConnectionRole>(
            segments: const [
              ButtonSegment(
                value: ConnectionRole.host,
                icon: Icon(Icons.add_link),
                label: Text('Crear sala'),
              ),
              ButtonSegment(
                value: ConnectionRole.join,
                icon: Icon(Icons.login),
                label: Text('Unirse'),
              ),
            ],
            selected: {_role},
            onSelectionChanged: (s) => setState(() => _role = s.first),
          ),
          const SizedBox(height: 16),
          SectionCard(
            title: hosting ? 'Servidor central' : 'Relay y sala',
            subtitle: hosting
                ? 'La sala se registra en el relay; comparte el código con los peers'
                : 'Resuelve el código de sala y conéctate al relay',
            child: Column(
              children: [
                LabeledField(
                  label: 'Host del relay',
                  controller: _host,
                  hint: 'relay.ejemplo.com o 192.168.1.10',
                  keyboardType: TextInputType.url,
                ),
                Row(
                  children: [
                    Expanded(
                      child: LabeledField(
                        label: 'Puerto UDP',
                        controller: _udpPort,
                        keyboardType: TextInputType.number,
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: LabeledField(
                        label: 'Puerto HTTP (rooms)',
                        controller: _obsPort,
                        keyboardType: TextInputType.number,
                      ),
                    ),
                  ],
                ),
                if (!hosting)
                  LabeledField(
                    label: 'Código de sala',
                    controller: _roomCode,
                    hint: 'GRVT-2847',
                    suffix: _canScanQr
                        ? IconButton(
                            icon: const Icon(Icons.qr_code_scanner),
                            tooltip: 'Escanear QR',
                            onPressed: () => _scanQr(context),
                          )
                        : null,
                  ),
                if (hosting)
                  _HostNote(scheme: scheme),
              ],
            ),
          ),
          if (c.error != null) ...[
            const SizedBox(height: 12),
            _ErrorBanner(message: c.error!),
          ],
          if (hosting && c.roomCode != null && c.roomCode!.isNotEmpty) ...[
            const SizedBox(height: 16),
            SectionCard(
              title: 'Comparte este código',
              subtitle: hosting
                  ? 'Los peers se unen con el código o escaneando el QR'
                  : null,
              child: QrPanel(
                code: c.roomCode!,
                caption: 'Room code Gravital Talk',
              ),
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
                : Icon(hosting ? Icons.podcasts : Icons.login),
            label: Text(
              c.busy
                  ? 'Conectando…'
                  : hosting
                      ? 'Crear sala y esperar peers'
                      : 'Unirse a la sala',
            ),
          ),
          const SizedBox(height: 8),
          OutlinedButton(
            onPressed: c.busy
                ? null
                : () async {
                    _persist(c);
                    final msg = await c.testRelay();
                    if (!context.mounted) return;
                    toastification.show(
                      context: context,
                      type: msg.contains('OK')
                          ? ToastificationType.success
                          : ToastificationType.warning,
                      title: Text(msg),
                      autoCloseDuration: const Duration(seconds: 3),
                    );
                  },
            child: const Text('Probar relay (healthz)'),
          ),
        ],
      ),
    );
  }

  /// Escáner de QR real (mobile_scanner) que rellena el código de sala.
  Future<void> _scanQr(BuildContext ctx) async {
    final code = await showModalBottomSheet<String>(
      context: ctx,
      isScrollControlled: true,
      builder: (sheetCtx) => SizedBox(
        height: 420,
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.all(12),
              child: Text('Escanea el QR de la sala',
                  style: Theme.of(sheetCtx).textTheme.titleMedium),
            ),
            Expanded(
              child: MobileScanner(
                controller: MobileScannerController(
                  formats: const [BarcodeFormat.qrCode],
                ),
                onDetect: (capture) {
                  final raw =
                      capture.barcodes.isEmpty ? null : capture.barcodes.first.rawValue;
                  if (raw != null && raw.trim().isNotEmpty) {
                    Navigator.of(sheetCtx).pop(raw.trim());
                  }
                },
              ),
            ),
          ],
        ),
      ),
    );
    if (!ctx.mounted || code == null) return;
    setState(() => _roomCode.text = code.toUpperCase());
    toastification.show(
      context: ctx,
      type: ToastificationType.success,
      title: const Text('Sala escaneada'),
      description: Text(code),
      autoCloseDuration: const Duration(seconds: 2),
    );
  }

  bool get _canScanQr =>
      !kIsWeb &&
      (defaultTargetPlatform == TargetPlatform.android ||
          defaultTargetPlatform == TargetPlatform.iOS ||
          defaultTargetPlatform == TargetPlatform.macOS);
}

class _HostNote extends StatelessWidget {
  const _HostNote({required this.scheme});

  final ColorScheme scheme;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.all(10),
      decoration: BoxDecoration(
        color: scheme.primary.withValues(alpha: 0.08),
        borderRadius: BorderRadius.circular(10),
      ),
      child: Row(
        children: [
          Icon(Icons.info_outline, size: 16, color: scheme.primary),
          const SizedBox(width: 8),
          Expanded(
            child: Text(
              'Al crear la sala se genera un session_id compartido que el relay '
              'usa para enrutar el handshake y el audio cifrado.',
              style: Theme.of(context)
                  .textTheme
                  .bodySmall
                  ?.copyWith(color: scheme.onSurfaceVariant),
            ),
          ),
        ],
      ),
    );
  }
}

class _ErrorBanner extends StatelessWidget {
  const _ErrorBanner({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Container(
      padding: const EdgeInsets.all(12),
      decoration: BoxDecoration(
        color: scheme.errorContainer,
        borderRadius: BorderRadius.circular(12),
      ),
      child: Row(
        children: [
          Icon(Icons.error_outline, color: scheme.onErrorContainer, size: 18),
          const SizedBox(width: 8),
          Expanded(
            child: Text(message,
                style: TextStyle(color: scheme.onErrorContainer)),
          ),
        ],
      ),
    );
  }
}
