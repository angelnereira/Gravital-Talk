import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:toastification/toastification.dart';

import 'core/theme.dart';
import 'models/connection.dart';
import 'screens/home_screen.dart';
import 'screens/onboarding_screen.dart';
import 'services/event_log.dart';
import 'services/room_events.dart';
import 'services/session_controller.dart';

/// Raíz de la aplicación.
class GravitalTalkApp extends StatefulWidget {
  const GravitalTalkApp({
    super.key,
    required this.controller,
    required this.log,
    required this.roomEvents,
    this.showOnboarding = false,
    this.onOnboardingDone,
  });

  final SessionController controller;
  final EventLog log;

  /// Estado de los participantes de la sala (stream `WatchRoom` del relay).
  final RoomEvents roomEvents;

  /// Muestra el onboarding de primer arranque en lugar de la home.
  final bool showOnboarding;

  /// Se invoca al completar/saltar el onboarding (persistencia).
  final VoidCallback? onOnboardingDone;

  @override
  State<GravitalTalkApp> createState() => _GravitalTalkAppState();
}

class _GravitalTalkAppState extends State<GravitalTalkApp> {
  late bool _showOnboarding = widget.showOnboarding;

  /// Termina el onboarding y muestra la home.
  ///
  /// El botón "Empezar" no hacía nada porque navegaba con
  /// `Navigator.of(context)`, y ese contexto es el del `Consumer`: está POR
  /// ENCIMA de `MaterialApp`, que es quien crea el Navigator. La llamada
  /// fallaba, la excepción quedaba en el log y la pantalla no cambiaba; por eso
  /// había que cerrar y reabrir la app.
  ///
  /// Se resuelve con estado en vez de con navegación imperativa: al terminar
  /// se reconstruye el árbol y `home` pasa a ser `HomeScreen`. No hay Navigator
  /// que buscar y no hay contexto que pueda estar mal.
  void _finishOnboarding() {
    widget.onOnboardingDone?.call();
    setState(() => _showOnboarding = false);
  }

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: widget.controller),
        ChangeNotifierProvider.value(value: widget.log),
        ChangeNotifierProvider.value(value: widget.roomEvents),
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
            home: _showOnboarding
                ? OnboardingScreen(onDone: _finishOnboarding)
                : const HomeScreen(),
          ),
        ),
      ),
    );
  }
}
