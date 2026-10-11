import 'dart:io';

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:shared_preferences/shared_preferences.dart';

/// Deja constancia de por qué se cerró la app la última vez.
///
/// Un cierre inesperado no deja rastro que el usuario pueda enseñar: la app
/// simplemente desaparece. Aquí se juntan las tres fuentes que sí lo saben
/// —el fallo que anota la librería nativa antes de morir, la excepción de la
/// parte Android y el motivo que guarda el propio sistema— en un texto que se
/// puede copiar desde Ajustes, sin cable ni herramientas.
class CrashReport {
  CrashReport._();

  static const _channel = MethodChannel('gravital_talk/diagnostics');
  static const _reportKey = 'diag.lastCrash';
  static const _seenExitKey = 'diag.lastExitSeen';
  static const _nativeFile = 'crash-native.txt';
  static const _platformFile = 'crash-java.txt';

  /// Informe del último cierre inesperado, o `null` si no hubo.
  static final ValueNotifier<String?> last = ValueNotifier(null);

  /// Recoge lo que dejó la ejecución anterior y devuelve el fichero donde la
  /// librería nativa debe anotar sus fallos (o `null` si no hay dónde).
  static Future<String?> init(SharedPreferences prefs) async {
    if (kIsWeb) return null;
    final dir = await _directory();
    final found = <String>[];
    if (dir != null) {
      for (final name in [_nativeFile, _platformFile]) {
        final text = _take(File('$dir/$name'));
        if (text != null) found.add(text);
      }
    }
    final exit = await _lastExit();
    if (exit != null) {
      final at = exit['timestamp'];
      if (at is int && at > (prefs.getInt(_seenExitKey) ?? 0)) {
        await prefs.setInt(_seenExitKey, at);
        final text = exit['text'];
        if (text is String) found.add(text);
      }
    }
    if (found.isNotEmpty) {
      await prefs.setString(_reportKey, found.join('\n'));
    }
    last.value = prefs.getString(_reportKey);
    return dir == null ? null : '$dir/$_nativeFile';
  }

  /// Olvida el informe una vez visto.
  static Future<void> clear() async {
    last.value = null;
    final prefs = await SharedPreferences.getInstance();
    await prefs.remove(_reportKey);
  }

  /// Lee y borra: cada fallo se cuenta una sola vez.
  static String? _take(File file) {
    try {
      if (!file.existsSync()) return null;
      final text = file.readAsStringSync().trim();
      file.deleteSync();
      if (text.isEmpty) return null;
      // Un fallo en bucle no debe llenar el informe.
      return text.length > 4000 ? text.substring(text.length - 4000) : text;
    } on FileSystemException {
      return null;
    }
  }

  static Future<String?> _directory() async {
    try {
      if (Platform.isAndroid) {
        return await _channel.invokeMethod<String>('filesDir');
      }
      final env = Platform.environment;
      final state = env['XDG_STATE_HOME'];
      final home = env['HOME'];
      final base = state != null && state.isNotEmpty
          ? state
          : (home == null ? null : '$home/.local/state');
      if (base == null) return null;
      final dir = Directory('$base/gravital-talk')..createSync(recursive: true);
      return dir.path;
    } on PlatformException {
      return null;
    } on MissingPluginException {
      return null;
    } on FileSystemException {
      return null;
    }
  }

  static Future<Map<Object?, Object?>?> _lastExit() async {
    if (!Platform.isAndroid) return null;
    try {
      return await _channel.invokeMethod<Map<Object?, Object?>>('lastExit');
    } on PlatformException {
      return null;
    } on MissingPluginException {
      return null;
    }
  }
}
