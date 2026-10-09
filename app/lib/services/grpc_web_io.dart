/// Implementación web del canal gRPC-Web.
///
/// Este fichero sólo se compila en navegador (ver el import condicional en
/// `grpc_room_api.dart`). Importar `grpc_web.dart` desde el código común mete
/// `dart:js_interop` en el kernel y rompe el build de Android/iOS/escritorio.
library;

import 'package:grpc/grpc.dart';
import 'package:grpc/grpc_web.dart' show GrpcWebClientChannel;

/// Crea un canal gRPC-Web hacia `host:grpcPort`.
///
/// Devuelve `ClientChannel`, que es el tipo que `ServerControlClient` acepta.
/// `GrpcWebClientChannel` extiende `ClientChannelBase`, no exportado por
/// `package:grpc`, pero implementa de hecho la interfaz de `ClientChannel`
/// (`shutdown`, y lo que el cliente usa para las llamadas), así que el cast es
/// seguro aquí.
ClientChannel createGrpcWebChannel(String host, int grpcPort) {
  final channel = GrpcWebClientChannel.xhr(Uri.parse('http://$host:$grpcPort'));
  return channel as ClientChannel;
}
