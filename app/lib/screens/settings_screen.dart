import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import '../core/constants.dart';
import '../models/connection.dart';
import '../services/native_bridge.dart';
import '../services/session_controller.dart';
import '../widgets/common.dart';

/// Ajustes de audio, protocolo, tema y diagnóstico.
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

    void update(AppSettings next) => c.updateSettings(next);

    return Scaffold(
      appBar: AppBar(title: const Text('Ajustes')),
      body: ListView(
        padding: const EdgeInsets.all(16),
        children: [
          SectionCard(
            title: 'Audio y protocolo',
            subtitle: 'Aplica a la próxima conexión',
            child: Column(
              children: [
                _Dropdown<int>(
                  label: 'Sample rate',
                  value: s.sampleRate,
                  items: AudioLimits.sampleRates
                      .map((v) => DropdownMenuItem(
                          value: v, child: Text('$v Hz')))
                      .toList(),
                  onChanged: (v) => update(s.copyWith(sampleRate: v)),
                ),
                _Dropdown<int>(
                  label: 'Canales',
                  value: s.channels,
                  items: AudioLimits.channels
                      .map((v) => DropdownMenuItem(
                          value: v, child: Text(v == 1 ? 'Mono' : 'Estéreo')))
                      .toList(),
                  onChanged: (v) => update(s.copyWith(channels: v)),
                ),
                _Dropdown<int>(
                  label: 'Frame',
                  value: s.frameDurationMs,
                  items: AudioLimits.frameDurationsMs
                      .map((v) =>
                          DropdownMenuItem(value: v, child: Text('$v ms')))
                      .toList(),
                  onChanged: (v) => update(s.copyWith(frameDurationMs: v)),
                ),
                _Dropdown<int>(
                  label: 'Jitter buffer',
                  value: s.jitterBufferMs,
                  items: AudioLimits.jitterBufferMs
                      .map((v) =>
                          DropdownMenuItem(value: v, child: Text('$v ms')))
                      .toList(),
                  onChanged: (v) => update(s.copyWith(jitterBufferMs: v)),
                ),
                _Dropdown<int>(
                  label: 'MTU',
                  value: s.mtu,
                  items: AudioLimits.mtus
                      .map((v) => DropdownMenuItem(
                          value: v, child: Text('$v bytes')))
                      .toList(),
                  onChanged: (v) => update(s.copyWith(mtu: v)),
                ),
                _Dropdown<int>(
                  label: 'Bitrate máximo',
                  value: s.maxBitrate,
                  items: const [24000, 32000, 48000, 64000, 96000, 128000]
                      .map((v) => DropdownMenuItem(
                          value: v,
                          child: Text('${(v / 1000).toStringAsFixed(0)} kbps')))
                      .toList(),
                  onChanged: (v) => update(s.copyWith(maxBitrate: v)),
                ),
                const SizedBox(height: 6),
                Align(
                  alignment: Alignment.centerLeft,
                  child: Text('Codec',
                      style: Theme.of(context).textTheme.labelMedium),
                ),
                const SizedBox(height: 8),
                SizedBox(
                  width: double.infinity,
                  child: SegmentedButton<AudioCodec>(
                    segments: AudioCodec.values
                        .map((codec) => ButtonSegment(
                            value: codec, label: Text(codec.label)))
                        .toList(),
                    selected: {s.codec},
                    onSelectionChanged: (sel) =>
                        update(s.copyWith(codec: sel.first)),
                  ),
                ),
                const SizedBox(height: 10),
                Row(
                  children: [
                    Icon(Icons.info_outline,
                        size: 15,
                        color: Theme.of(context).colorScheme.onSurfaceVariant),
                    const SizedBox(width: 6),
                    Expanded(
                      child: Text(
                        'Frame = ${s.samplesPerFrame} muestras (${s.sampleRate} Hz · ${s.channels} ch)',
                        style: Theme.of(context).textTheme.bodySmall,
                      ),
                    ),
                  ],
                ),
              ],
            ),
          ),
          const SizedBox(height: 16),
          SectionCard(
            title: 'Interfaz',
            child: _Dropdown<AppThemeMode>(
              label: 'Tema',
              value: s.themeMode,
              items: AppThemeMode.values
                  .map((t) => DropdownMenuItem(value: t, child: Text(t.label)))
                  .toList(),
              onChanged: (v) => update(s.copyWith(themeMode: v)),
            ),
          ),
          const SizedBox(height: 16),
          SectionCard(
            title: 'Diagnóstico',
            subtitle: 'Motor nativo y conectividad',
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                _KV(label: 'Motor', value: '${c.engineKind.label} · ${c.engineDetail}'),
                _KV(
                  label: 'Ping FFI',
                  value: _pingText(),
                ),
                const SizedBox(height: 10),
                Row(
                  children: [
                    Expanded(
                      child: OutlinedButton.icon(
                        onPressed: _testing
                            ? null
                            : () async {
                                setState(() => _testing = true);
                                final msg = await c.testRelay();
                                final stun = await c.discoverPublicAddress();
                                if (!mounted) return;
                                setState(() {
                                  _testing = false;
                                  _stunResult = stun;
                                });
                                if (context.mounted) {
                                  ScaffoldMessenger.of(context).showSnackBar(
                                      SnackBar(content: Text(msg)));
                                }
                              },
                        icon: _testing
                            ? const SizedBox(
                                width: 16,
                                height: 16,
                                child:
                                    CircularProgressIndicator(strokeWidth: 2))
                            : const Icon(Icons.network_check),
                        label: const Text('Test relay + STUN'),
                      ),
                    ),
                  ],
                ),
                if (_stunResult != null)
                  Padding(
                    padding: const EdgeInsets.only(top: 10),
                    child: _KV(label: 'IP pública', value: _stunResult!),
                  ),
              ],
            ),
          ),
          const SizedBox(height: 24),
          Center(
            child: Text('Gravital Talk $appVersion',
                style: Theme.of(context).textTheme.labelSmall),
          ),
        ],
      ),
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
}

class _Dropdown<T> extends StatelessWidget {
  const _Dropdown({
    required this.label,
    required this.value,
    required this.items,
    required this.onChanged,
  });

  final String label;
  final T value;
  final List<DropdownMenuItem<T>> items;
  final ValueChanged<T> onChanged;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(bottom: 12),
      child: DropdownButtonFormField<T>(
        initialValue: value,
        decoration: InputDecoration(labelText: label),
        items: items,
        onChanged: (v) {
          if (v != null) onChanged(v);
        },
      ),
    );
  }
}

class _KV extends StatelessWidget {
  const _KV({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;
    return Padding(
      padding: const EdgeInsets.only(bottom: 6),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 92,
            child: Text(label,
                style: Theme.of(context)
                    .textTheme
                    .bodySmall
                    ?.copyWith(color: scheme.onSurfaceVariant)),
          ),
          Expanded(
            child: SelectableText(value,
                style: const TextStyle(fontWeight: FontWeight.w600)),
          ),
        ],
      ),
    );
  }
}
