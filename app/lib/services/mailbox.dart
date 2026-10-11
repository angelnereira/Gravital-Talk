import 'dart:async';
import 'dart:math';
import 'dart:typed_data';

import 'package:mqtt_client/mqtt_client.dart';
import 'package:mqtt_client/mqtt_server_client.dart';

/// Buzón de señalización: un sitio común donde dos dispositivos se dejan
/// mensajes cortos cuando todavía no saben cómo alcanzarse directamente.
///
/// Sólo lleva las llamadas e invitaciones selladas (ver `knock`): nunca
/// audio, y nada legible para quien opera el buzón. Es lo que permite que una
/// invitación funcione entre redes distintas sin enseñar un segundo código.
abstract class Mailbox {
  /// Empieza a recibir lo que llegue a `topic`. Repetirlo no hace nada.
  void subscribe(String topic);

  void unsubscribe(String topic);

  /// Deja `data` en `topic` para quien esté escuchando ahora.
  void publish(String topic, Uint8List data);

  /// Todo lo recibido, de cualquier tema: cada mensaje dice de quién es.
  Stream<Uint8List> get messages;

  void dispose();
}

/// Buzón sobre servidores MQTT públicos.
///
/// Se usan varios a la vez: son gratuitos y de terceros, así que cualquiera
/// puede estar caído. Cada mensaje se publica en todos y se escucha en todos;
/// quien recibe descarta los repetidos por su contenido.
class MqttMailbox implements Mailbox {
  MqttMailbox(List<String> servers) {
    for (final server in servers) {
      final idx = server.lastIndexOf(':');
      final host = idx > 0 ? server.substring(0, idx) : server;
      final port = idx > 0 ? int.tryParse(server.substring(idx + 1)) : null;
      if (host.isEmpty) continue;
      _links.add(_Link(host, port ?? 8883, _clientId(), _topics, _incoming));
    }
  }

  /// Candidatos: públicos y con certificado válido el 2026-10-11, pero ese
  /// mismo día no llegaron a aceptar conexiones de forma fiable. Volver a
  /// probarlos con `dart run tool/mailbox_probe.dart` antes de activarlos.
  static const defaultServers = <String>[
    'broker.emqx.io:8883',
    'broker.hivemq.com:8883',
    'test.mosquitto.org:8886',
  ];

  final _links = <_Link>[];
  final _topics = <String>{};
  final _incoming = StreamController<Uint8List>.broadcast();

  /// Identificador de conexión al azar en cada arranque: el buzón no debe
  /// poder seguir a un dispositivo de un día para otro.
  static String _clientId() {
    final rnd = Random.secure();
    return 'gt-${List.generate(8, (_) => rnd.nextInt(256).toRadixString(16).padLeft(2, '0')).join()}';
  }

  @override
  Stream<Uint8List> get messages => _incoming.stream;

  @override
  void subscribe(String topic) {
    if (!_topics.add(topic)) return;
    for (final link in _links) {
      link.subscribe(topic);
    }
  }

  @override
  void unsubscribe(String topic) {
    if (!_topics.remove(topic)) return;
    for (final link in _links) {
      link.unsubscribe(topic);
    }
  }

  @override
  void publish(String topic, Uint8List data) {
    for (final link in _links) {
      link.publish(topic, data);
    }
  }

  @override
  void dispose() {
    for (final link in _links) {
      link.close();
    }
    _incoming.close();
  }
}

/// Conexión con un servidor. Se reconecta sola y nunca lanza hacia fuera: un
/// buzón caído no puede tumbar la app.
class _Link {
  _Link(String host, int port, String id, this._topics, this._sink)
      : _client = MqttServerClient.withPort(host, id, port) {
    _client
      ..secure = true
      ..keepAlivePeriod = 45
      ..connectTimeoutPeriod = 8000
      ..autoReconnect = true
      ..resubscribeOnAutoReconnect = true
      ..logging(on: false)
      // Por defecto la librería habla MQTT 3.1, que los servidores públicos
      // actuales rechazan sin más explicación.
      ..setProtocolV311()
      ..connectionMessage =
          MqttConnectMessage().withClientIdentifier(id).startClean();
    unawaited(_connect());
  }

  final MqttServerClient _client;
  final Set<String> _topics;
  final StreamController<Uint8List> _sink;
  bool _closed = false;
  StreamSubscription<List<MqttReceivedMessage<MqttMessage>>>? _updates;

  bool get _up =>
      _client.connectionStatus?.state == MqttConnectionState.connected;

  Future<void> _connect() async {
    var wait = 5;
    while (!_closed) {
      try {
        await _client.connect();
        if (_up) break;
      } catch (_) {
        // Sin red o servidor caído: se reintenta más despacio.
      }
      try {
        _client.disconnect();
      } catch (_) {
        // Ya estaba cerrado.
      }
      await Future<void>.delayed(Duration(seconds: wait));
      wait = min(wait * 2, 120);
    }
    if (_closed) return;
    _updates = _client.updates?.listen((batch) {
      for (final received in batch) {
        final message = received.payload;
        if (message is MqttPublishMessage && !_sink.isClosed) {
          _sink.add(Uint8List.fromList(message.payload.message));
        }
      }
    }, onError: (Object _) {});
    for (final topic in _topics) {
      _subscribeNow(topic);
    }
    // Lo que se quiso enviar mientras se conectaba (típico: abrir la app
    // desde un enlace y saludar en el acto), si aún tiene sentido.
    final fresh = DateTime.now().subtract(_outboxLife);
    final waiting = List.of(_outbox);
    _outbox.clear();
    for (final (topic, data, at) in waiting) {
      if (at.isAfter(fresh)) _publishNow(topic, data);
    }
  }

  static const _outboxLife = Duration(seconds: 20);
  final _outbox = <(String, Uint8List, DateTime)>[];

  void _subscribeNow(String topic) {
    try {
      _client.subscribe(topic, MqttQos.atMostOnce);
    } catch (_) {
      // Se repetirá al reconectar.
    }
  }

  void subscribe(String topic) {
    if (_up) _subscribeNow(topic);
  }

  void unsubscribe(String topic) {
    try {
      if (_up) _client.unsubscribe(topic);
    } catch (_) {
      // Da igual: la conexión se va a cerrar o ya no existe.
    }
  }

  void publish(String topic, Uint8List data) {
    if (_up) return _publishNow(topic, data);
    if (_outbox.length < 16) _outbox.add((topic, data, DateTime.now()));
  }

  void _publishNow(String topic, Uint8List data) {
    try {
      final builder = MqttClientPayloadBuilder();
      for (final byte in data) {
        builder.addByte(byte);
      }
      // Sin retención: una llamada vieja no debe sonar a quien se conecte
      // mañana.
      _client.publishMessage(topic, MqttQos.atLeastOnce, builder.payload!);
    } catch (_) {
      // Mensaje perdido: quien llama repite cada segundo.
    }
  }

  void close() {
    _closed = true;
    _updates?.cancel();
    try {
      _client.disconnect();
    } catch (_) {
      // Ya estaba cerrado.
    }
  }
}
