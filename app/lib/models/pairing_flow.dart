/// Flujo de emparejamiento: crear sala, unirse, y qué estado se muestra.
///
/// ## El problema que resuelve
///
/// Emparejar exigía elegir entre "modo servidor" y "modo P2P", y rellenar entre
/// tres y cinco campos de red —host, puerto UDP, puerto HTTP, código, token— sin
/// que ninguno tuviera un valor razonable por defecto. Para decir "hola" a
/// alguien en la misma habitación.
///
/// Este modelo reduce eso a dos acciones y **sin campos de red**:
///
/// ```text
/// Crear sala  → bindea, publica QR + código, avisa si no eres alcanzable
/// Unirse      → escanea QR o teclea el código
/// ```
///
/// La topología dejó de ser una decisión del usuario: el anfitrión siempre es
/// el que escucha, el invitado siempre conecta. Que el tráfico cruce internet o
/// se quede en la LAN es consecuencia, no opción.
library;

/// Paso del flujo de emparejamiento en el que está el usuario.
enum PairingStep {
  /// Sin hacer nada: se elige entre crear o unirse.
  idle,

  /// El anfitrión está creando la sala (resolviendo sala, bindeando…).
  creating,

  /// La sala existe y se está compartiendo. El anfitrión espera invitados.
  sharing,

  /// El invitado está resolviendo el código y conectando.
  joining,

  /// Conexión establecida.
  connected,

  /// Falló algo. `reason` dice qué.
  failed;

  /// `true` si hay una operación en vuelo (mostrar progreso, no error).
  bool get isBusy =>
      this == PairingStep.creating || this == PairingStep.joining;

  /// `true` si la app está en un estado en el que se puede hablar.
  bool get isLive => this == PairingStep.connected;
}

/// Por qué falló un emparejamiento.
///
/// Cada motivo arrastra la acción concreta que el usuario puede tomar. Un error
/// sin salida es un callejón sin salida.
enum PairingFailure {
  /// La red del anfitrión no permite recibir conexiones.
  ///
  /// Es el caso que hay que detectar *antes* de compartir el QR, no después.
  notReachable,

  /// El código no corresponde a esta sala.
  wrongCode,

  /// El código rotativo caducó mientras se tecleaba.
  codeExpired,

  /// No se encontró nadie en el tiempo esperado.
  noPeerFound,

  /// El relay rechazó la sala (no existe, o exige token).
  roomRejected,

  /// El motor nativo no está disponible.
  noEngine;

  /// Mensaje para el usuario.
  String get message => switch (this) {
        PairingFailure.notReachable =>
          'Este dispositivo no puede recibir conexiones. Úsalo como invitado.',
        PairingFailure.wrongCode =>
          'Ese código no corresponde a esta sala. Comprueba que ambos veis lo mismo.',
        PairingFailure.codeExpired =>
          'El código caducó mientras lo escribías. Pide uno nuevo.',
        PairingFailure.noPeerFound =>
          'No encontré a nadie. Comprueba que ambos estáis en la misma red.',
        PairingFailure.roomRejected =>
          'El servidor rechazó la sala. Puede que el código no exista o exija token.',
        PairingFailure.noEngine =>
          'No hay motor de audio nativo. Compila la librería para hablar.',
      };

  /// Acción sugerida, o `null` si no hay ninguna que el usuario pueda tomar.
  String? get action => switch (this) {
        PairingFailure.notReachable => null,
        PairingFailure.wrongCode => 'Corregir el código',
        PairingFailure.codeExpired => 'Pedir otro código',
        PairingFailure.noPeerFound => 'Reintentar',
        PairingFailure.roomRejected => 'Comprobar el servidor',
        PairingFailure.noEngine => null,
      };
}

/// Qué se comparte con el invitado.
///
/// Es el contenido del QR y lo que se lee en voz alta. Un único objeto, para
/// que "lo que se muestra" y "lo que se codifica" no puedan divergir.
class PairingTicket {
  const PairingTicket({
    required this.roomSecret,
    required this.code,
    required this.hostEndpoint,
    this.secsUntilRotation = 30,
  });

  /// Secreto de la sala. Lo configura el anfitrión en su sesión.
  final String roomSecret;

  /// Código legible derivado del secreto, para confirmar por teléfono.
  final String code;

  /// Endpoint publicado por el anfitrión, o `null` si aún no se resolvió.
  final String? hostEndpoint;

  /// Segundos hasta que el código rote.
  final int secsUntilRotation;

  /// `true` si al código le queda poco tiempo de vida.
  ///
  /// Es la señal para avisar "date prisa" antes de que falle.
  bool get codeIsExpiring => secsUntilRotation <= 5;
}
