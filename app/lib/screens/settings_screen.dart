import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:settings_ui/settings_ui.dart';
import 'package:toastification/toastification.dart';

import '../core/constants.dart';
import '../models/connection.dart';
import '../services/native_bridge.dart';
import '../services/session_controller.dart';

/// Ajustes de audio, protocolo, tema y diagnóstico (settings_ui).
class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  String? _stunResult;
  bool _testing = false;

  @override
  Widget build(BuildContext context) {
    final c = context.watch<SessionController>();
    final s = c.settings;
    final scheme = Theme.of(context).colorScheme;

    return Scaffold(
      appBar: AppBar(title: const Text('Ajustes')),
      body: SettingsList(
        lightTheme: SettingsThemeData(
          settingsListBackground: scheme.surface,
          settingsSectionBackground: scheme.surface,
        ),
        darkTheme: SettingsThemeData(
          settingsListBackground: scheme.surface,
          settingsSectionBackground: scheme.surface,
        ),
        sections: [
          SettingsSection(
            title: const Text('Audio y protocolo'),
            tiles: [
              _dropdownTile<int>(
                context,
                title: 'Sample rate',
                value: s.sampleRate,
                values: AudioLimits.sampleRates,
                label: (v) => '$v Hz',
                onChanged: (v) => c.updateSettings(s.copyWith(sampleRate: v)),
              ),
              _dropdownTile<int>(
                context,
                title: 'Canales',
                value: s.channels,
                values: AudioLimits.channels,
                label: (v) => v == 1 ? 'Mono' : 'Estéreo',
                onChanged: (v) => c.updateSettings(s.copyWith(channels: v)),
              ),
              _dropdownTile<int>(
                context,
                title: 'Frame',
                value: s.frameDurationMs,
                values: AudioLimits.frameDurationsMs,
                label: (v) => '$v ms',
                onChanged: (v) => c.updateSettings(s.copyWith(frameDurationMs: v)),
              ),
              _dropdownTile<int>(
                context,
                title: 'Jitter buffer',
                value: s.jitterBufferMs,
                values: AudioLimits.jitterBufferMs,
                label: (v) => '$v ms',
                onChanged: (v) => c.updateSettings(s.copyWith(jitterBufferMs: v)),
              ),
              _dropdownTile<int>(
                context,
                title: 'MTU',
                value: s.mtu,
                values: AudioLimits.mtus,
                label: (v) => '$v bytes',
                onChanged: (v) => c.updateSettings(s.copyWith(mtu: v)),
              ),
              _dropdownTile<int>(
                context,
                title: 'Bitrate máximo',
                value: s.maxBitrate,
                values: const [24000, 32000, 48000, 64000, 96000, 128000],
                label: (v) => '${(v / 1000).toStringAsFixed(0)} kbps',
                onChanged: (v) => c.updateSettings(s.copyWith(maxBitrate: v)),
              ),
              _dropdownTile<AudioCodec>(
                context,
                title: 'Codec',
                value: s.codec,
                values: AudioCodec.values,
                label: (v) => v.label,
                onChanged: (v) => c.updateSettings(s.copyWith(codec: v)),
              ),
              SettingsTile(
                title: const Text('Muestras por frame'),
                description: Text(
                    '${s.samplesPerFrame} muestras interleaved (${s.sampleRate} Hz · ${s.channels} ch)'),
                trailing: const Icon(Icons.straighten, size: 18),
                onPressed: null,
              ),
            ],
          ),
          SettingsSection(
            title: const Text('Interfaz'),
            tiles: [
              _dropdownTile<AppThemeMode>(
                context,
                title: 'Tema',
                value: s.themeMode,
                values: AppThemeMode.values,
                label: (v) => v.label,
                onChanged: (v) => c.updateSettings(s.copyWith(themeMode: v)),
              ),
            ],
          ),
          SettingsSection(
            title: const Text('Diagnóstico'),
            tiles: [
              SettingsTile(
                title: const Text('Motor'),
                description: Text('${c.engineKind.label} · ${c.engineDetail}'),
                trailing: const Icon(Icons.memory, size: 18),
                onPressed: null,
              ),
              SettingsTile(
                title: const Text('Ping FFI'),
                description: Text(_pingText()),
                trailing: const Icon(Icons.bolt, size: 18),
                onPressed: null,
              ),
              SettingsTile.navigation(
                title: const Text('Test relay + STUN'),
                description: Text(_stunResult == null
                    ? 'Comprueba el relay y descubre tu IP pública'
                    : 'IP pública: $_stunResult'),
                leading: const Icon(Icons.network_check),
                onPressed: _testing ? null : (_) => _runDiagnostics(c),
              ),
            ],
          ),
          SettingsSection(
            title: const Text('Acerca de'),
            tiles: [
              SettingsTile.navigation(
                title: const Text('Versión'),
                value: const Text(appVersion),
                leading: const Icon(Icons.info_outline),
                onPressed: (_) {},
              ),
            ],
          ),
        ],
      ),
    );
  }

  Future<void> _runDiagnostics(SessionController c) async {
    setState(() => _testing = true);
    final msg = await c.testRelay();
    final stun = await c.discoverPublicAddress();
    if (!mounted) return;
    setState(() {
      _testing = false;
      _stunResult = stun;
    });
    toastification.show(
      context: context,
      type: msg.contains('OK')
          ? ToastificationType.success
          : ToastificationType.warning,
      title: Text(msg),
      description: stun == null ? null : Text('IP pública: $stun'),
      autoCloseDuration: const Duration(seconds: 3),
    );
  }

  String _pingText() {
    try {
      final bridge = NativeBridge.tryLoad();
      if (bridge == null) return 'no disponible (modo demo)';
      return bridge.ping() ? 'OK' : 'falló';
    } catch (e) {
      return '$e';
    }
  }

  /// Tile con dropdown compacto como `trailing`.
  SettingsTile _dropdownTile<T extends Object>(
    BuildContext context, {
    required String title,
    required T value,
    required List<T> values,
    required String Function(T) label,
    required ValueChanged<T> onChanged,
  }) {
    return SettingsTile(
      title: Text(title),
      trailing: DropdownButtonHideUnderline(
        child: DropdownButton<T>(
          value: value,
          isDense: true,
          borderRadius: BorderRadius.circular(12),
          items: [
            for (final v in values)
              DropdownMenuItem<T>(value: v, child: Text(label(v))),
          ],
          onChanged: (v) {
            if (v != null) onChanged(v);
          },
        ),
      ),
      onPressed: null,
    );
  }
}
