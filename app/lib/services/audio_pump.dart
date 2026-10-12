import 'dart:async';
import 'dart:io';
import 'dart:isolate';
import 'dart:math';
import 'dart:typed_data';

import 'package:flutter/foundation.dart';
import 'package:flutter_sound/flutter_sound.dart';
import 'package:record/record.dart';

import 'native_bridge.dart';

/// Con `--dart-define=GT_AUDIO_STATS=true` el audio escribe una línea por
/// segundo con lo que le pasa a cada frame recibido (sale en `adb logcat`
/// también en release). Es como se mide dónde se va el retardo.
const _statsEnabled = bool.fromEnvironment('GT_AUDIO_STATS');

/// Audio de la sesión con plugins de Flutter (móvil).
///
/// En escritorio no se usa: allí la librería nativa trae su propio audio
/// (`gs_audio_*`) y el camino micrófono → red no pasa por Dart.
///
/// Reparto entre isolates, que no es arbitrario:
///
///   - **Isolate principal**: captura (`record`) y reproducción
///     (`flutter_sound`). Los plugins que entregan streams sólo funcionan
///     aquí: un isolate secundario no puede registrar el manejador de mensajes
///     de un `EventChannel`. La captura vivía en el worker, así que el stream
///     del micrófono nunca arrancaba y lo que se transmitía era el tono de
///     reserva.
///   - **Worker**: sólo la recepción, que es una llamada FFI bloqueante con
///     timeout y congelaría la UI si se hiciera en el principal.
class AudioPump {
  AudioPump._(
    this._session,
    this._isolate,
    this._port,
    this._speaker,
    this._frameBytes,
  );

  final NativeSession _session;
  final Isolate _isolate;
  final ReceivePort _port;
  final _SpeakerSink _speaker;
  final int _frameBytes;

  AudioRecorder? _recorder;
  StreamSubscription<Uint8List>? _micSub;
  final BytesBuilder _pending = BytesBuilder(copy: false);

  SendPort? _control;
  bool _ptt = false;
  bool _muted = false;
  bool _stopped = false;
  bool _micOk = false;
  double _micLevel = 0;
  double _rxLevel = 0;
  int _txFrames = 0;
  int _rxFrames = 0;
  DateTime _lastRx = DateTime.fromMillisecondsSinceEpoch(0);
  String? _warning;

  /// Nivel del micrófono 0..1. Cero si no se está transmitiendo.
  double get micLevel => _ptt ? _micLevel : 0;

  /// Nivel del audio recibido 0..1. Cae a cero cuando deja de llegar.
  double get rxLevel =>
      DateTime.now().difference(_lastRx).inMilliseconds > 250 ? 0 : _rxLevel;

  int get txFrames => _txFrames;
  int get rxFrames => _rxFrames;
  bool get micOk => _micOk;
  bool get speakerOk => _speaker.enabled;

  /// Aviso pendiente para el usuario (y lo consume).
  String? takeWarning() {
    final w = _warning;
    _warning = null;
    return w;
  }

  /// `true` donde hay plugins de captura y reproducción PCM.
  static bool get supported =>
      !kIsWeb && (Platform.isAndroid || Platform.isIOS || Platform.isMacOS);

  /// Arranca el audio de una sesión ya conectada.
  static Future<AudioPump> start({
    required NativeSession session,
    required int sampleRate,
    required int channels,
    required int frameDurationMs,
  }) async {
    final frameBytes = (sampleRate * frameDurationMs ~/ 1000) * channels * 2;
    final port = ReceivePort();
    final speaker = await _SpeakerSink.start(sampleRate, channels);
    final isolate = await Isolate.spawn(
      _receiveWorker,
      [port.sendPort, session.address, frameDurationMs],
      debugName: 'gs-audio-rx',
    );

    final pump = AudioPump._(session, isolate, port, speaker, frameBytes);
    port.listen(pump._onWorkerMessage);
    await pump._openMic(sampleRate, channels);
    if (!speaker.enabled) {
      pump._warning ??= 'No se pudo abrir el altavoz: no oirás al otro.';
    }
    return pump;
  }

