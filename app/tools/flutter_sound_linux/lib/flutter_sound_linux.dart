/// Implementación Linux de `flutter_sound`.
///
/// ## Por qué existe
///
/// `flutter_sound` no publica binarios para Linux, y Flutter necesita que todo
/// plugin declarado en `pubspec.yaml` tenga implementación nativa en cada
/// plataforma sobre la que se compila. Sin este paquete, `flutter build linux`
/// falla en CMake con `No target "flutter_sound_plugin"`, aunque el código
/// Dart nunca llegue a usarlo.
///
/// El audio en escritorio no pasa por aquí: lo lleva la librería nativa
/// (`gs_audio_*`, con cpal), que captura y reproduce sin plugins. Esta clase
/// **no implementa nada**: se registra como plataforma para que el build
/// enlace, y nadie la llama.
library;

import 'package:flutter_sound_platform_interface/flutter_sound_player_platform_interface.dart';
import 'package:flutter_sound_platform_interface/method_channel_flutter_sound_player.dart';

/// Plugin de reproducción para Linux: se registra y deja que los métodos
/// fallen en el method channel, que no tiene implementación nativa.
class FlutterSoundLinuxPlugin extends MethodChannelFlutterSoundPlayer {
  /// Registra esta implementación como la plataforma activa en Linux.
  static void registerWith() {
    FlutterSoundPlayerPlatform.instance = FlutterSoundLinuxPlugin();
  }
}

/// Alias con el nombre que exige el generador de plugins de Flutter
/// (`dartPluginClass` en el pubspec). Es la misma clase.
typedef FlutterSoundLinux = FlutterSoundLinuxPlugin;
