import 'dart:async';
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

  /// Puerto de la forma web de los enlaces (`http://localhost:47801/…`).
  static const localPort = 47801;

  static const _localPath = '/$pairingScheme/';

  static final _linkPattern = RegExp(
    '(?:$pairingScheme://|https?://(?:localhost|127\\.0\\.0\\.1)(?::\\d+)?'
    '$_localPath)\\S+',
  );

  /// Forma de `uri` (`gravital-talk://…`) para enviar por mensaje.
  ///
  /// Las apps de mensajería no hacen pulsable un esquema propio, pero sí un
  /// `http://`. Apunta al propio dispositivo de quien lo abre: Android se lo
  /// entrega a la app (que declara esos enlaces como suyos), y donde el
  /// sistema lo mande al navegador contesta [serveLocal]. No hay ninguna web
  /// de por medio. En iOS no hay equivalente: allí se envía el esquema.
  static String shareable(String uri) {
    const prefix = '$pairingScheme://';
    if (!uri.startsWith(prefix) || (!kIsWeb && Platform.isIOS)) return uri;
    return 'http://localhost:$localPort$_localPath'
        '${uri.substring(prefix.length)}';
  }

  /// Primer enlace de Gravital Talk que haya en `text`, en su forma
  /// `gravital-talk://…`, o `null`. Sirve para lo abierto, copiado o
  /// compartido, que puede venir envuelto en un mensaje y en cualquiera de
  /// las dos formas.
  static String? extract(String text) {
    final found = _linkPattern.firstMatch(text)?.group(0);
    if (found == null || found.startsWith('$pairingScheme://')) return found;
    final at = found.indexOf(_localPath);
    return '$pairingScheme://${found.substring(at + _localPath.length)}';
  }

  static HttpServer? _local;

  /// Contesta en este dispositivo a los enlaces `http://localhost:…` que el
  /// sistema haya mandado al navegador en vez de a la app.
  ///
  /// En Android redirige al esquema propio, que es lo que trae la app al
  /// frente. En escritorio el enlace se atiende aquí mismo. Sólo funciona
  /// con la app en marcha; si el puerto está ocupado se sigue sin ello.
  static Future<void> serveLocal() async {
    if (kIsWeb || _local != null) return;
    if (!Platform.isAndroid && !Platform.isLinux) return;
    try {
      final server =
          await HttpServer.bind(InternetAddress.loopbackIPv4, localPort);
      _local = server;
      server.listen(_answerLocal, onError: (Object _) {});
    } on Object {
      // Otra copia de la app, u otro programa, tiene el puerto.
    }
  }

  static Future<void> _answerLocal(HttpRequest request) async {
    final response = request.response;
    try {
      final link = request.uri.path.startsWith(_localPath)
          ? extract('http://localhost${request.uri}')
          : null;
      if (link == null) {
        response.statusCode = HttpStatus.notFound;
      } else if (Platform.isAndroid) {
        response.statusCode = HttpStatus.found;
        response.headers.set(HttpHeaders.locationHeader, link);
      } else {
        _onLink?.call(link);
        response.headers.contentType = ContentType.html;
        response.write(
          '<!doctype html><meta charset="utf-8"><title>Gravital Talk</title>'
          '<p style="font:16px sans-serif;margin:3em">Abierto en Gravital '
          'Talk. Puedes cerrar esta pestaña.</p>',
        );
      }
    } finally {
      await response.close();
    }
  }

  /// Escritorio: deja que el sistema abra los enlaces con esta app y que una
  /// segunda apertura le pase el enlace a la que ya está en marcha.
  ///
  /// Devuelve `true` si el enlace se entregó a otra instancia: entonces ésta
  /// no tiene nada que hacer y debe cerrarse.
  static Future<bool> desktopHandoff(List<String> args) async {
    if (kIsWeb || !Platform.isLinux) return false;
    final link = args.map(extract).nonNulls.firstOrNull;
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
    final fromArgs = args.map(extract).nonNulls.firstOrNull;
    if (fromArgs != null) return fromArgs;
    if (kIsWeb || !Platform.isAndroid) return null;
    try {
      final raw = await _channel.invokeMethod<String>('getInitialLink');
      return raw == null ? null : extract(raw);
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
      if (call.method != 'onLink' || link is! String) return;
      final found = extract(link);
      if (found != null) onLink(found);
    });
  }
}
