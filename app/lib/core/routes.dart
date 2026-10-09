/// Transiciones entre pantallas.
///
/// Todas las navegaciones usaban `MaterialPageRoute`, que en Android hace un
/// fade genérico sin dirección: no se distingue "entrar" de "volver". Estas
/// rutas dan contexto de dirección con el gesto de retroceso.
///
/// Regla: las pantallas de detalle (setup, sesión) entran deslizando desde la
/// derecha. Los modales suben desde abajo, que es lo que el usuario espera de
/// algo que se descarta.
library;
import 'package:flutter/material.dart';

import '../core/tokens.dart' show GravitalDurations;


/// Ruta estándar para pantallas de detalle: entra desde la derecha.
class ForwardRoute<T> extends PageRouteBuilder<T> {
  ForwardRoute({required WidgetBuilder builder})
      : super(
          pageBuilder: (context, animation, secondaryAnimation) =>
              builder(context),
          transitionDuration: GravitalDurations.normal,
          reverseTransitionDuration: GravitalDurations.fast,
          transitionsBuilder: (context, animation, secondaryAnimation, child) =>
              _slideForward(animation, child),
        );
}

/// Ruta para acciones modales: sube desde abajo.
///
/// Se llama `BottomSheetRoute` y no `ModalRoute` porque Flutter ya exporta una
/// clase con ese nombre: el mismo conflicto que tuvo `Durations`.
class BottomSheetRoute<T> extends PageRouteBuilder<T> {
  BottomSheetRoute({required WidgetBuilder builder})
      : super(
          pageBuilder: (context, animation, secondaryAnimation) =>
              builder(context),
          transitionDuration: GravitalDurations.normal,
          reverseTransitionDuration: GravitalDurations.fast,
          opaque: false,
          barrierDismissible: true,
          barrierColor: Colors.black54,
          transitionsBuilder: (context, animation, secondaryAnimation, child) =>
              _slideUp(animation, child),
        );
}

/// Ruta para acciones breves (reintentos, avisos): sólo fade, sin desplazar.
class QuickRoute<T> extends PageRouteBuilder<T> {
  QuickRoute({required WidgetBuilder builder})
      : super(
          pageBuilder: (context, animation, secondaryAnimation) =>
              builder(context),
          transitionDuration: GravitalDurations.fast,
          reverseTransitionDuration: GravitalDurations.fast,
          transitionsBuilder: (context, animation, secondaryAnimation, child) =>
              _fadeThrough(animation, child),
        );
}

/// Desliza la pantalla nueva desde la derecha y desplaza la anterior hacia la
/// izquierda, así el gesto de "volver" se entiende sin pensarlo.
Widget _slideForward(Animation<double> animation, Widget child) {
  final curved = CurvedAnimation(
    parent: animation,
    curve: Curves.easeOutCubic,
    reverseCurve: Curves.easeInCubic,
  );
  return SlideTransition(
    position: Tween<Offset>(
      begin: const Offset(1, 0),
      end: Offset.zero,
    ).animate(curved),
    child: FadeTransition(opacity: curved, child: child),
  );
}

Widget _slideUp(Animation<double> animation, Widget child) {
  final curved = CurvedAnimation(
    parent: animation,
    curve: Curves.easeOutCubic,
    reverseCurve: Curves.easeInCubic,
  );
  return SlideTransition(
    position: Tween<Offset>(
      begin: const Offset(0, 1),
      end: Offset.zero,
    ).animate(curved),
    child: FadeTransition(opacity: curved, child: child),
  );
}

/// Fade breve: para contenido que no cambia de lugar en la jerarquía.
Widget _fadeThrough(Animation<double> animation, Widget child) {
  final curved = CurvedAnimation(
    parent: animation,
    curve: Curves.easeOut,
    reverseCurve: Curves.easeIn,
  );
  return FadeTransition(opacity: curved, child: child);
}
