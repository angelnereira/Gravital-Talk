import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:permission_handler/permission_handler.dart';

/// Mantiene la sala viva con la app en segundo plano.
///
/// En Android eso exige un servicio en primer plano con su notificación: sin
/// él, el sistema corta el micrófono y congela la app al poco de salir de
/// ella. En escritorio no hace falta nada: la ventana minimizada sigue
/// funcionando.
class BackgroundSession {
  BackgroundSession._();

  static const _channel = MethodChannel('gravital_talk/background');

  static bool get _supported => !kIsWeb && Platform.isAndroid;

  /// Empieza a proteger la sala `room`. Que falle no impide conversar: sólo
  /// se pierde la escucha con la app oculta.
  static Future<void> start(String room) async {
    if (!_supported) return;
    try {
      // La notificación es desde donde se vuelve o se sale; sin permiso el
      // servicio funciona igual, sólo que no se ve.
      if (await Permission.notification.isDenied) {
        await Permission.notification.request();
      }
      await _channel.invokeMethod<bool>('start', {'room': room});
    } on PlatformException {
      // Sin servicio: la sala sigue mientras la app esté a la vista.
    } on MissingPluginException {
      // Idem.
    }
  }

  static Future<void> stop() async {
    if (!_supported) return;
    try {
      await _channel.invokeMethod<bool>('stop');
    } on PlatformException {
      // Ya estaba parado.
    } on MissingPluginException {
      // Idem.
    }
  }

  /// Avisa cuando el usuario pulsa "Salir de la sala" en la notificación.
  static void onLeave(void Function() leave) {
    if (!_supported) return;
    _channel.setMethodCallHandler((call) async {
      if (call.method == 'leave') leave();
    });
  }
}
