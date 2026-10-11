import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../core/glass.dart';
import '../core/routes.dart';
import '../core/tokens.dart';
import '../screens/pairing_screen.dart';
import '../services/contacts.dart';
import '../services/share.dart';

/// Lista de contactos de la home: un toque para volver a hablar con alguien.
class ContactsSection extends StatelessWidget {
  const ContactsSection({super.key, required this.enabled});

  /// `false` mientras hay una sala en curso: no se puede llamar a otro.
  final bool enabled;

  @override
  Widget build(BuildContext context) {
    final contacts = context.watch<ContactsService>();
    if (!contacts.available) return const SizedBox.shrink();
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                'Contactos',
                style: theme.textTheme.titleMedium?.copyWith(
                  fontWeight: FontWeight.w800,
                  color: glass.primary,
                ),
              ),
            ),
            TextButton.icon(
              onPressed: () => showModalBottomSheet<void>(
                context: context,
                showDragHandle: true,
                isScrollControlled: true,
                builder: (_) => const _AddContactSheet(),
              ),
              icon: const Icon(Icons.person_add_alt_1),
              label: const Text('Añadir'),
            ),
          ],
        ),
        const SizedBox(height: Spacing.sm),
        if (contacts.contacts.isEmpty)
          Text(
            'Cuando hables con alguien se guardará aquí, para volver a '
            'llamarle con un toque. También puedes añadir a alguien '
            'enviándole tu enlace.',
            style:
                theme.textTheme.bodyMedium?.copyWith(color: glass.secondary),
          )
        else
          GlassSurface(
            padding: const EdgeInsets.symmetric(
                horizontal: Spacing.lg, vertical: Spacing.xs),
            child: Column(
              children: [
                for (final contact in contacts.contacts)
                  _ContactRow(contact: contact, enabled: enabled),
              ],
            ),
          ),
      ],
    );
  }
}

class _ContactRow extends StatelessWidget {
  const _ContactRow({required this.contact, required this.enabled});

  final Contact contact;
  final bool enabled;

  void _call(BuildContext context) {
    Navigator.push(
      context,
      ForwardRoute(
        builder: (_) =>
            PairingScreen(mode: PairingMode.host, callee: contact),
      ),
    );
  }

  Future<void> _rename(BuildContext context) async {
    final service = context.read<ContactsService>();
    final ctl = TextEditingController(text: contact.name);
    final name = await showDialog<String>(
      context: context,
      builder: (dialog) => AlertDialog(
        title: const Text('Nombre del contacto'),
        content: TextField(
          controller: ctl,
          autofocus: true,
          maxLength: 40,
          textCapitalization: TextCapitalization.words,
          onSubmitted: (v) => Navigator.pop(dialog, v),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.pop(dialog),
            child: const Text('Cancelar'),
          ),
          FilledButton(
            onPressed: () => Navigator.pop(dialog, ctl.text),
            child: const Text('Guardar'),
          ),
        ],
      ),
    );
    if (name != null) await service.rename(contact, name);
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    // Creado al enseñar el enlace de contacto: todavía no lo ha abierto nadie.
    final pending =
        contact.device.isEmpty && contact.name == ContactsService.pendingName;
    final initial =
        contact.name.isEmpty ? '?' : contact.name.characters.first.toUpperCase();

    return Padding(
      padding: const EdgeInsets.symmetric(vertical: Spacing.sm),
      child: Row(
        children: [
          CircleAvatar(
            radius: 20,
            backgroundColor: theme.colorScheme.primary.withValues(alpha: 0.18),
            child: Text(
              initial,
              style: TextStyle(
                color: glass.primary,
                fontWeight: FontWeight.w800,
              ),
            ),
          ),
          const SizedBox(width: Spacing.md),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  contact.name,
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: theme.textTheme.bodyLarge?.copyWith(
                    fontWeight: FontWeight.w600,
                    color: glass.primary,
                  ),
                ),
                Text(
                  pending
                      ? 'Enlace enviado, aún sin abrir'
                      : _lastSeen(contact.lastSeen),
                  style: theme.textTheme.bodySmall
                      ?.copyWith(color: glass.secondary),
                ),
              ],
            ),
          ),
          PopupMenuButton<String>(
            tooltip: 'Más',
            icon: Icon(Icons.more_vert, color: glass.secondary),
            onSelected: (action) {
              if (action == 'rename') {
                _rename(context);
              } else {
                context.read<ContactsService>().remove(contact);
              }
            },
            itemBuilder: (_) => const [
              PopupMenuItem(value: 'rename', child: Text('Cambiar nombre')),
              PopupMenuItem(value: 'remove', child: Text('Eliminar')),
            ],
          ),
          IconButton.filled(
            tooltip: 'Llamar a ${contact.name}',
            onPressed: enabled && !pending ? () => _call(context) : null,
            icon: const Icon(Icons.call),
          ),
        ],
      ),
    );
  }

  static String _lastSeen(int ms) {
    if (ms == 0) return 'Sin conversaciones todavía';
    final ago = DateTime.now().difference(DateTime.fromMillisecondsSinceEpoch(ms));
    if (ago.inMinutes < 2) return 'Hablasteis hace un momento';
    if (ago.inHours < 1) return 'Hablasteis hace ${ago.inMinutes} min';
    if (ago.inDays < 1) return 'Hablasteis hace ${ago.inHours} h';
    if (ago.inDays == 1) return 'Hablasteis ayer';
    return 'Hablasteis hace ${ago.inDays} días';
  }
}

