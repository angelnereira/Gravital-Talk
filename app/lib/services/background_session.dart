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
  static bool _standby = false;
  static bool _inSession = false;

  /// Mantiene la app despierta sin sala, para recibir llamadas de contactos.
  static Future<void> setStandby(bool on) async {
    _standby = on;
    if (!_supported || _inSession) return;
    try {
      // Sin permiso de notificaciones la llamada no se ve con la app oculta.
      if (on && await Permission.notification.isDenied) {
        await Permission.notification.request();
      }
      await _channel.invokeMethod<bool>(on ? 'start' : 'stop', {
        'mode': 'standby',
      });
    } on PlatformException {
      // Sin servicio: las llamadas sólo llegan con la app abierta.
    } on MissingPluginException {
      // Idem.
    }
  }

  /// Notificación de "te llaman" para cuando la app no está a la vista.
  static Future<void> notifyCall(String name) => _plain('notifyCall', name);

  static Future<void> cancelCall() => _plain('cancelCall', '');

  /// Arranca o para el tono y la vibración de llamada entrante.
  static Future<void> ring(bool on) => _invoke('ring', {'on': on});

  /// Aviso corriente del sistema, para cuando la app no está a la vista.
  static Future<void> notifyInfo(String title, String text) =>
      _invoke('notifyInfo', {'title': title, 'text': text});

  /// Contestar (`true`) o rechazar desde la notificación de la llamada.
  static void Function(bool accept)? onCallAction;

  static Future<void> _invoke(String method, Map<String, Object> args) async {
    if (!_supported) return;
    try {
      await _channel.invokeMethod<bool>(method, args);
    } on PlatformException {
      // Sin aviso: lo mismo se ve al abrir la app.
    } on MissingPluginException {
      // Idem.
    }
  }

  static Future<void> _plain(String method, String name) async {
    if (!_supported) return;
    try {
      await _channel.invokeMethod<bool>(method, {'name': name});
    } on PlatformException {
      // Sin notificación: la solicitud se ve al abrir la app.
    } on MissingPluginException {
      // Idem.
    }
  }

  static String _room = '';

  /// Refleja en la notificación si se está hablando, escuchando o en pausa.
  static Future<void> update({required bool talking, required bool paused}) async {
    if (!_supported || !_inSession) return;
    try {
      await _channel.invokeMethod<bool>('start', {
        'room': _room,
        'status': paused ? 'paused' : (talking ? 'talking' : 'listening'),
      });
    } on PlatformException {
      // La notificación se queda con el estado anterior.
    } on MissingPluginException {
      // Idem.
    }
  }

  static Future<void> start(String room) async {
    _inSession = true;
    _room = room;
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
    _inSession = false;
    if (!_supported) return;
    try {
      // Con las llamadas en segundo plano activas, el servicio no se para:
      // vuelve a la espera.
      if (_standby) {
        await _channel.invokeMethod<bool>('start', {'mode': 'standby'});
        return;
      }
      await _channel.invokeMethod<bool>('stop');
    } on PlatformException {
      // Ya estaba parado.
    } on MissingPluginException {
      // Idem.
    }
  }

  /// Avisa cuando el usuario pulsa "Salir de la sala" en la notificación.
  static void onLeave(void Function() leave, {void Function()? togglePtt}) {
    if (!_supported) return;
    _channel.setMethodCallHandler((call) async {
      if (call.method == 'leave') leave();
      if (call.method == 'togglePtt') togglePtt?.call();
      if (call.method == 'answerCall') onCallAction?.call(true);
      if (call.method == 'declineCall') onCallAction?.call(false);
    });
  }
}
