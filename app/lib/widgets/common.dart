/// Widgets propios que el sistema GEUI todavía no cubre.
///
/// Todo lo que ya existe como componente del sistema vive en
/// `lib/ui/components/` y se importa desde ahí: [StatusBadge], [SectionCard],
/// [MetricTile], [ModeCard], [QrPanel], [EventLogView], [PttButton],
/// [LevelMeter], [ReconnectingBanner] y [ParticipantsCard] tienen su versión
/// GEUI, con sus mismos nombres y parámetros.
library;

import 'package:flutter/material.dart';

import '../ui/tokens.dart';

/// Campo de texto con etiqueta y estilo uniforme.
///
/// La forma y el relleno del campo no se declaran aquí: los saca
/// `NTheme.inputDecorationTheme`, que ya los fija en tokens del sistema
/// (`NShape.extraSmall` para campos de texto, § 6.1 de
/// `docs/gravital-experience-ui.md`). Repetirlos sería la forma más rápida de
/// que un campo se salga del sistema.
class LabeledField extends StatelessWidget {
  const LabeledField({
    super.key,
    required this.label,
    required this.controller,
    this.hint,
    this.keyboardType,
    this.obscure = false,
    this.suffix,
    this.validator,
    this.errorText,
    this.onChanged,
  });

  final String label;
  final TextEditingController controller;
  final String? hint;
  final TextInputType? keyboardType;
  final bool obscure;
  final Widget? suffix;
  final String? Function(String?)? validator;

  /// Mensaje de error del campo, para mostrarlo en línea.
  final String? errorText;

  /// Se invoca al cambiar el texto, para poder limpiar el error anterior.
  final ValueChanged<String>? onChanged;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: NSpace.md),
      child: TextFormField(
        controller: controller,
        keyboardType: keyboardType,
        obscureText: obscure,
        validator: validator,
        onChanged: onChanged,
        autovalidateMode: AutovalidateMode.disabled,
        decoration: InputDecoration(
          labelText: label,
          hintText: hint,
          suffix: suffix,
          errorText: errorText,
          errorMaxLines: 2,
        ),
      ),
    );
  }
}