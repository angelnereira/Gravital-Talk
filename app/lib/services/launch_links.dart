import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import 'pairing_uri.dart';

/// Invitaciones que llegan desde fuera de la app.
///
/// Una invitación es un enlace `gravital-talk://pair?…`: el mismo contenido
/// que el QR. Abrirlo (desde un chat, un correo, otra app) debe llevar directo
/// a la sala, sin pasar por el escáner ni por teclear nada.
///
/// - **Android**: el enlace llega como intent a `MainActivity`, que lo pasa
///   por este canal.
/// - **Escritorio**: llega como argumento de la línea de órdenes, que es como
///   el sistema lanza al manejador de un esquema de URI.
class LaunchLinks {
  LaunchLinks._();

  static const _channel = MethodChannel('gravital_talk/links');

  static void Function(String link)? _onLink;

  /// Primer enlace `gravital-talk://…` que haya en `text`, o `null`. Sirve
  /// para lo copiado o compartido, que suele venir envuelto en un mensaje.
  static String? extract(String text) =>
      RegExp('$pairingScheme://\\S+').firstMatch(text)?.group(0);

  /// Escritorio: deja que el sistema abra los enlaces con esta app y que una
  /// segunda apertura le pase el enlace a la que ya está en marcha.
  ///
  /// Devuelve `true` si el enlace se entregó a otra instancia: entonces ésta
  /// no tiene nada que hacer y debe cerrarse.
  static Future<bool> desktopHandoff(List<String> args) async {
    if (kIsWeb || !Platform.isLinux) return false;
    final link = args.where((a) => a.startsWith('$pairingScheme://')).firstOrNull;
    final env = Platform.environment;
    // Ligado a los datos del usuario: dos copias con datos distintos (las
    // pruebas) no se pisan.
    final base = env['XDG_DATA_HOME'] ?? '${env['HOME']}/.local/share';
    final path = '$base/gravital-talk.sock';
    final address = InternetAddress(path, type: InternetAddressType.unix);
    if (link != null) {
      try {
        final socket = await Socket.connect(address, 0,
            timeout: const Duration(milliseconds: 500));
        socket.write(link);
        await socket.flush();
        await socket.close();
        return true;
      } on Object {
        // Nadie escuchando: esta instancia es la primera.
      }
    }
    try {
      final stale = File(path);
      if (stale.existsSync()) stale.deleteSync();
      final server = await ServerSocket.bind(address, 0);
      server.listen((client) async {
        final text = await client
            .map(String.fromCharCodes)
            .join()
            .timeout(const Duration(seconds: 2), onTimeout: () => '');
        final received = extract(text);
        if (received != null) _onLink?.call(received);
      }, onError: (Object _) {});
    } on Object {
      // Sin socket (ruta demasiado larga, permisos): se sigue sin instancia
      // única, como antes.
    }
    _registerScheme(base);
    return false;
  }

  /// Registra esta app como la que abre `gravital-talk://` en el escritorio.
  static void _registerScheme(String dataHome) {
    try {
      final exe = Platform.resolvedExecutable;
      final file = File('$dataHome/applications/gravital-talk.desktop');
      final content = '[Desktop Entry]\n'
          'Type=Application\n'
          'Name=Gravital Talk\n'
          'Exec="$exe" %u\n'
          'Terminal=false\n'
          'Categories=Network;AudioVideo;\n'
          'MimeType=x-scheme-handler/$pairingScheme;\n';
      if (file.existsSync() && file.readAsStringSync() == content) return;
      file.parent.createSync(recursive: true);
      file.writeAsStringSync(content);
      Process.run('xdg-mime', [
        'default',
        'gravital-talk.desktop',
        'x-scheme-handler/$pairingScheme',
      ]).ignore();
    } on Object {
      // Escritorio sin xdg: los enlaces se seguirán pegando a mano.
    }
  }

  /// Enlace con el que se abrió la app, o `null`.
  static Future<String?> initial(List<String> args) async {
    for (final arg in args) {
      if (arg.startsWith('$pairingScheme://')) return arg;
    }
    if (kIsWeb || !Platform.isAndroid) return null;
    try {
      return await _channel.invokeMethod<String>('getInitialLink');
    } on PlatformException {
      return null;
    } on MissingPluginException {
      return null;
    }
  }

  /// Avisa de los enlaces que lleguen con la app ya abierta.
  static void listen(void Function(String link) onLink) {
    _onLink = onLink;
    if (kIsWeb || !Platform.isAndroid) return;
    _channel.setMethodCallHandler((call) async {
      final link = call.arguments;
      if (call.method == 'onLink' && link is String) onLink(link);
    });
  }
}
