import 'package:flutter/material.dart';
import 'package:introduction_screen/introduction_screen.dart';

import '../ui/tokens.dart';

/// Presentación de primer arranque: tres ideas, las que hacen falta para
/// empezar a hablar.
///
/// Va sobre el mismo fondo que el resto de la app y toma sus colores del
/// esquema: en claro y en oscuro se lee igual de bien. No explica el
/// protocolo; eso está en "Acerca de" para quien lo busque.
///
/// Cada página es una ilustración circular sobre `primaryContainer` y dos
/// niveles de texto: el título manda y el cuerpo acompaña. Es un momento
/// expresivo (§7.2 de la regla de diseño), así que el paso entre páginas usa
/// la curva acentuada y el punto activo toma el color de marca.
class OnboardingScreen extends StatelessWidget {
  const OnboardingScreen({super.key, this.onDone});

  final VoidCallback? onDone;

  static const _pages = <(IconData, String, String)>[
    (
      Icons.mic_none_rounded,
      'Pulsa y habla',
      'Mantén pulsado el botón para hablar y suéltalo para escuchar. La voz '
          'va directa de un dispositivo al otro, cifrada de extremo a extremo.',
    ),
    (
      Icons.qr_code_2_rounded,
      'Un enlace para empezar',
      'Toca Invitar y comparte el enlace o enseña el código. Quien lo abra '
          'con Gravital Talk entra a hablar contigo, sin cuentas ni registros.',
    ),
    (
      Icons.people_alt_outlined,
      'Tus contactos, a un toque',
      'Con quien hables queda guardado para volver a llamarle. Y la '
          'conversación sigue aunque guardes el teléfono.',
    ),
  ];

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final scheme = theme.colorScheme;

    final decoration = PageDecoration(
      pageColor: Colors.transparent,
      imagePadding: const EdgeInsets.only(top: NSpace.xxxl),
      contentMargin: const EdgeInsets.symmetric(horizontal: NSpace.xl),
      titleTextStyle: (theme.textTheme.headlineSmall ?? const TextStyle())
          .copyWith(color: scheme.onSurface),
      bodyTextStyle: (theme.textTheme.bodyLarge ?? const TextStyle())
          .copyWith(color: scheme.onSurfaceVariant, height: 1.4),
    );
    final action = theme.textTheme.titleMedium
        ?.copyWith(fontWeight: FontWeight.w700, color: scheme.primary);

    return IntroductionScreen(
      // Transparente: detrás está el fondo común a toda la app.
      globalBackgroundColor: Colors.transparent,
      pages: [
        for (final (icon, title, body) in _pages)
          PageViewModel(
            title: title,
            body: body,
            // Círculo de contenedor con el icono dentro: el color lo pone el
            // esquema y el icono usa su sobre-color, de modo que se lee igual
            // en claro, en oscuro y con el tema dinámico del teléfono.
            image: Container(
              padding: const EdgeInsets.all(NSpace.xl),
              decoration: BoxDecoration(
                color: scheme.primaryContainer,
                borderRadius: BorderRadius.circular(NShape.full),
              ),
              child: Icon(icon, size: 84, color: scheme.onPrimaryContainer),
            ),
            decoration: decoration,
          ),
      ],
      onDone: () => onDone?.call(),
      onSkip: () => onDone?.call(),
      showSkipButton: true,
      showNextButton: true,
      next: Icon(Icons.arrow_forward, color: scheme.primary),
      done: Text('Empezar', style: action),
      skip: Text(
        'Saltar',
        style: theme.textTheme.titleMedium
            ?.copyWith(color: scheme.onSurfaceVariant),
      ),
      curve: NCurve.emphasizedDecelerate,
      dotsDecorator: DotsDecorator(
        activeColor: scheme.primary,
        color: scheme.onSurfaceVariant.withValues(alpha: 0.4),
        size: const Size(8, 8),
        activeSize: const Size(22, 8),
        activeShape: NShape.rounded(NShape.full),
      ),
    );
  }
}