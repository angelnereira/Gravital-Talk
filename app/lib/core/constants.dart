/// Constantes globales de la app Gravital Talk.
library;

/// Versión mostrada en la UI (independiente de la versión nativa).
const appVersion = '0.3.0-alpha.1';

/// Versión del ABI C soportada por esta app.
const supportedAbiVersion = 1;

/// Puertos por defecto del ecosistema Gravital Talk.
class DefaultPorts {
  static const udp = 9000;
  static const ws = 9090;
  static const observability = 9100;

  const DefaultPorts._();
}

/// Límites y valores de audio aceptados por el protocolo.
class AudioLimits {
  static const sampleRates = <int>[8000, 12000, 16000, 24000, 48000];
  static const frameDurationsMs = <int>[10, 20, 40, 60];
  static const channels = <int>[1, 2];
  static const jitterBufferMs = <int>[20, 40, 60, 100, 200, 500];
  static const mtus = <int>[576, 1200, 1400];

  const AudioLimits._();
}

/// Claves de persistencia (`shared_preferences`).
class PrefKeys {
  static const serverHost = 'server.host';
  static const serverUdpPort = 'server.udpPort';
  static const serverObsPort = 'server.obsPort';
  static const lastRoomCode = 'server.lastRoomCode';
  static const serverToken = 'server.token';
  static const p2pHost = 'p2p.host';
  static const p2pPort = 'p2p.port';
  static const p2pLocalPort = 'p2p.localPort';
  static const sampleRate = 'audio.sampleRate';
  static const channels = 'audio.channels';
  static const frameDurationMs = 'audio.frameDurationMs';
  static const jitterBufferMs = 'audio.jitterBufferMs';
  static const mtu = 'audio.mtu';
  static const maxBitrate = 'audio.maxBitrate';
  static const codec = 'audio.codec';
  static const themeMode = 'ui.themeMode';
  static const nativeLibPath = 'advanced.nativeLibPath';

  const PrefKeys._();
}
