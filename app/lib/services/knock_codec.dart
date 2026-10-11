import 'dart:typed_data';

/// Sella y abre las llamadas a contactos. Lo implementa la librería nativa:
/// la criptografía no se reescribe en Dart.
abstract class KnockCodec {
  /// Identificador público (8 bytes) del contacto que comparte `secret`.
  Uint8List? contactId(Uint8List secret);

  Uint8List? knockSeal(Uint8List secret, Uint8List payload);

  /// `null` si el datagrama no viene de quien comparte `secret`.
  Uint8List? knockOpen(Uint8List secret, Uint8List datagram);
}
