import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Comparte un texto por lo que el sistema ofrezca (mensajería, correo…).
///
/// En Android abre el panel de compartir. Donde no lo hay (escritorio) lo
/// copia al portapapeles y devuelve `false`, para que quien llama diga
/// "copiado" en vez de "compartido".
Future<bool> shareText(String text) async {
  if (!kIsWeb && Platform.isAndroid) {
    try {
      await const MethodChannel('gravital_talk/links')
          .invokeMethod<bool>('share', {'text': text});
      return true;
    } on PlatformException {
      // Sin panel de compartir: al portapapeles.
    } on MissingPluginException {
      // Idem.
    }
  }
  await Clipboard.setData(ClipboardData(text: text));
  return false;
}
