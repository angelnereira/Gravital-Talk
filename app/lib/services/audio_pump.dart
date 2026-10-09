import 'dart:async';
import 'dart:io';
import 'dart:isolate';
import 'dart:math';

import 'package:flutter/foundation.dart';
import 'package:flutter_sound/flutter_sound.dart';
import 'package:record/record.dart';

import 'native_bridge.dart';

/// Captura y reproducción de audio real sobre la sesión nativa.
///
/// Arquitectura:
///   - [AudioPump] (isolate principal): recibe frames del worker y los
///     reproduce con `flutter_sound` (PCM16). Si el plugin no está
///     disponible, los descarta sin romper la sesión.
///   - Worker isolate: captura el micrófono con `record` (PCM16), acumula
///     frames del tamaño configurado y los envía a la sesión FFI cuando PTT
///     está activo; además hace polling de recepción con timeout y reporta
///     el nivel de micrófono (RMS) para el VU meter.
///   - En plataformas sin plugins (web/desktop sin soporte) o sin micrófono,
///     cae a una fuente senoidal interna para poder probar el flujo de red.
class AudioPump {
  AudioPump._(this._isolate, this._port, this._control, this._speaker);

  final Isolate _isolate;
  final ReceivePort _port;
  final SendPort _control;
  final _SpeakerSink _speaker;

  double _level = 0;
  int _rxFrames = 0;
  bool _stopped = false;

  /// Nivel de micrófono 0..1 (RMS suavizado).
  double get micLevel => _level;

  /// Frames de audio recibidos desde que arrancó el pump.
  int get receivedFrames => _rxFrames;

  bool get speakerEnabled => _speaker.enabled;

  /// Arranca el pipeline de audio para una sesión ya conectada.
  static Future<AudioPump> start({
    required int handleAddress,
    required int sampleRate,
    required int channels,
    required int frameDurationMs,
  }) async {
    final port = ReceivePort();
    final speaker = await _SpeakerSink.start(sampleRate, channels);

    final isolate = await Isolate.spawn(
      _audioWorkerMain,
      [
        port.sendPort,
        handleAddress,
        sampleRate,
        channels,
        frameDurationMs,
        _canUsePlugins,
      ],
      debugName: 'gs-audio-pump',
    );

    // El worker informa su puerto de control como primer mensaje.
    final controlCompleter = Completer<SendPort>();
    late final AudioPump pump;
    pump = AudioPump._(isolate, port, _DeferredSendPort(controlCompleter.future), speaker);

    port.listen((message) {
      if (message is! Map) return;
      switch (message['t']) {
        case 'port':
          completePort(controlCompleter, message['p'] as SendPort, pump);
        case 'level':
          pump._level = (message['v'] as num).toDouble().clamp(0.0, 1.0);
        case 'audio':
          final data = message['d'];
          if (data is Uint8List) {
            pump._rxFrames++;
            unawaited(speaker.feed(data));
          }
      }
    });

    // Espera al puerto de control (con timeout defensivo).
    return pump;
  }

  static void completePort(
    Completer<SendPort> completer,
    SendPort port,
    AudioPump pump,
  ) {
    if (!completer.isCompleted) {
      completer.complete(port);
      (pump._control as _DeferredSendPort).attach(port);
    }
  }

  /// Activa/desactiva el envío según PTT.
  void setPtt(bool on) {
    if (_stopped) return;
    _send({'t': 'ptt', 'v': on});
  }

  void _send(Map<String, Object?> msg) {
    _control.send(msg);
  }

  /// Para el worker y libera el playback.
  Future<void> stop() async {
    if (_stopped) return;
    _stopped = true;
    _send({'t': 'stop'});
    await Future<void>.delayed(const Duration(milliseconds: 250));
    _isolate.kill(priority: Isolate.immediate);
    _port.close();
    await _speaker.stop();
  }

  static final bool _canUsePlugins = !kIsWeb &&
      (Platform.isAndroid || Platform.isIOS || Platform.isMacOS);
}

