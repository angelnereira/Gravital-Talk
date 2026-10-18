/// Arranque y construcción del tema (GEUI).
///
/// ## Por qué no hay color dinámico
///
/// La sección 4.2 de `docs/gravital-experience-ui.md` reserva el color dinámico
/// para apps personales cuyo color de marca no importa. Gravital Talk ya tenía
/// una paleta propia y medida (teal de marca + ámbar de transmisión), y el
/// amarillo plano de un fondo de pantalla rompería los contrastes que el modo
/// demo y el estado "transmitiendo" dan por buenos. La decisión es por tanto
/// **semilla de marca fija en todas las plataformas**, que es lo que permite
/// `fromSeed` y mantiene claro, oscuro y alto contraste consistentes.
///
/// Reintroducirlo exigiría `dynamic_color`, y con él volver a medir los
/// contrastes de los estados semánticos en cada Android distinto.
library;

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import 'theme.dart';

/// Prepara el binding, el modo borde a borde y el overlay del sistema.
///
/// Idempotente: llamarla más de una vez no cambia nada.
void prepareApp() {
  WidgetsFlutterBinding.ensureInitialized();
  SystemChrome.setEnabledSystemUIMode(SystemUiMode.edgeToEdge);
  SystemChrome.setSystemUIOverlayStyle(
    const SystemUiOverlayStyle(
      systemNavigationBarColor: Colors.transparent,
      systemNavigationBarContrastEnforced: false,
    ),
  );
}

/// Los cuatro temas de la app, desde la misma semilla.
///
/// Se calculan una vez por arranque: `fromSeed` es puro y comparatively caro,
/// así que llamarlo en cada `build` de `MaterialApp` lo repetiría en cada
/// reconstrucción del árbol.
class GravitalThemes {
  GravitalThemes()
      : light = NTheme.build(NTheme.scheme(Brightness.light)),
        dark = NTheme.build(NTheme.scheme(Brightness.dark)),
        highContrastLight =
            NTheme.build(NTheme.scheme(Brightness.light, contrastLevel: 1.0)),
        highContrastDark =
            NTheme.build(NTheme.scheme(Brightness.dark, contrastLevel: 1.0));

  final ThemeData light;
  final ThemeData dark;
  final ThemeData highContrastLight;
  final ThemeData highContrastDark;
}