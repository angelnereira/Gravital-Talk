/// Relay publicado por HTTPS en vez de por UDP.
///
/// Plataformas como Railway sólo publican un puerto HTTP(S) y no dejan entrar
/// UDP. El relay (`gs-relay`) atiende ahí tanto la API de salas
/// (`/api/rooms`) como el audio por WebSocket (`/ws`). Para usarlo, el campo
/// «servidor» lleva una URL (`https://talk.gravitalcloud.com`) en lugar de un
/// host: se reconoce por el esquema.
class WebRelay {
  const WebRelay._(this.base);

  /// Base HTTP(S) del relay, sin ruta.
  final Uri base;

  /// Relay de Gravital Talk en Railway: el servidor por defecto de la app.
  ///
  /// Es el dominio que da Railway, que funciona sin depender del DNS de
  /// `gravitalcloud.com`. `https://talk.gravitalcloud.com` apunta al mismo
  /// servicio y se puede usar igual en el campo de servidor.
  static const defaultUrl = 'https://gs-relay-production.up.railway.app';

  /// Interpreta [host] como relay web. `null` si es un host normal (UDP).
  ///
  /// Acepta `https://`, `http://`, `wss://` y `ws://`; los dos últimos se
  /// traducen a su equivalente HTTP para la API de salas.
  static WebRelay? parse(String host) {
    final h = host.trim();
    final uri = Uri.tryParse(h);
    if (uri == null || uri.host.isEmpty) return null;
    final scheme = switch (uri.scheme) {
      'https' || 'wss' => 'https',
      'http' || 'ws' => 'http',
      _ => null,
    };
    if (scheme == null) return null;
    return WebRelay._(Uri(
      scheme: scheme,
      host: uri.host,
      port: uri.hasPort ? uri.port : null,
    ));
  }

  /// `true` si [host] es la URL de un relay web.
  static bool matches(String host) => parse(host) != null;

  /// URL de la API para [path] (que empieza por `/`, puede llevar `?query`).
  Uri api(String path) => Uri.parse('$base$path');

  /// URL del WebSocket de audio.
  String get webSocketUrl {
    final ws = base.scheme == 'https' ? 'wss' : 'ws';
    return base.replace(scheme: ws, path: '/ws').toString();
  }
}