/// `SendPort` que espera a conocerse (primer mensaje del worker).
class _DeferredSendPort implements SendPort {
  _DeferredSendPort(this._future);

  final Future<SendPort> _future;
  SendPort? _resolved;

  void attach(SendPort port) => _resolved = port;

  @override
  void send(Object? message) {
    final r = _resolved;
    if (r != null) {
      r.send(message);
    } else {
      unawaited(_future.then((p) => p.send(message)));
    }
  }

  @override
  bool operator ==(Object other) => other is SendPort && other == _resolved;

  @override
  int get hashCode => _resolved?.hashCode ?? identityHashCode(this);
}

// ─────────────────────────────────────────────────────────────────────────
// Worker isolate
// ─────────────────────────────────────────────────────────────────────────

Future<void> _audioWorkerMain(List<Object?> args) async {
  final mainPort = args[0] as SendPort;
  final handleAddress = args[1] as int;
  final sampleRate = args[2] as int;
  final channels = args[3] as int;
  final frameMs = args[4] as int;
  final usePlugins = args[5] as bool;

  final control = ReceivePort();
  mainPort.send({'t': 'port', 'p': control.sendPort});

  final bridge = NativeBridge.tryLoad();
  if (bridge == null) {
    control.close();
    return;
  }
  final session = NativeSession.at(bridge, handleAddress);

  var ptt = false;
  var stopped = false;
  control.listen((msg) {
    if (msg is! Map) return;
    switch (msg['t']) {
      case 'ptt':
        ptt = msg['v'] as bool;
      case 'stop':
        stopped = true;
    }
  });

  final frameBytes = (sampleRate * frameMs ~/ 1000) * channels * 2;

  // ── Captura ────────────────────────────────────────────────────────────
  AudioRecorder? recorder;
  StreamSubscription<Uint8List>? micSub;
  Stream<Uint8List>? micStream;
  if (usePlugins) {
    try {
      recorder = AudioRecorder();
      // `hasPermission` ya pide el permiso si no está concedido (`request`
      // vale `true` por defecto en el paquete `record`). Lo que faltaba era
      // avisar cuando se deniega: sin esto el PTT "funcionaba" pero no se
      // transmitía nada, y el usuario no tenía forma de saber por qué.
      if (await recorder.hasPermission()) {
        micStream = await recorder.startStream(RecordConfig(
          encoder: AudioEncoder.pcm16bits,
          sampleRate: sampleRate,
          numChannels: channels,
        ));
      } else {
        // Sin micrófono no hay comunicación, y el usuario tiene que saberlo
        // para poder arreglarlo en Ajustes.
        mainPort.send({
          't': 'error',
          'v': 'Sin permiso de micrófono. Habilítalo en Ajustes del sistema '
              'para poder hablar.',
        });
      }
    } catch (e) {
      micStream = null;
      mainPort.send({
        't': 'error',
        'v': 'No se pudo abrir el micrófono: $e',
      });
    }
  }

  final buffer = <int>[];
  var lastLevelSent = DateTime.fromMillisecondsSinceEpoch(0);
  double smoothLevel = 0;

  void reportLevel(double value) {
    smoothLevel = smoothLevel * 0.7 + value * 0.3;
    final now = DateTime.now();
    if (now.difference(lastLevelSent).inMilliseconds >= 80) {
      lastLevelSent = now;
      mainPort.send({'t': 'level', 'v': smoothLevel});
    }
  }

  void sendFrame(List<int> frame) {
    try {
      session.sendAudio(Uint8List.fromList(frame));
      reportLevel(_rms(frame));
    } catch (_) {
      stopped = true;
    }
  }

  micSub = micStream?.listen((chunk) {
    if (stopped) return;
    if (!ptt) {
      buffer.clear();
      return;
    }
    buffer.addAll(chunk);
    while (buffer.length >= frameBytes) {
      final frame = buffer.sublist(0, frameBytes);
      buffer.removeRange(0, frameBytes);
      if (stopped) break;
      sendFrame(frame);
    }
  });

  // ── Fuente senoidal si no hay micrófono ────────────────────────────────
  Timer? sineTimer;
  if (micStream == null) {
    final rng = Random();
    var phase = 0.0;
    final step = 2 * pi * 440.0 / sampleRate;
    sineTimer = Timer.periodic(Duration(milliseconds: frameMs), (_) {
      if (stopped || !ptt) return;
      final frame = <int>[];
      final mono = sampleRate * frameMs ~/ 1000;
      for (var i = 0; i < mono; i++) {
        // Mezcla tono + ruido suave para que el RMS no sea constante.
        final s = (sin(phase) * 12000 + (rng.nextDouble() - 0.5) * 1500)
            .clamp(-32768.0, 32767.0)
            .toInt();
        for (var c = 0; c < channels; c++) {
          frame
            ..add(s & 0xFF)
            ..add((s >> 8) & 0xFF);
        }
        phase += step;
        if (phase > 2 * pi) phase -= 2 * pi;
      }
      sendFrame(frame);
    });
  }

  // ── Recepción (polling con timeout para poder parar) ───────────────────
  unawaited((() async {
    while (!stopped) {
      try {
        final frame = session.recvAudioTimeout(timeoutMs: frameMs);
        if (frame != null) {
          mainPort.send({'t': 'audio', 'd': frame});
        }
      } catch (_) {
        break;
      }
      await Future<void>.delayed(Duration.zero);
    }
  })());

  // ── Espera de parada ───────────────────────────────────────────────────
  while (!stopped) {
    await Future<void>.delayed(const Duration(milliseconds: 50));
  }

  await micSub?.cancel();
  sineTimer?.cancel();
  try {
    await recorder?.stop();
    await recorder?.dispose();
  } catch (_) {
    // El plugin pudo no haberse abierto nunca.
  }
  control.close();
}

