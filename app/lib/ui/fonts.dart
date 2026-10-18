/// Tipografía del sistema: Google Sans Flex.
///
/// ## Por qué esta fuente
///
/// Es la fuente de marca de Google, abierta bajo SIL OFL 1.1 desde noviembre de
/// 2025, así que se puede incrustar y redistribuir en una app comercial. Al
/// ser variable, un solo archivo de 3,8 MB cubre toda la escala: el peso y el
/// tamaño óptico salen de los ejes, no de ficheros aparte.
///
/// ## Por qué los ejes van en `fontVariations` y no en `fontWeight`
///
/// Flutter no deduce el eje `wght` de una fuente variable a partir de
/// `fontWeight`: eso funciona con familias que declaran instancias nombradas
/// (wght 400, 500, 700…), no con un fichero variable único. Hay que declarar
/// `FontVariation.weight()` a mano, o la app se ve con el peso por defecto
/// (400) en toda la escala y los títulos no se distinguen del cuerpo.
///
/// El tamaño óptico (`opsz`) se envía igual que el tamaño en puntos: es lo que
/// hace que una misma variable se dibuje correctamente a 11 y a 57 dp.
library;

import 'dart:ui' show FontVariation;

import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Nombre de la familia declarada en `pubspec.yaml`.
const String kFontFamily = 'GoogleSansFlex';

/// Eje de redondez. El valor por defecto de la fuente es 8; los datos y los
/// formularios van a 0, que es lo que hace legibles las cifras.
const FontVariation kRoundness = FontVariation('ROND', 0);

/// Eje de redondez para el texto corriente.
const FontVariation kRoundnessText = FontVariation('ROND', 8);

/// Las variables de una fuente, recortadas a los rangos que declara.
///
/// Fuera de rango el motor recorta por su cuenta, pero hacerlo aquí deja el
/// valor en el sitio donde se puede ver y evita sorpresas al subir el tamaño de
/// letra por accesibilidad.
class FontAxis {
  const FontAxis._();

  static const double _wghtMin = 1;
  static const double _wghtMax = 1000;
  static const double _opszMin = 1;
  static const double _opszMax = 144;

  /// Peso, en el rango 1–1000 del eje.
  static double weight(double value) =>
      value.clamp(_wghtMin, _wghtMax).toDouble();

  /// Tamaño óptico, en el rango 1–144 del eje.
  static double opticalSize(double points) =>
      points.clamp(_opszMin, _opszMax).toDouble();
}

/// Registra la licencia OFL para que aparezca en la pantalla de licencias.
///
/// Sin esto la app distribuye un fichero bajo OFL sin mostrar su licencia, que
/// es justo lo que la licencia exige.
void registerFontLicense() {
  LicenseRegistry.addLicense(() async* {
    final text = await rootBundle.loadString('assets/fonts/OFL.txt');
    yield LicenseEntryWithLineBreaks(['Google Sans Flex'], text);
  });
}