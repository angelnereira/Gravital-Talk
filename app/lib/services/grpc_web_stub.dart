/// Versión no-web del canal gRPC-Web.
///
/// Se compila en Android/iOS/escritorio. El plano de control funciona igual por
/// REST, así que en vez de fingir que gRPC-Web existe, se lanza un error
/// descriptivo que `FallbackRoomApi` convierte en "usa REST".
library;

import 'package:grpc/grpc.dart';

/// Siempre lanza: gRPC-Web no existe fuera del navegador.
ClientChannel createGrpcWebChannel(String host, int grpcPort) {
  throw UnsupportedError(
    'gRPC-Web sólo está disponible en el navegador. '
    'En escritorio y móvil el plano de control usa REST.',
  );
}