  void _onWorkerMessage(Object? message) {
    if (message is SendPort) {
      _control = message;
    } else if (message is Uint8List) {
      _speaker.stats?.arrived();
      _rxFrames++;
      _rxLevel = _rxLevel * 0.5 + pcmLevel(message) * 0.5;
      _lastRx = DateTime.now();
      if (_muted) return;
      _speaker.feed(_mixTone(message));
    }
  }

  Uint8List? _tone;
  int _tonePos = 0;

  /// Suena `mono` (PCM de 16 bits) por el altavoz. Si está llegando voz se
  /// suma a ella en vez de encolarse detrás: un aviso no debe retrasarla.
  void playTone(Uint8List mono, int channels) {
    if (_stopped || _muted) return;
    final Uint8List tone;
    if (channels <= 1) {
      tone = mono;
    } else {
      tone = Uint8List(mono.length * channels);
      for (var i = 0; i + 1 < mono.length; i += 2) {
        for (var ch = 0; ch < channels; ch++) {
          tone[(i * channels) + ch * 2] = mono[i];
          tone[(i * channels) + ch * 2 + 1] = mono[i + 1];
        }
      }
    }
    final receiving = DateTime.now().difference(_lastRx) <
        const Duration(milliseconds: 100);
    if (receiving) {
      _tone = tone;
      _tonePos = 0;
    } else {
      _tone = null;
      _speaker.feed(tone);
    }
  }

  /// Suma a `frame` el tramo pendiente del tono, si lo hay.
  Uint8List _mixTone(Uint8List frame) {
    final tone = _tone;
    if (tone == null) return frame;
    final out = Uint8List.fromList(frame);
    final data = ByteData.sublistView(out);
    final src = ByteData.sublistView(tone);
    var i = 0;
    for (; i + 1 < out.length && _tonePos + 1 < tone.length; i += 2) {
      final sum = data.getInt16(i, Endian.little) +
          src.getInt16(_tonePos, Endian.little);
      data.setInt16(i, sum.clamp(-32768, 32767), Endian.little);
      _tonePos += 2;
    }
    if (_tonePos + 1 >= tone.length) _tone = null;
    return out;
  }

  Future<void> _openMic(int sampleRate, int channels) async {
    try {
      final recorder = AudioRecorder();
      // `hasPermission` pide el permiso si aún no se concedió.
      if (!await recorder.hasPermission()) {
        await recorder.dispose();
        _warning = 'Sin permiso de micrófono: puedes escuchar pero no hablar. '
            'Concédelo en los ajustes del sistema.';
        return;
      }
      final stream = await recorder.startStream(RecordConfig(
        encoder: AudioEncoder.pcm16bits,
        sampleRate: sampleRate,
        numChannels: channels,
        noiseSuppress: true,
        autoGain: true,
        // Entregar de frame en frame: un buffer mayor es latencia añadida.
        streamBufferSize: _frameBytes,
      ));
      _recorder = recorder;
      _micSub = stream.listen(_onMicChunk);
      _micOk = true;
    } catch (e) {
      _warning = 'No se pudo abrir el micrófono: $e';
    }
  }

  void _onMicChunk(Uint8List chunk) {
    if (_stopped) return;
    if (!_ptt) {
      // Lo capturado sin PTT se tira: no debe salir al pulsar después.
      _pending.clear();
      return;
    }
    _pending.add(chunk);
    if (_pending.length < _frameBytes) return;

    final all = _pending.takeBytes();
    var offset = 0;
    while (all.length - offset >= _frameBytes) {
      final frame = Uint8List.sublistView(all, offset, offset + _frameBytes);
      offset += _frameBytes;
      try {
        _session.sendAudio(frame);
        _txFrames++;
        _micLevel = _micLevel * 0.6 + pcmLevel(frame) * 0.4;
      } on NativeException catch (e) {
        // Un envío fallido no para el audio: la sesión puede estar
        // reconectando. Se avisa una vez y se sigue intentando.
        _warning ??= 'No se pudo enviar audio: ${e.message}';
      }
    }
    if (offset < all.length) {
      _pending.add(Uint8List.sublistView(all, offset));
    }
  }

