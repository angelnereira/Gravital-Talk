import 'package:shared_preferences/shared_preferences.dart';

import '../core/constants.dart';
import '../models/connection.dart';

/// Persistencia de ajustes y perfiles de conexión.
class SettingsStore {
  Future<SharedPreferences> get _prefs => SharedPreferences.getInstance();

  Future<AppSettings> loadSettings() async {
    final p = await _prefs;
    return AppSettings(
      sampleRate: p.getInt(PrefKeys.sampleRate) ?? 48000,
      channels: p.getInt(PrefKeys.channels) ?? 1,
      frameDurationMs: p.getInt(PrefKeys.frameDurationMs) ?? 20,
      jitterBufferMs: p.getInt(PrefKeys.jitterBufferMs) ?? 60,
      mtu: p.getInt(PrefKeys.mtu) ?? 1200,
      maxBitrate: p.getInt(PrefKeys.maxBitrate) ?? 64000,
      codec: AudioCodec.values.firstWhere(
        (c) => c.name == p.getString(PrefKeys.codec),
        orElse: () => AudioCodec.opus,
      ),
      themeMode: AppThemeMode.values.firstWhere(
        (t) => t.name == p.getString(PrefKeys.themeMode),
        orElse: () => AppThemeMode.system,
      ),
    );
  }

  Future<void> saveSettings(AppSettings s) async {
    final p = await _prefs;
    await p.setInt(PrefKeys.sampleRate, s.sampleRate);
    await p.setInt(PrefKeys.channels, s.channels);
    await p.setInt(PrefKeys.frameDurationMs, s.frameDurationMs);
    await p.setInt(PrefKeys.jitterBufferMs, s.jitterBufferMs);
    await p.setInt(PrefKeys.mtu, s.mtu);
    await p.setInt(PrefKeys.maxBitrate, s.maxBitrate);
    await p.setString(PrefKeys.codec, s.codec.name);
    await p.setString(PrefKeys.themeMode, s.themeMode.name);
  }

  Future<ServerProfile> loadServerProfile() async {
    final p = await _prefs;
    return ServerProfile(
      host: p.getString(PrefKeys.serverHost) ?? '',
      udpPort: p.getInt(PrefKeys.serverUdpPort) ?? DefaultPorts.udp,
      observabilityPort:
          p.getInt(PrefKeys.serverObsPort) ?? DefaultPorts.observability,
      roomCode: p.getString(PrefKeys.lastRoomCode) ?? '',
    );
  }

  Future<void> saveServerProfile(ServerProfile s) async {
    final p = await _prefs;
    await p.setString(PrefKeys.serverHost, s.host);
    await p.setInt(PrefKeys.serverUdpPort, s.udpPort);
    await p.setInt(PrefKeys.serverObsPort, s.observabilityPort);
    await p.setString(PrefKeys.lastRoomCode, s.roomCode);
  }

  Future<P2pProfile> loadP2pProfile() async {
    final p = await _prefs;
    return P2pProfile(
      host: p.getString(PrefKeys.p2pHost) ?? '',
      port: p.getInt(PrefKeys.p2pPort) ?? DefaultPorts.udp,
      localPort: p.getInt(PrefKeys.p2pLocalPort) ?? 0,
    );
  }

  Future<void> saveP2pProfile(P2pProfile s) async {
    final p = await _prefs;
    await p.setString(PrefKeys.p2pHost, s.host);
    await p.setInt(PrefKeys.p2pPort, s.port);
    await p.setInt(PrefKeys.p2pLocalPort, s.localPort);
  }

  Future<String?> loadNativeLibPath() async {
    final p = await _prefs;
    return p.getString(PrefKeys.nativeLibPath);
  }
}
