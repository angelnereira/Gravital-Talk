/// Estado de una sesión y métricas asociadas.
library;

/// Mapa 1:1 con `GsSessionState` del ABI C.
enum SessionState {
  idle(0, 'Inactiva'),
  handshaking(1, 'Conectando'),
  active(2, 'Activa'),
  paused(3, 'En pausa'),
  closing(4, 'Cerrando'),
  closed(5, 'Cerrada'),
  error(6, 'Error'),
  reconnecting(7, 'Reconectando');

  const SessionState(this.code, this.label);

  final int code;
  final String label;

  static SessionState fromCode(int code) {
    for (final s in values) {
      if (s.code == code) return s;
    }
    return SessionState.error;
  }

  bool get isLive =>
      this == SessionState.active || this == SessionState.paused;
}

/// Snapshot de métricas (mapa 1:1 con `GsMetrics`).
class SessionMetrics {
  const SessionMetrics({
    this.rttMs = 0,
    this.jitterMs = 0,
    this.lossPercent = 0,
    this.reorderPercent = 0,
    this.bufferFillPercent = 0,
    this.estimatedMos = 0,
    this.packetsSent = 0,
    this.packetsReceived = 0,
    this.bytesSent = 0,
    this.bytesReceived = 0,
  });

  final double rttMs;
  final double jitterMs;
  final double lossPercent;
  final double reorderPercent;
  final double bufferFillPercent;
  final double estimatedMos;
  final int packetsSent;
  final int packetsReceived;
  final int bytesSent;
  final int bytesReceived;

  static const zero = SessionMetrics();
}

/// Motores disponibles para ejecutar la sesión.
enum EngineKind {
  /// `libgravital_talk_ffi` real (handshake + AEAD + FEC).
  native('FFI nativo', 'Motor Rust real'),

  /// Simulación local para desarrollo de UI (sin librería nativa).
  demo('Demo', 'Simulación local');

  const EngineKind(this.label, this.description);

  final String label;
  final String description;
}
