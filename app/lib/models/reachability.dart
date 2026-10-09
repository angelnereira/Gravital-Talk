/// Diagnostic de alcanzabilidad de red, tal y como lo calcula el core.
///
/// Mapa 1:1 con `gravital_talk_transport::reach::Reachability`. Se duplica el
/// enum en Dart en vez de compartirlo porque el core es Rust y no hay forma de
/// que el compilador verifique que los dos no se desincronicen. Lo que sí se
/// puede hacer es que el fallo sea ruidoso: `fromCode` devuelve `unknown` para
/// códigos nuevos, y la UI muestra "no he podido comprobarlo" en vez de asumir
/// que todo va bien.
library;

/// Estado de la red del anfitrión respecto a quién puede conectarse.
enum NetworkReachability {
  /// IP pública enrutable: alcanzable desde cualquier red.
  ///
  /// No garantiza que el puerto esté abierto —eso es del router—, pero descarta
  /// el caso peor.
  public(0),

  /// IP privada: sólo alcanzable desde la misma LAN.
  lanOnly(1),

  /// CGNAT: nadie de fuera podrá conectarse a este dispositivo.
  ///
  /// Es el caso que hay que reportar, porque ni el reenvío de puertos lo
  /// arregla: el operador es quien tendría que hacerlo.
  cgnat(2),

  /// No se pudo determinar (STUN no respondió).
  unknown(3);

  const NetworkReachability(this.code);

  /// Código numérico que devuelve la FFI.
  final int code;

  /// Reconstruye el enum desde el código de la FFI.
  ///
  /// Un código desconocido se trata como `unknown` a propósito: asumir
  /// "alcanzable" cuando el core ha añadido un estado nuevo sería peor que
  /// admitir que no sabemos.
  static NetworkReachability fromCode(int code) {
    for (final r in values) {
      if (r.code == code) return r;
    }
    return NetworkReachability.unknown;
  }

  /// `true` si este dispositivo puede ser anfitrión para invitados remotos.
  bool get canHostRemotely => this == NetworkReachability.public;

  /// Mensaje para el usuario, con la acción concreta que puede tomar.
  ///
  /// `null` cuando no hay nada que avisar. Un diagnóstico que no dice qué hacer
  /// es sólo una forma elegante de decir "no funciona".
  String? get warning {
    switch (this) {
      case NetworkReachability.public:
        return null;
      case NetworkReachability.lanOnly:
        return 'Sólo te pueden alcanzar dispositivos en tu misma red wifi. '
            'Para invitados de fuera, reenvía el puerto en tu router.';
      case NetworkReachability.cgnat:
        return 'Tu conexión comparte la IP entre varios clientes (CGNAT), así '
            'que nadie de fuera puede conectarse a este dispositivo. '
            'Úsalo como invitado, no como anfitrión.';
      case NetworkReachability.unknown:
        return 'No pude comprobar si eres alcanzable desde fuera. '
            'Puedes intentarlo; si falla, prueba desde otra red.';
    }
  }
}
