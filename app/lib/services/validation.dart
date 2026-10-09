/// Validación de los campos de conexión.
///
/// Sin esto, un error de tecleo sólo se descubría al fallar el handshake, con
/// el mensaje genérico del transporte. Estas validaciones fallan antes, con el
/// motivo concreto y sobre el campo que está mal.
library;

/// Resultado de validar un campo.
class FieldError {
  const FieldError(this.message);

  final String message;
}

/// Valida un host o dirección IP.
///
/// Acepta nombres de servidor (`relay.midominio.dev`) e IPv4. Rechaza cadenas
/// vacías, esquemas incluidos por error (`http://…`, frecuente al copiar de un
/// navegador) y espacios.
FieldError? validateHost(String? value, {required String fieldName}) {
  final host = (value ?? '').trim();
  if (host.isEmpty) return FieldError('Falta $fieldName');

  // Pegar "http://relay:9100" en el campo de host es un error típico al copiar
  // de un navegador, y el fallo posterior en red no lo explica.
  if (host.contains('://')) {
    return FieldError('Escribe sólo el host, sin http://');
  }
  if (host.contains(' ')) return FieldError('El host no lleva espacios');
  if (host.contains('/')) return FieldError('El host no lleva rutas');

  // Un primer carácter no alfanumérico descarta casi todo lo inválido sin
  // mantener una lista de TLDs.
  final first = host.codeUnitAt(0);
  final isValidStart = (first >= 0x30 && first <= 0x39) // 0-9
      ||
      (first >= 0x41 && first <= 0x5A) // A-Z
      ||
      (first >= 0x61 && first <= 0x7A) // a-z
      ;
  if (!isValidStart) return FieldError('Host no válido');

  return null;
}

/// Valida un puerto TCP/UDP.
///
/// El rango útil es 1-65535. Puerto 0 significa "efímero" en el sistema, que no
/// es lo que quiere alguien que teclea un puerto a mano.
FieldError? validatePort(String? value, {required String fieldName}) {
  final text = (value ?? '').trim();
  if (text.isEmpty) return FieldError('Falta $fieldName');

  final port = int.tryParse(text);
  if (port == null) return FieldError('Puerto no numérico');
  if (port < 1 || port > 65535) {
    return FieldError('Puerto fuera de rango (1-65535)');
  }
  return null;
}

/// Valida un código de sala.
///
/// El relay genera códigos con el formato `XXXX-NNNN` (ver `relay/rooms.rs`), y
/// una sala inexistente devuelve "not found" en vez de un error de formato, así
/// que conviene avisar antes de la llamada.
FieldError? validateRoomCode(String? value) {
  final code = (value ?? '').trim().toUpperCase();
  if (code.isEmpty) return const FieldError('Falta el código de sala');
  if (code.length < 4) return const FieldError('Código demasiado corto');
  if (code.contains(' ')) return const FieldError('El código no lleva espacios');
  return null;
}

/// Valida un token de sala (PSK de Noise).
///
/// El token es opcional: vacío significa sala abierta. Sólo se valida su forma
// mínima cuando se rellena.
FieldError? validateRoomToken(String? value) {
  final token = (value ?? '').trim();
  if (token.isEmpty) return null; // opcional
  if (token.length < 8) {
    return const FieldError('Token demasiado corto (mínimo 8 caracteres)');
  }
  return null;
}

/// Normaliza un código de sala para mostrarlo y guardarlo.
String normalizeRoomCode(String value) => value.trim().toUpperCase();
