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
    if (kIsWeb || !Platform.isAndroid) return;
    _channel.setMethodCallHandler((call) async {
      final link = call.arguments;
      if (call.method == 'onLink' && link is String) onLink(link);
    });
  }
}
