/// Modelos de configuración de la app.
library;

/// Modo de conexión soportado por el protocolo.
enum ConnectionMode {
  /// Sala a través de un servidor central (relay). Requiere room code.
  server,

  /// Conexión directa peer-to-peer (UDP, sin intermediarios).
  p2p,
}

/// Rol dentro de una conexión.
enum ConnectionRole {
  /// Espera/acepta la conexión (host). En modo servidor crea la sala.
  host,

  /// Inicia la conexión (cliente). En modo servidor se une a la sala.
  join,
}

/// Codec de audio negociable.
enum AudioCodec {
  pcm(0x01, 'PCM (sin compresión)'),
  opus(0x02, 'Opus 64 kbps');

  const AudioCodec(this.code, this.label);

  final int code;
  final String label;
}

/// Modo de tema de la interfaz.
enum AppThemeMode {
  system('Sistema'),
  light('Claro'),
  dark('Oscuro');

  const AppThemeMode(this.label);

  final String label;
}

/// Ajustes de audio/protocolo persistidos.
class AppSettings {
  const AppSettings({
    this.sampleRate = 48000,
    this.channels = 1,
    this.frameDurationMs = 20,
    this.jitterBufferMs = 60,
    this.mtu = 1200,
    this.maxBitrate = 64000,
    this.codec = AudioCodec.opus,
    this.themeMode = AppThemeMode.system,
  });

  final int sampleRate;
  final int channels;
  final int frameDurationMs;
  final int jitterBufferMs;
  final int mtu;
  final int maxBitrate;
  final AudioCodec codec;
  final AppThemeMode themeMode;

  AppSettings copyWith({
    int? sampleRate,
    int? channels,
    int? frameDurationMs,
    int? jitterBufferMs,
    int? mtu,
    int? maxBitrate,
    AudioCodec? codec,
    AppThemeMode? themeMode,
  }) {
    return AppSettings(
      sampleRate: sampleRate ?? this.sampleRate,
      channels: channels ?? this.channels,
      frameDurationMs: frameDurationMs ?? this.frameDurationMs,
      jitterBufferMs: jitterBufferMs ?? this.jitterBufferMs,
      mtu: mtu ?? this.mtu,
      maxBitrate: maxBitrate ?? this.maxBitrate,
      codec: codec ?? this.codec,
      themeMode: themeMode ?? this.themeMode,
    );
  }

  /// Muestras por frame calculadas (interleaved).
  int get samplesPerFrame =>
      (sampleRate * frameDurationMs ~/ 1000) * channels;
}

/// Perfil de conexión a un servidor (relay).
class ServerProfile {
  const ServerProfile({
    this.host = '',
    this.udpPort = 9000,
    this.observabilityPort = 9100,
    this.roomCode = '',
  });

  final String host;
  final int udpPort;
  final int observabilityPort;
  final String roomCode;

  ServerProfile copyWith({
    String? host,
    int? udpPort,
    int? observabilityPort,
    String? roomCode,
  }) {
    return ServerProfile(
      host: host ?? this.host,
      udpPort: udpPort ?? this.udpPort,
      observabilityPort: observabilityPort ?? this.observabilityPort,
      roomCode: roomCode ?? this.roomCode,
    );
  }
}

/// Perfil de conexión P2P directa.
class P2pProfile {
  const P2pProfile({
    this.host = '',
    this.port = 9000,
    this.localPort = 0,
  });

  final String host;
  final int port;

  /// Puerto UDP local de escucha (`0` = efímero).
  final int localPort;

  P2pProfile copyWith({String? host, int? port, int? localPort}) {
    return P2pProfile(
      host: host ?? this.host,
      port: port ?? this.port,
      localPort: localPort ?? this.localPort,
    );
  }
}
