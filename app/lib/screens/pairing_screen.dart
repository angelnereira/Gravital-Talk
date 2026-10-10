import 'package:flutter/material.dart';
import 'package:mobile_scanner/mobile_scanner.dart';
import 'package:provider/provider.dart';

import '../core/states.dart';
import '../core/tokens.dart';
import '../models/session.dart';
import '../services/pairing_uri.dart';
import '../screens/session_screen.dart';
import '../services/session_controller.dart';
import '../widgets/common.dart';

/// Pantalla única de emparejamiento.
///
/// Sustituye a `ServerSetupScreen` y `P2pSetupScreen`, que entre las dos
/// pedían ocho campos de red antes de poder decir "holo". Aquí hay dos acciones
/// y ningún campo de red:
///
/// - **Crear sala**: bindea un puerto, publica el endpoint por STUN, genera el
///   código y espera. Si la red no puede recibir conexiones, lo dice *antes* de
///   que el invitado lo intente.
/// - **Unirse**: escanea el QR o teclea el código. Si el QR trae el endpoint
///   completo (formato nuevo), se rellena solo.
///
/// La topología ya no es una decisión del usuario: anfitrión escucha, invitado
/// conecta. Que cruce internet o se quede en la LAN es consecuencia.
class PairingScreen extends StatefulWidget {
  const PairingScreen({super.key});

  @override
  State<PairingScreen> createState() => _PairingScreenState();
}

class _PairingScreenState extends State<PairingScreen> {
  var _joining = false;

  /// Aviso de alcanzabilidad, o `null` si no hay nada que decir.
  ///
  /// Se consulta al crear la sala, no al entrar: el anfitrión necesita saber si
  /// los demás podrán alcanzarlo *antes* de compartir el QR. Si no, el fallo
  /// llega cuando el invitado ya ha escaneado.
  String? _reachability;

  /// Diagnostica la red y guarda el aviso que corresponda.
  Future<void> _checkReachability(SessionController c) async {
    try {
      final reach = await c.diagnoseReachability();
      if (!mounted) return;
      setState(() => _reachability = reach?.warning);
    } catch (_) {
      // Un fallo del diagnóstico nunca debe impedir crear la sala: es un aviso,
      // no un requisito.
      if (mounted) setState(() => _reachability = null);
    }
  }

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();

