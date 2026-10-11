import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:toastification/toastification.dart';

import 'core/glass.dart';
import 'core/theme.dart';
import 'models/connection.dart';
import 'screens/home_screen.dart';
import 'core/routes.dart';
import 'screens/onboarding_screen.dart';
import 'screens/pairing_screen.dart';
import 'services/launch_links.dart';
import 'services/pairing_uri.dart';
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
    this.initialLink,
    this.startAsHost = false,
    this.autoTalk,
  });

  /// Prueba: abrir directamente la pantalla de crear sala.
  final bool startAsHost;

  /// Prueba: al conectar, hablar y escuchar en turnos de esta duración.
  final Duration? autoTalk;

  /// Invitación con la que se abrió la app, o `null`.
  final String? initialLink;

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
  // Quien llega con una invitación viene a hablar, no a leer la presentación.
  late bool _showOnboarding = widget.showOnboarding &&
      widget.initialLink == null &&
      !widget.startAsHost;

  /// El Navigator se alcanza por clave porque las invitaciones llegan desde
  /// fuera del árbol de widgets (un intent, un argumento): no hay `context`.
  final _navigator = GlobalKey<NavigatorState>();

  @override
  void initState() {
    super.initState();
    LaunchLinks.listen(_openInvite);
    final link = widget.initialLink;
    if (link != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _openInvite(link));
    } else if (widget.startAsHost) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        _navigator.currentState?.push(ForwardRoute(
          builder: (_) => const PairingScreen(mode: PairingMode.host),
        ));
      });
    }
    if (widget.autoTalk != null) widget.controller.addListener(_autoTalk);
  }

  bool _autoTalking = false;

  /// Turnos automáticos de hablar/escuchar mientras la sesión esté viva.
  Future<void> _autoTalk() async {
    final turn = widget.autoTalk;
    final c = widget.controller;
    if (turn == null || _autoTalking || !c.isLive) return;
    _autoTalking = true;
    while (c.isLive) {
      await c.pttDown();
      await Future<void>.delayed(turn);
      await c.pttUp();
      await Future<void>.delayed(turn);
    }
    _autoTalking = false;
  }

  @override
  void dispose() {
    widget.controller.removeListener(_autoTalk);
    super.dispose();
  }

  /// Entra en la sala de una invitación y, si conecta, abre la pantalla de
  /// hablar.
  Future<void> _openInvite(String link) async {
    final c = widget.controller;
    // Una respuesta a la invitación no abre una sala: se le entrega al
    // anfitrión que la está esperando.
    if (PairingAnswer.tryParse(link) != null) {
      final ok = await c.acceptAnswer(link);
      toastification.show(
        type: ok ? ToastificationType.success : ToastificationType.error,
        title: Text(ok ? 'Respuesta recibida' : 'No se pudo usar la respuesta'),
        description: Text(ok ? 'Buscando camino hacia el invitado…' : c.error ?? ''),
        autoCloseDuration: const Duration(seconds: 4),
      );
      return;
    }
    if (c.busy || c.isLive) {
      toastification.show(
        title: const Text('Ya estás en una sala'),
        description: const Text('Termínala antes de abrir otra invitación.'),
        autoCloseDuration: const Duration(seconds: 4),
      );
      return;
    }
    if (_showOnboarding) _finishOnboarding();
    // Se entra por la misma pantalla que al escanear: muestra el progreso y,
    // si hace falta, el código de respuesta. Ella misma pasa a la de hablar
    // al conectar y enseña el error si falla.
    final nav = _navigator.currentState;
    if (nav == null) return;
    nav.popUntil((route) => route.isFirst);
    await nav.push(ForwardRoute(
      builder: (_) => PairingScreen(mode: PairingMode.join, invite: link),
    ));
  }

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
            navigatorKey: _navigator,
            debugShowCheckedModeBanner: false,
            // El fondo va por debajo del Navigator, no dentro de cada
            // pantalla: así todas las rutas comparten el mismo degradado y el
            // vidrio tiene algo que difuminar en cualquiera de ellas.
            builder: (context, child) =>
                GravitalBackground(child: child ?? const SizedBox.shrink()),
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
