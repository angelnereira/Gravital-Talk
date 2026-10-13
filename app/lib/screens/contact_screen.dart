import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:provider/provider.dart';

import '../core/glass.dart';
import '../core/routes.dart';
import '../core/states.dart';
import '../core/tokens.dart';
import '../services/contacts.dart';
import '../services/session_controller.dart';
import 'pairing_screen.dart';

/// Ficha de un contacto: lo que se sabe de él y lo que el usuario decide
/// sobre sus llamadas. Nada de lo que se edita aquí sale del dispositivo.
class ContactScreen extends StatefulWidget {
  const ContactScreen({super.key, required this.contactId});

  final String contactId;

  @override
  State<ContactScreen> createState() => _ContactScreenState();
}

class _ContactScreenState extends State<ContactScreen> {
  late final ContactsService _contacts = context.read<ContactsService>();
  late final TextEditingController _name;
  late final TextEditingController _note;

  @override
  void initState() {
    super.initState();
    final contact = _contacts.byId(widget.contactId);
    _name = TextEditingController(text: contact?.name ?? '');
    _note = TextEditingController(text: contact?.note ?? '');
  }

  @override
  void dispose() {
    _name.dispose();
    _note.dispose();
    super.dispose();
  }

  Future<void> _saveName() async {
    final contact = _contacts.byId(widget.contactId);
    final name = _name.text.trim();
    if (contact == null || name.isEmpty || name == contact.name) return;
    await _contacts.rename(contact, name);
  }

  Future<void> _saveNote() =>
      _contacts.edit(widget.contactId, note: _note.text);