  /// Pausa o reanuda la escucha: en pausa se sigue recibiendo, sin sonar.
  void setMuted(bool on) => _muted = on;

  /// Abre o cierra el paso del micrófono a la red.
  void setPtt(bool on) {
    if (_stopped) return;
    _ptt = on;
    if (!on) {
      _pending.clear();
      _micLevel = 0;
    }
  }

  /// Para el worker y libera micrófono y altavoz.
  Future<void> stop() async {
    if (_stopped) return;
    _stopped = true;
    _ptt = false;
    await _micSub?.cancel();
    try {
      await _recorder?.stop();
      await _recorder?.dispose();
    } catch (_) {
      // El plugin pudo no haberse abierto nunca.
    }
    _control?.send(null);
    // Margen para que el worker salga de su llamada bloqueante antes de que
    // quien llama destruya la sesión que está leyendo.
    await Future<void>.delayed(const Duration(milliseconds: 120));
    _isolate.kill(priority: Isolate.immediate);
    _port.close();
    await _speaker.stop();
  }
}

/// Worker de recepción: saca frames de la sesión y los manda al principal.
///
/// Cualquier mensaje por el puerto de control lo para.
Future<void> _receiveWorker(List<Object?> args) async {
  final mainPort = args[0] as SendPort;
  final handleAddress = args[1] as int;
  final frameMs = args[2] as int;

  final control = ReceivePort();
  mainPort.send(control.sendPort);
  var stopped = false;
  control.listen((_) => stopped = true);

  final bridge = NativeBridge.tryLoad();
  if (bridge == null) {
    control.close();
    return;
  }
  final session = NativeSession.at(bridge, handleAddress);

  while (!stopped) {
    try {
      final frame = session.recvAudioTimeout(timeoutMs: frameMs * 2);
      if (frame != null) {
        mainPort.send(frame);
      }
    } on NativeException {
      // Sesión cerrada o aún sin peer: esperar en vez de girar en vacío.
      await Future<void>.delayed(const Duration(milliseconds: 50));
    }
    // Ceder el turno para que llegue el mensaje de parada.
    await Future<void>.delayed(Duration.zero);
  }
  control.close();
}

/// Nivel 0..1 de un frame PCM16 LE (RMS con ganancia para el medidor).
double pcmLevel(Uint8List pcm) {
  final n = pcm.length ~/ 2;
  if (n == 0) return 0;
  final data = ByteData.sublistView(pcm);
  var sum = 0.0;
  for (var i = 0; i < n; i++) {
    final f = data.getInt16(i * 2, Endian.little) / 32768.0;
    sum += f * f;
  }
  return (sqrt(sum / n) * 3).clamp(0.0, 1.0);
}

// ─────────────────────────────────────────────────────────────────────────
// Reproducción (isolate principal, PCM16 vía flutter_sound)
// ─────────────────────────────────────────────────────────────────────────

class _SpeakerSink {
  FlutterSoundPlayer? _player;
  bool _enabled = false;

  /// `true` mientras el reproductor tiene un bloque sin aceptar.
  bool _feeding = false;

  /// Si el reproductor no contesta en este tiempo se da el bloque por
  /// entregado: un futuro que no termina no puede parar el audio.
  static const _acceptTimeout = Duration(milliseconds: 250);

  /// Medidas de la entrega, sólo si se pidieron al compilar.
  final _PlayoutStats? stats = _statsEnabled ? _PlayoutStats() : null;

  bool get enabled => _enabled;

  static Future<_SpeakerSink> start(int sampleRate, int channels) async {
    final sink = _SpeakerSink();
    if (!AudioPump.supported) return sink;
    try {
      final player = FlutterSoundPlayer();
      await player.openPlayer();
      await player.startPlayerFromStream(
        codec: Codec.pcm16,
        interleaved: true,
        numChannels: channels,
        sampleRate: sampleRate,
        bufferSize: 1024,
      );
      sink._player = player;
      sink._enabled = true;
    } catch (_) {
      sink._enabled = false;
    }
    return sink;
  }

