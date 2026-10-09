/// Emparejamiento por QR y código de autorización.
///
/// ## El problema que resuelve
///
/// El QR de la sala codificaba **sólo el código** (`GRVT-2847`). Eso sirve para
/// no teclear nueve caracteres, pero poco más: si el otro dispositivo está en
/// otra red, sigue haciendo falta introducir a mano el host del relay, el puerto
/// UDP y el de observabilidad. Un QR que te ahorra el código pero no la
/// dirección ha resuelto la parte fácil.
///
/// El formato que se usa aquí es el que ya documenta el README del
/// repositorio:
///
/// ```text
/// gravital-talk://pair?v=1&host=relay.ejemplo.com&udp=9000&obs=9100&room=GRVT-A3F2&token=secreto
/// ```
///
/// Todos los campos menos `host` y `room` son opcionales: si faltan, se rellena
/// con el valor por defecto. `token` sólo se incluye cuando la sala lo tiene.
///
/// ## Compatibilidad hacia atrás
///
/// Un QR antiguo que contenga sólo el código (`GRVT-A3F2`, sin esquema) sigue
/// funcionando: se interpreta como código de sala y el usuario completa el host
/// a mano. Ver [`PairingUri.tryParse`].
library;

import '../core/constants.dart';

/// Esquema del URI de emparejamiento.
const pairingScheme = 'gravital-talk';

/// Versión del formato. Si algún día cambian los campos, se sube y los clientes
/// viejos rechazan en vez de interpretar mal.
const pairingVersion = 1;

/// Un destino de emparejamiento, listo para rellenar un formulario.
class PairingUri {
  const PairingUri({
    required this.host,
    required this.room,
    this.udpPort,
    this.obsPort,
    this.token,
  });

  /// Intenta interpretar un valor escaneado.
  ///
  /// Acepta tres formas, de más nueva a más antigua:
  ///
  /// 1. URI completo: `gravital-talk://pair?host=…&room=…`
  /// 2. URI sin esquema: `//pair?host=…&room=…` (algunos escáneres lo cortan)
  /// 3. Código pelado: `GRVT-A3F2`
  ///
  /// Devuelve `null` si no reconoce nada, para que el llamador distinga "esto
  /// no es un QR de Gravital Talk" de "esto es un QR con un campo que falta".
  static PairingUri? tryParse(String raw) {
    final text = raw.trim();
    if (text.isEmpty) return null;

    // Forma 3: un código pelado. Se acepta con host vacío a propósito: el
    // usuario tendrá que ponerlo, pero al menos el código ya está.
    if (!text.contains('://') && !text.startsWith('//')) {
      final code = text.toUpperCase();
      if (_looksLikeRoomCode(code)) {
        return PairingUri(host: '', room: code);
      }
      return null;
    }

    // Formas 1 y 2: parsear como URI.
    final normalized = text.startsWith('//') ? 'gravital-talk:$text' : text;
    final uri = Uri.tryParse(normalized);
    if (uri == null || uri.scheme != pairingScheme) return null;

    final host = uri.queryParameters['host']?.trim() ?? '';
    final room = (uri.queryParameters['room'] ?? '').trim().toUpperCase();
    if (room.isEmpty) return null;

    return PairingUri(
      host: host,
      room: room,
      udpPort: int.tryParse(uri.queryParameters['udp'] ?? ''),
      obsPort: int.tryParse(uri.queryParameters['obs'] ?? ''),
      token: uri.queryParameters['token']?.trim(),
    );
  }

  static bool _looksLikeRoomCode(String code) {
    // El formato del relay es `XXXX-NNNN`. Se acepta cualquier cosa con guión y
    // longitud razonable para no romper códigos futuros, pero se exige el guión
    // para no confundir un código con texto arbitrario.
    return code.contains('-') && code.length >= 4 && code.length <= 24;
  }

  /// Host del relay. Vacío si el QR no lo llevaba (QR antiguo).
  final String host;

  /// Código de sala.
  final String room;

  /// Puerto UDP. `null` = usar el de por defecto.
  final int? udpPort;

  /// Puerto HTTP de observabilidad. `null` = usar el de por defecto.
  final int? obsPort;

  /// Token de sala. `null` o vacío = sala abierta.
  final String? token;

  /// `true` si el QR traía todo lo necesario para conectar sin teclear nada.
  ///
  /// Es la comprobación que decide si la pantalla de unirse pasa directa a
  /// conectar o si tiene que pedir el host al usuario.
  bool get isComplete => host.isNotEmpty && room.isNotEmpty;

  /// Genera el URI correspondiente, para mostrarlo como QR.
  ///
  /// El token se incluye sólo si no está vacío: un QR con `token=` no aporta
  /// nada y confunde.
  String toUri() {
    final params = <String, String>{
      'v': '$pairingVersion',
      if (host.isNotEmpty) 'host': host,
      'room': room,
    };
    if (udpPort != null) params['udp'] = '$udpPort';
    if (obsPort != null) params['obs'] = '$obsPort';
    if (token != null && token!.isNotEmpty) params['token'] = token!;

    final query = params.entries
        .map((e) =>
            '${Uri.encodeQueryComponent(e.key)}=${Uri.encodeQueryComponent(e.value)}')
        .join('&');
    return '$pairingScheme://pair?$query';
  }

  /// Versión legible para compartir en voz alta o pegar en un chat.
  ///
  /// No es el URI: una persona no lee `gravital-talk://pair?…` por teléfono.
  String toHumanReadable() {
    final parts = <String>['Sala $room'];
    if (host.isNotEmpty) {
      final port = udpPort ?? DefaultPorts.udp;
      parts.add('servidor $host:$port');
    }
    if (token != null && token!.isNotEmpty) {
      parts.add('token requerido');
    }
    return parts.join(' · ');
  }
}