    return Scaffold(
      appBar: AppBar(
        title: const Text('Conectar'),
        actions: [
          if (c.isLive)
            Padding(
              padding: const EdgeInsets.only(right: Spacing.md),
              child: Center(child: StatusBadge(c.state)),
            ),
        ],
      ),
      body: SafeArea(
        child: ListView(
          padding: const EdgeInsets.all(Spacing.lg),
          children: [
            // El aviso de modo demo va primero: sin motor nativo nada de lo que
            // sigue funciona, y el usuario debe saberlo antes de pulsar.
            if (c.engineKind == EngineKind.demo) ...[
              const WarningBanner(
                message: 'Sin librería nativa: el audio es una simulación. '
                    'Compila libgravital_talk_ffi para hablar de verdad.',
              ),
              const SizedBox(height: Spacing.lg),
            ],
            if (c.error != null) ...[
              ErrorBanner(
                message: c.error!,
                onRetry: c.busy ? null : () => _join(c),
                onDismiss: c.clearError,
              ),
              const SizedBox(height: Spacing.md),
            ],
            if (_joining) ...[
              LoadingStates.centered(message: 'Conectando…'),
            ] else if (c.isLive) ...[
              _connectedCard(c),
            ] else ...[
              _createCard(c),
              const SizedBox(height: Spacing.lg),
              _joinCard(c),
            ],
          ],
        ),
      ),
    );
  }

  // ── Crear ────────────────────────────────────────────────────────────────

  Widget _createCard(SessionController c) {
    return SectionCard(
      title: 'Crear sala',
      subtitle: 'Comparte el QR o el código. El otro se une solo.',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          FilledButton.icon(
            onPressed: c.busy
                ? null
                : () async {
                    await c.createRoom();
                    if (mounted) await _checkReachability(c);
                  },
            icon: const Icon(Icons.add_link),
            label: const Text('Crear sala y compartir'),
          ),
          if (c.roomCode != null && c.roomCode!.isNotEmpty) ...[
            const SizedBox(height: Spacing.lg),
            // El QR codifica el endpoint COMPLETO (IP pública + puerto), así
            // que escanearlo conecta sin teclear nada. Se muestra en cuanto la
            // sala existe, no tras conectar: el anfitrión necesita algo que
            // compartir desde el primer segundo.
            QrPanel(
              code: _pairingUri(c).toUri(),
              caption: _pairingUri(c).toHumanReadable(),
            ),
            // Aviso de alcanzabilidad ANTES de compartir: si este dispositivo
            // no puede recibir conexiones, el QR no servirá de nada y el
            // invitado se enteraría después de escanearlo.
            if (_reachability != null) ...[
              const SizedBox(height: Spacing.lg),
              WarningBanner(message: _reachability!),
            ],
            const SizedBox(height: Spacing.lg),
            // Y el código legible, para leerlo por teléfono.
            Text(
              c.roomCode!,
              textAlign: TextAlign.center,
              style: Theme.of(context).textTheme.headlineSmall?.copyWith(
                    fontWeight: FontWeight.w800,
                    letterSpacing: 3,
                  ),
            ),
            const SizedBox(height: Spacing.xs),
            Text(
              'Léelo en voz alta si el QR no funciona',
              textAlign: TextAlign.center,
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
            ),
          ],
        ],
      ),
    );
  }

  // ── Unirse ───────────────────────────────────────────────────────────────

  Widget _joinCard(SessionController c) {
    final code = c.server.roomCode;
    return SectionCard(
      title: 'Unirse',
      subtitle: 'Escanea el QR o escribe el código que te pasen.',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          LabeledField(
            label: 'Código de sala',
            controller: _codeController(c),
            hint: 'GRVT-2847',
          ),
          const SizedBox(height: Spacing.sm),
          FilledButton.icon(
            onPressed: c.busy ? null : () => _join(c),
            icon: const Icon(Icons.login),
            label: const Text('Unirse'),
          ),
          const SizedBox(height: Spacing.sm),
          OutlinedButton.icon(
            onPressed: c.busy ? null : () => _scanQr(context, c),
            icon: const Icon(Icons.qr_code_scanner),
            label: const Text('Escanear QR'),
          ),
          if (code.isNotEmpty) ...[
            const SizedBox(height: Spacing.md),
            Text(
              'El servidor se toma del QR cuando lo escaneas.',
              style: Theme.of(context).textTheme.labelSmall?.copyWith(
                    color: Theme.of(context).colorScheme.onSurfaceVariant,
                  ),
            ),
          ],
        ],
      ),
    );
  }

  /// Controlador del campo de código.
  ///
  /// Se crea una vez y se sincroniza con el perfil guardado: recrearlo en cada
  /// `build` perdería lo que el usuario está tecleando.
  TextEditingController? _codeCtl;
  TextEditingController _codeController(SessionController c) {
    return _codeCtl ??= TextEditingController(text: c.server.roomCode);
  }

  // ── Conectado ────────────────────────────────────────────────────────────

  Widget _connectedCard(SessionController c) {
    return SectionCard(
      title: 'Sesión activa',
      subtitle: c.roomCode != null ? 'Sala ${c.roomCode}' : 'P2P directo',
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          FilledButton.icon(
            onPressed: () => Navigator.of(context).push(
              MaterialPageRoute(builder: (_) => const SessionScreen()),
            ),
            icon: const Icon(Icons.mic),
            label: const Text('Hablar'),
          ),
          const SizedBox(height: Spacing.sm),
          OutlinedButton.icon(
            onPressed: c.busy ? null : () => _confirmClose(context, c),
            icon: const Icon(Icons.call_end, color: Colors.red),
            label: Text('Finalizar',
                style: TextStyle(color: Theme.of(context).colorScheme.error)),
          ),
        ],
      ),
    );
  }

  // ── Acciones ─────────────────────────────────────────────────────────────

  Future<void> _join(SessionController c) async {
    // El código se toma del campo; el perfil se actualiza antes de conectar
    // para que el controlador lo vea.
    final raw = _codeCtl?.text.trim() ?? '';
    if (raw.isEmpty) {
      c.reportError('Escribe el código de la sala.');
      return;
    }

    // Si lo que se pegó es un QR completo, se rellena todo desde él.
    final parsed = PairingUri.tryParse(raw);
    c.updateServerProfile(c.server.copyWith(
      roomCode: parsed?.room ?? raw.toUpperCase(),
    ));
    if (parsed != null && parsed.host.isNotEmpty) {
      c.updateServerProfile(c.server.copyWith(
        host: parsed.host,
        udpPort: parsed.udpPort ?? c.server.udpPort,
      ));
    }

    // El destino sale del QR si lo trae; si no, del escrito a mano. Se comprueba
    // que no esté vacío: `??` no captura la cadena vacía, y conectar a ""
    // llega al motor nativo como argumento inválido (NativeException -2).
    final scannedHost = parsed?.host.trim() ?? '';
    final endpoint = scannedHost.isNotEmpty ? scannedHost : c.server.host.trim();
    if (endpoint.isEmpty) {
      c.reportError(
        'El QR no lleva la dirección del anfitrión. Pídele que comparta el '
        'código o vuelve a generarlo.',
      );
      setState(() => _joining = false);
      return;
    }
    final port = parsed?.udpPort ?? c.server.udpPort;

    setState(() => _joining = true);
    final ok = await c.joinEndpoint(endpoint, port);
    if (!mounted) return;
    setState(() => _joining = false);
    if (ok && c.isLive) {
      Navigator.of(context).pushReplacement(
        MaterialPageRoute(builder: (_) => const SessionScreen()),
      );
    }
  }

  Future<void> _scanQr(BuildContext context, SessionController c) async {
    final raw = await showModalBottomSheet<String>(
      context: context,
      isScrollControlled: true,
      builder: (sheetCtx) => SizedBox(
        height: 420,
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.all(Spacing.md),
              child: Text('Escanea el QR de la sala',
                  style: Theme.of(sheetCtx).textTheme.titleMedium),
            ),
            Expanded(
              child: MobileScanner(
                controller: MobileScannerController(
                  formats: const [BarcodeFormat.qrCode],
                ),
                onDetect: (capture) {
                  final value =
                      capture.barcodes.isEmpty ? null : capture.barcodes.first.rawValue;
                  if (value != null && value.trim().isNotEmpty) {
                    Navigator.of(sheetCtx).pop(value.trim());
                  }
                },
              ),
            ),
          ],
        ),
      ),
    );
    if (raw == null || !mounted) return;

    final parsed = PairingUri.tryParse(raw);
    if (parsed == null) {
      c.reportError('Ese QR no es de Gravital Talk.');
      return;
    }
    // Un QR con el endpoint completo rellena host y código de una vez.
    c.updateServerProfile(c.server.copyWith(
      host: parsed.host.isNotEmpty ? parsed.host : c.server.host,
      roomCode: parsed.room,
      udpPort: parsed.udpPort ?? c.server.udpPort,
    ));
    _codeCtl?.text = parsed.room;
    setState(() {});
  }

  Future<void> _confirmClose(BuildContext context, SessionController c) async {
    final ok = await ConfirmDialog.show(
      context,
      title: 'Finalizar sesión',
      message: 'Se cortará el audio en curso. Podréis volver a emparejar.',
      confirmLabel: 'Finalizar',
      destructive: true,
    );
    if (!ok) return;
    await c.disconnect();
  }

  /// URI de emparejamiento con lo que el anfitrión debe compartir.
  ///
  /// El `host` sale de `publicEndpoint` —la IP pública que STUN vio para el
  /// anfitrión— y no de `c.server.host`, que es la dirección del relay y está
  /// vacía al crear una sala. Con el campo vacío el QR salía sin destino y el
  /// invitado fallaba con "argumento inválido" al intentar conectar a "".
  PairingUri _pairingUri(SessionController c) {
    final endpoint = c.publicEndpoint;
    final (host, port) = _splitEndpoint(endpoint, c.server.udpPort);
    return PairingUri(
      host: host,
      room: c.roomCode ?? '',
      udpPort: port,
      token: c.server.token,
    );
  }

  /// Parte un endpoint `ip:puerto` (o `[ipv6]:puerto`).
  ///
  /// Si el endpoint no trae puerto se devuelve el de por defecto.
  (String, int) _splitEndpoint(String endpoint, int fallbackPort) {
    final text = endpoint.trim();
    if (text.isEmpty) return ('', fallbackPort);

    // IPv6 va entre corchetes.
    if (text.startsWith('[')) {
      final end = text.indexOf(']');
      if (end > 0) {
        final host = text.substring(1, end);
        final port = int.tryParse(text.substring(end + 2)) ?? fallbackPort;
        return (host, port);
      }
    }

    final idx = text.lastIndexOf(':');
    if (idx <= 0 || idx == text.length - 1) return (text, fallbackPort);
    final port = int.tryParse(text.substring(idx + 1));
    if (port == null) return (text, fallbackPort);
    return (text.substring(0, idx), port);
  }
}