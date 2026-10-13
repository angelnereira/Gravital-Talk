import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../core/glass.dart';
import '../core/routes.dart';
import '../core/states.dart';
import '../core/tokens.dart';
import '../screens/contact_screen.dart';
import '../screens/invite_screen.dart';
import '../screens/pairing_screen.dart';
import '../services/contacts.dart';

/// Lista de contactos de la home: un toque para volver a hablar con alguien.
class ContactsSection extends StatelessWidget {
  const ContactsSection({super.key, required this.enabled, this.onPaste});

  /// Abre la invitación que haya en el portapapeles.
  final VoidCallback? onPaste;

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
            IconButton(
              tooltip: 'Abrir invitación copiada',
              onPressed: onPaste,
              icon: const Icon(Icons.content_paste_go),
            ),
            TextButton.icon(
              onPressed: () => Navigator.push(
                context,
                ForwardRoute(builder: (_) => const InviteScreen()),
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
          DialogActions(
            cancelLabel: 'Cancelar',
            onCancel: () => Navigator.pop(dialog),
            confirmLabel: 'Guardar',
            onConfirm: () => Navigator.pop(dialog, ctl.text),
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
            child: InkWell(
              borderRadius: BorderRadius.circular(Radii.sm),
              onTap: () => Navigator.push(
                context,
                ForwardRoute(
                  builder: (_) => ContactScreen(contactId: contact.id),
                ),
              ),
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
                      : contact.muted
                          ? 'Silenciado'
                          : _lastSeen(contact.lastSeen),
                  style: theme.textTheme.bodySmall
                      ?.copyWith(color: glass.secondary),
                ),
              ],
              ),
            ),
          ),
          PopupMenuButton<String>(
            tooltip: 'Más',
            icon: Icon(Icons.more_vert, color: glass.secondary),
            onSelected: (action) {
              if (action == 'details') {
                Navigator.push(
                  context,
                  ForwardRoute(
                    builder: (_) => ContactScreen(contactId: contact.id),
                  ),
                );
              } else if (action == 'rename') {
                _rename(context);
              } else {
                context.read<ContactsService>().remove(contact);
              }
            },
            itemBuilder: (_) => const [
              PopupMenuItem(value: 'details', child: Text('Ver ficha')),
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
