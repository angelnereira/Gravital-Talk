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

/// Cómo se trata y se comprime la voz: cada perfil es un equilibrio distinto
/// entre naturalidad, limpieza y datos. Los ajustes de cada uno viven en la
/// librería nativa (`VoiceProfile` del códec); aquí sólo se elige.
enum VoiceProfile {
  clear(
    1,
    'Clara',
    32000,
    'La voz más natural. Unos 0,55 MB por minuto hablado.',
  ),
  call(
    2,
    'Llamada',
    16000,
    'Limpia y estable, con menos datos. Unos 0,45 MB por minuto.',
  ),
  noisy(
    3,
    'Ruido',
    24000,
    'Para la calle o un sitio ruidoso: quita más fondo, a costa de algo de '
        'naturalidad. Unos 0,5 MB por minuto.',
  ),
  light(
    4,
    'Ligero',
    12000,
    'El mínimo de datos, para coberturas malas. No transmite los silencios. '
        'Unos 0,4 MB por minuto.',
  );

  const VoiceProfile(this.code, this.label, this.bitrateCap, this.description);

  /// Código que entiende la librería nativa (`GS_VOICE_*`).
  final int code;
  final String label;

  /// Tope de la tasa de bits del perfil.
  final int bitrateCap;
  final String description;
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
    this.codec = AudioCodec.opus,
    this.themeMode = AppThemeMode.system,
    this.rendezvous = '',
    this.voiceProfile = VoiceProfile.clear,
    this.turnTones = true,
    this.nativeAudio = false,
    this.pttBubble = true,
    this.headsetPtt = true,
    this.volumePtt = false,
  });

  /// Valores que usa la app cuando nadie ha configurado nada.
  ///
  /// Existen porque los ajustes dejaron de exponer estos campos: la negociación
  /// los decide en el handshake. Queda la constante para poder mostrarlos como
  /// información, que es distinto de dejarlos configurables.
  static const defaults = AppSettings();

  final int sampleRate;
  final int channels;
  final int frameDurationMs;
  final int jitterBufferMs;
  final int mtu;
  final AudioCodec codec;
  final AppThemeMode themeMode;

  /// Servidor de encuentro (`host:puerto` de un `gs-relay`), o vacío.
  ///
  /// Es lo que permite conectar entre redes distintas: presenta a los dos
  /// extremos para que perforen su NAT. No lleva audio. Vacío = las salas sólo
  /// funcionan dentro de la misma red (o con el anfitrión alcanzable desde
  /// fuera).
  final String rendezvous;

  /// Tratamiento y compresión de la voz.
  final VoiceProfile voiceProfile;

  /// Tonos cortos al empezar y terminar un turno de palabra.
  final bool turnTones;

  /// Android: captar y reproducir dentro de la librería nativa en vez de con
  /// los plugins. Menos latencia; experimental hasta probarlo en más equipos.
  final bool nativeAudio;

  /// Android: burbuja flotante para hablar con la app oculta.
  final bool pttBubble;

  /// Android: el botón del auricular (cable o Bluetooth) habla durante la
  /// sala.
  final bool headsetPtt;

  /// Android: los botones de volumen hablan con la app a la vista. Apagado
  /// por defecto: mientras está activo, el volumen se cambia desde el
  /// sistema.
  final bool volumePtt;

  AppSettings copyWith({
    int? sampleRate,
    int? channels,
    int? frameDurationMs,
    int? jitterBufferMs,
    int? mtu,
    AudioCodec? codec,
    AppThemeMode? themeMode,
    String? rendezvous,
    VoiceProfile? voiceProfile,
    bool? turnTones,
    bool? nativeAudio,
    bool? pttBubble,
    bool? headsetPtt,
    bool? volumePtt,
  }) {
    return AppSettings(
      sampleRate: sampleRate ?? this.sampleRate,
      channels: channels ?? this.channels,
      frameDurationMs: frameDurationMs ?? this.frameDurationMs,
      jitterBufferMs: jitterBufferMs ?? this.jitterBufferMs,
      mtu: mtu ?? this.mtu,
      codec: codec ?? this.codec,
      themeMode: themeMode ?? this.themeMode,
      rendezvous: rendezvous ?? this.rendezvous,
      voiceProfile: voiceProfile ?? this.voiceProfile,
      turnTones: turnTones ?? this.turnTones,
      nativeAudio: nativeAudio ?? this.nativeAudio,
      pttBubble: pttBubble ?? this.pttBubble,
      headsetPtt: headsetPtt ?? this.headsetPtt,
      volumePtt: volumePtt ?? this.volumePtt,
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
    this.token = '',
  });

  final String host;
  final int udpPort;
  final int observabilityPort;
  final String roomCode;

  /// Token de sala (PSK de Noise). Vacío = sala abierta.
  final String token;

  ServerProfile copyWith({
    String? host,
    int? udpPort,
    int? observabilityPort,
    String? roomCode,
    String? token,
  }) {
    return ServerProfile(
      host: host ?? this.host,
      udpPort: udpPort ?? this.udpPort,
      observabilityPort: observabilityPort ?? this.observabilityPort,
      roomCode: roomCode ?? this.roomCode,
      token: token ?? this.token,
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