  Future<void> _remove(Contact contact) async {
    final sure = await showDialog<bool>(
      context: context,
      builder: (dialog) => AlertDialog(
        title: Text('¿Eliminar a ${contact.name}?'),
        content: const Text(
          'Dejaréis de poder llamaros. Para volver a hablar habrá que '
          'compartir una invitación nueva.',
        ),
        actions: [
          DialogActions(
            cancelLabel: 'Cancelar',
            onCancel: () => Navigator.pop(dialog, false),
            confirmLabel: 'Eliminar',
            onConfirm: () => Navigator.pop(dialog, true),
            destructive: true,
          ),
        ],
      ),
    );
    if (sure != true || !mounted) return;
    final navigator = Navigator.of(context);
    await _contacts.remove(contact);
    navigator.pop();
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    final contact = context.watch<ContactsService>().byId(widget.contactId);
    final busy = context.select<SessionController, bool>(
      (c) => c.busy || c.isLive,
    );
    if (contact == null) {
      return Scaffold(
        appBar: AppBar(title: const Text('Contacto')),
        body: const Center(child: Text('Este contacto ya no existe.')),
      );
    }
    final pending =
        contact.device.isEmpty && contact.name == ContactsService.pendingName;
    final initial = contact.name.isEmpty
        ? '?'
        : contact.name.characters.first.toUpperCase();

    return Scaffold(
      appBar: AppBar(title: const Text('Contacto')),
      body: SafeArea(
        child: ListView(
          padding: const EdgeInsets.all(Spacing.xl),
          children: [
            Center(
              child: CircleAvatar(
                radius: 40,
                backgroundColor:
                    theme.colorScheme.primary.withValues(alpha: 0.18),
                child: Text(
                  initial,
                  style: theme.textTheme.headlineMedium?.copyWith(
                    color: glass.primary,
                    fontWeight: FontWeight.w800,
                  ),
                ),
              ),
            ),
            const SizedBox(height: Spacing.lg),
            GlassSurface(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  TextField(
                    controller: _name,
                    maxLength: 40,
                    textCapitalization: TextCapitalization.words,
                    decoration: const InputDecoration(
                      labelText: 'Nombre',
                      counterText: '',
                    ),
                    onSubmitted: (_) => _saveName(),
                    onTapOutside: (_) {
                      FocusManager.instance.primaryFocus?.unfocus();
                      _saveName();
                    },
                  ),
                  const SizedBox(height: Spacing.md),
                  TextField(
                    controller: _note,
                    maxLength: 280,
                    minLines: 1,
                    maxLines: 4,
                    textCapitalization: TextCapitalization.sentences,
                    decoration: const InputDecoration(
                      labelText: 'Nota',
                      hintText: 'Quién es, de dónde le conoces…',
                    ),
                    onTapOutside: (_) {
                      FocusManager.instance.primaryFocus?.unfocus();
                      _saveNote();
                    },
                  ),
                ],
              ),
            ),
            const SizedBox(height: Spacing.lg),
            GlassSurface(
              padding: const EdgeInsets.symmetric(
                horizontal: Spacing.lg,
                vertical: Spacing.xs,
              ),
              child: Column(
                children: [
                  SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    title: const Text('Contestar sin preguntar'),
                    subtitle: const Text(
                      'Cuando llame, entras directamente a escucharle, como '
                      'un walkie-talkie.',
                    ),
                    value: contact.autoAnswer,
                    onChanged: contact.muted
                        ? null
                        : (on) => _contacts.edit(contact.id, autoAnswer: on),
                  ),
                  SwitchListTile(
                    contentPadding: EdgeInsets.zero,
                    title: const Text('Silenciar sus llamadas'),
                    subtitle: const Text(
                      'No suena ni avisa cuando llama. Tú puedes seguir '
                      'llamándole.',
                    ),
                    value: contact.muted,
                    onChanged: (on) => _contacts.edit(contact.id, muted: on),
                  ),
                ],
              ),
            ),
            const SizedBox(height: Spacing.lg),
            GlassSurface(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  _Fact('Estado',
                      pending ? 'Invitación enviada, aún sin abrir' : 'Añadido'),
                  _Fact('Última vez', _when(contact.lastSeen)),
                  if (contact.lastLocal != null)
                    _Fact('En tu red', contact.lastLocal!),
                  if (contact.lastPublic != null)
                    _Fact('Desde fuera', contact.lastPublic!),
                  _Fact(
                    'Código de seguridad',
                    _groups(contact.id),
                    hint: 'Debe coincidir con el que él ve en tu ficha.',
                    copy: true,
                  ),
                ],
              ),
            ),
            const SizedBox(height: Spacing.xl),
            FilledButton.icon(
              onPressed: busy || pending
                  ? null
                  : () => Navigator.pushReplacement(
                        context,
                        ForwardRoute(
                          builder: (_) => PairingScreen(
                            mode: PairingMode.host,
                            callee: contact,
                          ),
                        ),
                      ),
              icon: const Icon(Icons.call),
              label: Text('Llamar a ${contact.name}'),
            ),
            const SizedBox(height: Spacing.md),
            OutlinedButton.icon(
              onPressed: () => _remove(contact),
              icon: const Icon(Icons.delete_outline),
              label: const Text('Eliminar contacto'),
            ),
          ],
        ),
      ),
    );
  }

  static String _groups(String id) {
    final up = id.toUpperCase();
    return [
      for (var i = 0; i < up.length; i += 4)
        up.substring(i, i + 4 > up.length ? up.length : i + 4),
    ].join(' ');
  }

  static String _when(int ms) {
    if (ms == 0) return 'Aún no habéis hablado';
    final t = DateTime.fromMillisecondsSinceEpoch(ms);
    String two(int v) => v.toString().padLeft(2, '0');
    return '${two(t.day)}/${two(t.month)}/${t.year} · '
        '${two(t.hour)}:${two(t.minute)}';
  }
}

class _Fact extends StatelessWidget {
  const _Fact(this.label, this.value, {this.hint, this.copy = false});

  final String label;
  final String value;
  final String? hint;
  final bool copy;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final glass = GlassTextTheme.of(context);
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: Spacing.sm),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  label,
                  style: theme.textTheme.bodySmall
                      ?.copyWith(color: glass.secondary),
                ),
                Text(
                  value,
                  style: theme.textTheme.bodyLarge
                      ?.copyWith(color: glass.primary),
                ),
                if (hint != null)
                  Text(
                    hint!,
                    style: theme.textTheme.bodySmall
                        ?.copyWith(color: glass.secondary),
                  ),
              ],
            ),
          ),
          if (copy)
            IconButton(
              tooltip: 'Copiar',
              icon: Icon(Icons.copy, size: 18, color: glass.secondary),
              onPressed: () => Clipboard.setData(ClipboardData(text: value)),
            ),
        ],
      ),
    );
  }
}