/// Añadir un contacto sin haber hablado antes: se le pasa un enlace (o lo
/// escanea) y los dos quedan guardados; o se pega aquí el que haya enviado él.
class _AddContactSheet extends StatefulWidget {
  const _AddContactSheet();

  @override
  State<_AddContactSheet> createState() => _AddContactSheetState();
}

class _AddContactSheetState extends State<_AddContactSheet> {
  final _paste = TextEditingController();
  String? _link;
  String? _status;

  @override
  void initState() {
    super.initState();
    context.read<ContactsService>().contactLink().then((link) {
      if (mounted) setState(() => _link = link);
    });
  }

  @override
  void dispose() {
    _paste.dispose();
    super.dispose();
  }

  String _message(String link) =>
      'Añádeme en Gravital Talk para hablar con un toque:\n$link';

  Future<void> _add() async {
    final contacts = context.read<ContactsService>();
    final navigator = Navigator.of(context);
    final contact = await contacts.addFromLink(_paste.text);
    if (!mounted) return;
    if (contact == null) {
      setState(() => _status = 'Ese enlace de contacto no es válido.');
    } else {
      navigator.pop();
    }
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final link = _link;
    return SafeArea(
      child: SingleChildScrollView(
        padding: EdgeInsets.fromLTRB(Spacing.xl, 0, Spacing.xl,
            Spacing.xl + MediaQuery.viewInsetsOf(context).bottom),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Text(
              'Añadir contacto',
              textAlign: TextAlign.center,
              style: theme.textTheme.titleLarge
                  ?.copyWith(fontWeight: FontWeight.w800),
            ),
            const SizedBox(height: Spacing.xs),
            Text(
              'Envíale tu enlace o que escanee el código con «Unirse». Al '
              'abrirlo con Gravital Talk quedáis guardados el uno al otro.',
              textAlign: TextAlign.center,
              style: theme.textTheme.bodyMedium,
            ),
            const SizedBox(height: Spacing.lg),
            if (link == null)
              const Center(child: CircularProgressIndicator())
            else ...[
              Center(
                child: Semantics(
                  label: 'Código QR de tu enlace de contacto',
                  child: Container(
                    padding: const EdgeInsets.all(Spacing.md),
                    decoration: BoxDecoration(
                      color: Colors.white,
                      borderRadius: BorderRadius.circular(Radii.glass),
                    ),
                    child: QrImageView(
                      data: link,
                      size: 200,
                      backgroundColor: Colors.white,
                    ),
                  ),
                ),
              ),
              const SizedBox(height: Spacing.lg),
              Row(
                children: [
                  Expanded(
                    child: FilledButton.tonalIcon(
                      onPressed: () async {
                        final shared = await shareText(_message(link));
                        if (!shared && mounted) {
                          setState(() => _status = 'Enlace copiado.');
                        }
                      },
                      icon: const Icon(Icons.ios_share),
                      label: const Text('Compartir'),
                    ),
                  ),
                  const SizedBox(width: Spacing.md),
                  Expanded(
                    child: OutlinedButton.icon(
                      onPressed: () async {
                        await Clipboard.setData(ClipboardData(text: link));
                        if (mounted) setState(() => _status = 'Enlace copiado.');
                      },
                      icon: const Icon(Icons.link),
                      label: const Text('Copiar'),
                    ),
                  ),
                ],
              ),
            ],
            const SizedBox(height: Spacing.xl),
            TextField(
              controller: _paste,
              decoration: InputDecoration(
                labelText: '¿Te han enviado el suyo? Pégalo aquí',
                suffixIcon: IconButton(
                  tooltip: 'Añadir',
                  onPressed: _add,
                  icon: const Icon(Icons.check),
                ),
              ),
              onSubmitted: (_) => _add(),
            ),
            if (_status != null) ...[
              const SizedBox(height: Spacing.sm),
              Text(_status!, textAlign: TextAlign.center),
            ],
          ],
        ),
      ),
    );
  }
}
