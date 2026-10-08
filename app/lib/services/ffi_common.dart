/// Tipos comunes del ABI (sin `dart:ffi`) — compilables en todas las plataformas.
library;

/// Códigos de estado del ABI C (`GsStatus`).
class GsStatus {
  static const ok = 0;
  static const nullPointer = -1;
  static const invalidArgument = -2;
  static const io = -3;
  static const timeout = -4;
  static const handshake = -5;
  static const protocol = -6;
  static const invalidState = -7;
  static const closed = -8;
  static const bufferTooSmall = -9;
  static const internal = -99;

  static String describe(int code) {
    switch (code) {
      case nullPointer:
        return 'puntero nulo';
      case invalidArgument:
        return 'argumento inválido';
      case io:
        return 'error de E/S';
      case timeout:
        return 'timeout';
      case handshake:
        return 'handshake fallido';
      case protocol:
        return 'error de protocolo';
      case invalidState:
        return 'estado inválido';
      case closed:
        return 'sesión cerrada';
      case bufferTooSmall:
        return 'buffer demasiado pequeño';
      case internal:
        return 'error interno';
      default:
        return 'estado $code';
    }
  }

  const GsStatus._();
}

/// Excepción lanzada por el ABI nativo.
class NativeException implements Exception {
  NativeException(this.status, this.message);

  final int status;
  final String message;

  @override
  String toString() => 'NativeException($status): $message';
}