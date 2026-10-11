import 'package:flutter/material.dart';
import 'package:introduction_screen/introduction_screen.dart';

import '../core/glass.dart';
import '../core/tokens.dart';

/// Presentación de primer arranque: tres ideas, las que hacen falta para
/// empezar a hablar.
///
/// Va sobre el mismo fondo y con el mismo vidrio que el resto de la app, y
/// toma los colores de texto del tema: en claro y en oscuro se lee igual de
/// bien. No explica el protocolo; eso está en "Acerca de" para quien lo
/// busque.
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
    final glass = GlassTextTheme.of(context);

    final decoration = PageDecoration(
      pageColor: Colors.transparent,
      imagePadding: const EdgeInsets.only(top: Spacing.xxxl),
      contentMargin: const EdgeInsets.symmetric(horizontal: Spacing.xl),
      titleTextStyle: (theme.textTheme.headlineSmall ?? const TextStyle())
          .copyWith(fontWeight: FontWeight.w800, color: glass.primary),
      bodyTextStyle: (theme.textTheme.bodyLarge ?? const TextStyle())
          .copyWith(color: glass.secondary, height: 1.4),
    );
    final action = TextStyle(fontWeight: FontWeight.w700, color: glass.primary);

    return IntroductionScreen(
      // Transparente: detrás está el degradado común a toda la app.
      globalBackgroundColor: Colors.transparent,
      pages: [
        for (final (icon, title, body) in _pages)
          PageViewModel(
            title: title,
            body: body,
            image: GlassSurface(
              glow: scheme.primary.withValues(alpha: 0.3),
              borderRadius: 48,
              padding: const EdgeInsets.all(Spacing.xxl),
              // Color de texto del tema, no el primario: el resplandor de la
              // tarjeta ya es primario y el icono se perdía dentro.
              child: Icon(icon, size: 84, color: glass.primary),
            ),
            decoration: decoration,
          ),
      ],
      onDone: () => onDone?.call(),
      onSkip: () => onDone?.call(),
      showSkipButton: true,
      showNextButton: true,
      next: Icon(Icons.arrow_forward, color: glass.primary),
      done: Text('Empezar', style: action),
      skip: Text('Saltar', style: TextStyle(color: glass.secondary)),
      curve: Curves.fastLinearToSlowEaseIn,
      dotsDecorator: DotsDecorator(
        activeColor: scheme.primary,
        color: glass.secondary.withValues(alpha: 0.4),
        size: const Size(8, 8),
        activeSize: const Size(22, 8),
        activeShape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(8),
        ),
      ),
    );
  }
}
