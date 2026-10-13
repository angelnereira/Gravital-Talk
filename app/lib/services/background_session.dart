import 'dart:async';
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

  // ── Burbuja para hablar ──────────────────────────────────────────────────

  static bool _bubble = true;
  static bool _talking = false;
  static bool _paused = false;
  static bool _peerTalking = false;
  static String? _caller;

  /// Activa o quita la burbuja flotante (Ajustes). Sale sólo con la app
  /// oculta, con una sala en curso o una llamada entrante, y si el sistema
  /// deja mostrarla encima de otras apps.
  static Future<void> setBubbleEnabled(bool on) async {
    if (_bubble == on) return;
    _bubble = on;
    await _syncBubble();
  }

  /// `true` si Android deja dibujar la burbuja encima de otras apps.
  static Future<bool> bubbleAllowed() async {
    if (!_supported) return false;
    try {
      return await Permission.systemAlertWindow.isGranted;
    } on Object {
      return false;
    }
  }

  /// Lleva a la pantalla del sistema donde se concede el permiso.
  static Future<bool> requestBubble() async {
    if (!_supported) return false;
    try {
      return (await Permission.systemAlertWindow.request()).isGranted;
    } on Object {
      return false;
    }
  }

  /// El otro toma o suelta la palabra: la burbuja late mientras habla.
  static Future<void> peerTalking(bool on) async {
    if (_peerTalking == on) return;
    _peerTalking = on;
    await _syncBubble();
  }

  /// Llamada entrante de `caller` esperando respuesta, o `null` si ya no.
  static Future<void> ringing(String? caller) async {
    if (_caller == caller) return;
    _caller = caller;
    await _syncBubble();
  }

  static Future<void> _syncBubble() => _invoke('bubble', bubbleState());

  /// Lo que se le manda a la burbuja: modo (`off`, `session`, `ringing`) y
  /// estado de la sala.
  @visibleForTesting
  static Map<String, Object> bubbleState() => {
        'mode': _inSession
            ? 'session'
            : (_caller != null ? 'ringing' : 'off'),
        'talking': _talking,
        'paused': _paused,
        'peerTalking': _peerTalking,
        'caller': _caller ?? '',
        'enabled': _bubble,
      };

  /// Manda la app al fondo, de vuelta a lo que se estaba haciendo.
  static Future<void> moveToBack() => _invoke('moveToBack', {});

  /// Refleja en la notificación si se está hablando, escuchando o en pausa.
  static Future<void> update({required bool talking, required bool paused}) async {
    _talking = talking;
    _paused = paused;
    if (!_supported || !_inSession) return;
    unawaited(_syncBubble());
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
    _talking = false;
    _paused = false;
    _peerTalking = false;
    _caller = null;
    if (!_supported) return;
    unawaited(_syncBubble());
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
    _talking = false;
    _paused = false;
    _peerTalking = false;
    if (!_supported) return;
    unawaited(_syncBubble());
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

  /// Contestar desde la burbuja: como "Contestar", pero al conectar la app
  /// vuelve al fondo y se sigue hablando desde la burbuja.
  static void Function()? onBubbleAnswer;

  /// Avisa cuando el usuario pulsa "Salir de la sala" en la notificación, el
  /// botón de hablar de la notificación (`togglePtt`) o mantiene pulsada la
  /// burbuja (`ptt`, con `true` al pulsar y `false` al soltar).
  static void onLeave(
    void Function() leave, {
    void Function()? togglePtt,
    void Function(bool down)? ptt,
  }) {
    if (!_supported) return;
    _channel.setMethodCallHandler((call) async {
      switch (call.method) {
        case 'leave':
          leave();
        case 'togglePtt':
          togglePtt?.call();
        case 'pttDown':
          ptt?.call(true);
        case 'pttUp':
          ptt?.call(false);
        case 'answerCall':
          onCallAction?.call(true);
        case 'answerFromBubble':
          onBubbleAnswer?.call();
        case 'declineCall':
          onCallAction?.call(false);
      }
    });
  }
}