/// RMS normalizado (0..1) de un frame PCM16 LE.
double _rms(List<int> pcm) {
  if (pcm.length < 2) return 0;
  var sum = 0.0;
  var n = 0;
  for (var i = 0; i + 1 < pcm.length; i += 2) {
    var s = pcm[i] | (pcm[i + 1] << 8);
    if (s >= 0x8000) s -= 0x10000;
    final f = s / 32768.0;
    sum += f * f;
    n++;
  }
  if (n == 0) return 0;
  return (sqrt(sum / n) * 3).clamp(0.0, 1.0);
}

// ─────────────────────────────────────────────────────────────────────────
// Playback (isolate principal, PCM16 vía flutter_sound)
// ─────────────────────────────────────────────────────────────────────────

class _SpeakerSink {
  _SpeakerSink._(this._player);

  FlutterSoundPlayer? _player;
  bool _enabled = false;

  bool get enabled => _enabled;

  static Future<_SpeakerSink> start(int sampleRate, int channels) async {
    final sink = _SpeakerSink._(null);
    if (!AudioPump._canUsePlugins) return sink;
    try {
      final player = FlutterSoundPlayer();
      await player.openPlayer();
      await player.startPlayerFromStream(
        codec: Codec.pcm16,
        interleaved: true,
        numChannels: channels,
        sampleRate: sampleRate,
        bufferSize: 2048,
      );
      sink._player = player;
      sink._enabled = true;
    } catch (_) {
      sink._enabled = false;
    }
    return sink;
  }

  Future<void> feed(Uint8List data) async {
    final p = _player;
    if (!_enabled || p == null) return;
    try {
      await p.feedUint8FromStream(data);
    } catch (_) {
      // Buffer underflow / reproducción detenida: ignorar.
    }
  }

  Future<void> stop() async {
    final p = _player;
    if (p == null) return;
    try {
      await p.stopPlayer();
      await p.closePlayer();
    } catch (_) {
      // ignorar
    }
    _enabled = false;
    _player = null;
  }
}
