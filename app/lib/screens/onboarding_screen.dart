import 'package:flutter/material.dart';
import 'package:introduction_screen/introduction_screen.dart';

/// Onboarding de primer arranque (3 páginas, animado).
class OnboardingScreen extends StatelessWidget {
  const OnboardingScreen({super.key, this.onDone});

  final VoidCallback? onDone;

  static const _pageDecoration = PageDecoration(
    titleTextStyle: TextStyle(fontSize: 24, fontWeight: FontWeight.w800),
    bodyTextStyle: TextStyle(fontSize: 15),
    imagePadding: EdgeInsets.only(top: 40),
    pageColor: Colors.transparent,
  );

  @override
  Widget build(BuildContext context) {
    final scheme = Theme.of(context).colorScheme;

    return IntroductionScreen(
      globalBackgroundColor: scheme.surface,
      pages: [
        PageViewModel(
          title: 'Audio en tiempo real',
          body:
              'Push-to-talk con latencia mínima: mantén pulsado el botón y habla. '
              'Handshake X25519 y cifrado ChaCha20-Poly1305 por paquete.',
          image: Icon(Icons.graphic_eq, size: 110, color: scheme.primary),
          decoration: _pageDecoration,
        ),
        PageViewModel(
          title: 'Servidor o P2P',
          body:
              'Crea una sala con código QR en un servidor central, o conecta '
              'directo a otro dispositivo sin intermediarios.',
          image: Icon(Icons.hub_outlined, size: 110, color: scheme.primary),
          decoration: _pageDecoration,
        ),
        PageViewModel(
          title: 'Control total',
          body:
              'Codec, sample rate, jitter buffer, MTU y bitrate configurables. '
              'Métricas en vivo: RTT, jitter, pérdida, buffer y MOS.',
          image: Icon(Icons.tune, size: 110, color: scheme.primary),
          decoration: _pageDecoration,
        ),
      ],
      onDone: () => onDone?.call(),
      onSkip: () => onDone?.call(),
      showSkipButton: true,
      showNextButton: true,
      next: const Icon(Icons.arrow_forward),
      done: const Text('Empezar', style: TextStyle(fontWeight: FontWeight.w700)),
      skip: const Text('Saltar'),
      curve: Curves.fastLinearToSlowEaseIn,
      dotsDecorator: DotsDecorator(
        activeColor: scheme.primary,
        color: scheme.outlineVariant,
        size: const Size(8, 8),
        activeSize: const Size(22, 8),
        activeShape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(8),
        ),
      ),
    );
  }
}