  /// Entrega un bloque al reproductor, o lo tira si aún no aceptó el anterior.
  ///
  /// No hay cola a propósito. El reproductor acepta un bloque en unos 4 ms,
  /// así que sólo se tira lo que llega en ráfaga (tras un parón de la red), y
  /// eso es justo lo que no conviene reproducir: entra al mismo ritmo que
  /// sale, y lo que se acumule delante se queda como retardo fijo hasta el
  /// final del turno. Medido (2026-10-12, portátil → Pixel 8 Pro por LTE):
  /// guardando hasta 60 ms en vez de tirar, se perdía un 2-4 % de los frames
  /// en lugar de un 7 %, pero el retardo al final del turno subía de 150 a
  /// 230 ms.
  ///
  /// De uno en uno porque el reproductor lo exige: con dos entregas a la vez
  /// el futuro de la primera no termina nunca.
  void feed(Uint8List data) {
    final p = _player;
    if (!_enabled || p == null) return;
    if (_feeding) {
      stats?.dropped++;
      return;
    }
    _feeding = true;
    final fedAt = stats?.fed(1);
    p
        .feedUint8FromStream(data)
        .timeout(_acceptTimeout)
        .then<void>((_) {}, onError: (Object _) {
      // Reproducción detenida a medias, o sin respuesta: se sigue.
    }).whenComplete(() {
      _feeding = false;
      if (fedAt != null) stats?.accepted(fedAt);
    });
  }

  Future<void> stop() async {
    final p = _player;
    if (p == null) return;
    _enabled = false;
    _player = null;
    try {
      await p.stopPlayer();
      await p.closePlayer();
    } catch (_) {
      // ignorar
    }
  }
}

/// Lo que le pasa al audio recibido entre que llega al isolate principal y
/// que el reproductor lo acepta. Se vuelca una vez por segundo.
class _PlayoutStats {
  final Stopwatch _clock = Stopwatch()..start();
  int _windowStart = 0;
  int _lastArrival = -1;

  int _frames = 0;
  int _gapSum = 0;
  int _gapMax = 0;
  int _backlogSum = 0;
  int _backlogMax = 0;
  int _feeds = 0;
  int _acceptSum = 0;
  int _acceptMax = 0;
  int _accepts = 0;
  int dropped = 0;

  /// Llegó un frame del worker.
  void arrived() {
    final now = _clock.elapsedMicroseconds;
    // Un silencio entre turnos no es un intervalo entre frames.
    if (_lastArrival >= 0 && now - _lastArrival < 500000) {
      final gap = now - _lastArrival;
      _gapSum += gap;
      if (gap > _gapMax) _gapMax = gap;
      _frames++;
    }
    _lastArrival = now;
    if (now - _windowStart >= 1000000) _flush(now);
  }

  /// Se entregó un bloque al reproductor con `backlog` sin aceptar.
  int fed(int backlog) {
    _feeds++;
    _backlogSum += backlog;
    if (backlog > _backlogMax) _backlogMax = backlog;
    return _clock.elapsedMicroseconds;
  }

  /// El reproductor aceptó el bloque entregado en `fedAt`.
  void accepted(int fedAt) {
    final took = _clock.elapsedMicroseconds - fedAt;
    _acceptSum += took;
    if (took > _acceptMax) _acceptMax = took;
    _accepts++;
  }

  void _flush(int now) {
    if (_feeds > 0 || dropped > 0) {
      String ms(int us) => (us / 1000).toStringAsFixed(1);
      // ignore: avoid_print
      print('GT_AUDIO_STATS frames=$_frames '
          'gap_avg=${ms(_frames == 0 ? 0 : _gapSum ~/ _frames)} '
          'gap_max=${ms(_gapMax)} '
          'backlog_avg=${(_backlogSum / (_feeds == 0 ? 1 : _feeds)).toStringAsFixed(2)} '
          'backlog_max=$_backlogMax '
          'accept_avg=${ms(_accepts == 0 ? 0 : _acceptSum ~/ _accepts)} '
          'accept_max=${ms(_acceptMax)} '
          'dropped=$dropped');
    }
    _windowStart = now;
    _frames = _gapSum = _gapMax = 0;
    _backlogSum = _backlogMax = _feeds = 0;
    _acceptSum = _acceptMax = _accepts = 0;
    dropped = 0;
  }
}
