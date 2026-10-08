import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:toastification/toastification.dart';

import 'core/theme.dart';
import 'models/connection.dart';
import 'screens/home_screen.dart';
import 'screens/onboarding_screen.dart';
import 'services/event_log.dart';
import 'services/session_controller.dart';

/// Raíz de la aplicación.
class GravitalTalkApp extends StatelessWidget {
  const GravitalTalkApp({
    super.key,
    required this.controller,
    required this.log,
    this.showOnboarding = false,
    this.onOnboardingDone,
  });

  final SessionController controller;
  final EventLog log;

  /// Muestra el onboarding de primer arranque en lugar de la home.
  final bool showOnboarding;

  /// Se invoca al completar/saltar el onboarding (persistencia).
  final VoidCallback? onOnboardingDone;

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: controller),
        ChangeNotifierProvider.value(value: log),
      ],
      child: ToastificationWrapper(
        child: Consumer<SessionController>(
          builder: (context, c, _) => MaterialApp(
            title: 'Gravital Talk',
            debugShowCheckedModeBanner: false,
            theme: GravitalTheme.light(),
            darkTheme: GravitalTheme.dark(),
            themeMode: switch (c.settings.themeMode) {
              AppThemeMode.system => ThemeMode.system,
              AppThemeMode.light => ThemeMode.light,
              AppThemeMode.dark => ThemeMode.dark,
            },
            home: showOnboarding
                ? OnboardingScreen(
                    onDone: () {
                      onOnboardingDone?.call();
                      Navigator.of(context).pushReplacement(
                        MaterialPageRoute(builder: (_) => const HomeScreen()),
                      );
                    },
                  )
                : const HomeScreen(),
          ),
        ),
      ),
    );
  }
}
