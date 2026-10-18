import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../core/routes.dart';
import '../services/contacts.dart';
import '../services/launch_links.dart';
import '../services/share.dart';
import '../ui/patterns.dart';
import '../ui/semantic_colors.dart';
import '../ui/tokens.dart';
import 'pairing_screen.dart';

/// Invitar: la única pantalla que hace falta para empezar a hablar con
/// alguien nuevo.
///
/// Enseña un enlace (y su QR). Quien lo abra con la app queda guardado como
/// contacto y, si esta pantalla sigue abierta, la conversación arranca sola:
/// no hay código de sala que dictar ni respuesta que escanear.
class InviteScreen extends StatefulWidget {
  const InviteScreen({super.key});

  @override
  State<InviteScreen> createState() => _InviteScreenState();
}

class _InviteScreenState extends State<InviteScreen> {
  late final ContactsService _contacts;
  String? _link;
  bool _copied = false;

  /// `true` si se salió hacia la conversación: la espera sigue viva para
  /// que quien abra el enlace después se sume a ella.
  bool _handedOff = false;

  @override
  void initState() {
    super.initState();
    _contacts = context.read<ContactsService>();
    _contacts.autoCall.addListener(_onAnswered);
    _contacts.contactLink().then((link) {
      if (!mounted) return;
      if (link != null) _contacts.startInviting(link);
      setState(() => _link = link);
    });
  }

  void _onAnswered() {
    final contact = _contacts.autoCall.value;
    if (contact == null || !mounted) return;
    _contacts.autoCall.value = null;
    _handedOff = true;
    Navigator.of(context).pushReplacement(ForwardRoute(
      builder: (_) => PairingScreen(mode: PairingMode.host, callee: contact),
    ));
  }

  @override
  void dispose() {
    _contacts.autoCall.removeListener(_onAnswered);
    if (!_handedOff) _contacts.stopInviting();
    super.dispose();
  }

  String _message(String link) =>
      'Hablemos por Gravital Talk. Abre este enlace con la app:\n'
      '${LaunchLinks.shareable(link)}';

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;
    // Estar esperando a que abran la invitación es una llamada en curso, no un
    // dato: el indicador lo dice con el tono semántico, no con el color de
    // marca, que en esta pantalla ya está en el botón principal.
    final semantic = SemanticColors.of(context);
    final link = _link;

    return Scaffold(
      appBar: AppBar(title: const Text('Invitar')),
      body: SafeArea(
        child: Center(
          child: SingleChildScrollView(
            padding: const EdgeInsets.all(NSpace.xl),
            child: ConstrainedBox(
              constraints: const BoxConstraints(maxWidth: 420),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  Card(
                    color: scheme.surfaceContainerLow,
                    shape: NShape.rounded(NShape.medium),
                    child: Padding(
                      padding: const EdgeInsets.all(NSpace.xl),
                      child: Column(
                        children: [
                          Text(
                            'Envía este enlace',
                            textAlign: TextAlign.center,
                            style: theme.textTheme.titleLarge?.copyWith(
                              fontWeight: FontWeight.w800,
                              color: scheme.onSurface,
                            ),
                          ),
                          const SizedBox(height: NSpace.xs),
                          Text(
                            'O que escaneen el código. Al abrirlo con Gravital '
                            'Talk empezáis a hablar y quedáis guardados como '
                            'contactos.',
                            textAlign: TextAlign.center,
                            style: theme.textTheme.bodyMedium
                                ?.copyWith(color: scheme.onSurfaceVariant),
                          ),
                          const SizedBox(height: NSpace.xl),
                          if (link == null)
                            Padding(
                              padding: const EdgeInsets.all(NSpace.xl),
                              child: UILoading.centered(
                                message: 'Preparando tu invitación…',
                              ),
                            )
                          else
                            Semantics(
                              label: 'Código QR de tu invitación',
                              child: Container(
                                padding: const EdgeInsets.all(NSpace.md),
                                decoration: BoxDecoration(
                                  // El QR es la excepción a la paleta: los
                                  // lectores necesitan el fondo blanco y el
                                  // contraste negro del módulo.
                                  color: Colors.white,
                                  borderRadius:
                                      BorderRadius.circular(NShape.extraLarge),
                                ),
                                child: QrImageView(
                                  data: link,
                                  size: 232,
                                  backgroundColor: Colors.white,
                                ),
                              ),
                            ),
                          const SizedBox(height: NSpace.lg),
                          Row(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: [
                              SizedBox(
                                width: 16,
                                height: 16,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                  color: semantic.calling.color,
                                ),
                              ),
                              const SizedBox(width: NSpace.md),
                              Flexible(
                                child: Text(
                                  'Esperando a que lo abran…',
                                  style: theme.textTheme.bodyMedium
                                      ?.copyWith(color: scheme.onSurfaceVariant),
                                ),
                              ),
                            ],
                          ),
                        ],
                      ),
                    ),
                  ),
                  if (link != null) ...[
                    const SizedBox(height: NSpace.lg),
                    FilledButton.icon(
                      onPressed: () async {
                        final shared = await shareText(_message(link));
                        if (!shared && mounted) setState(() => _copied = true);
                      },
                      icon: const Icon(Icons.ios_share),
                      label: const Text('Compartir invitación'),
                    ),
                    const SizedBox(height: NSpace.md),
                    OutlinedButton.icon(
                      onPressed: () async {
                        await Clipboard.setData(
                          ClipboardData(text: LaunchLinks.shareable(link)),
                        );
                        if (mounted) setState(() => _copied = true);
                      },
                      icon: Icon(_copied ? Icons.check : Icons.link),
                      label: Text(_copied ? 'Enlace copiado' : 'Copiar enlace'),
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