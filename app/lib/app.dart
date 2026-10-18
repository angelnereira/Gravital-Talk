import 'dart:async';

import 'package:flutter/material.dart';
import 'package:provider/provider.dart';
import 'package:toastification/toastification.dart';

import 'core/routes.dart';
import 'models/connection.dart';
import 'screens/home_screen.dart';
import 'ui/app.dart';
import 'ui/patterns.dart' show UIDialogActions;
import 'screens/invite_screen.dart';
import 'screens/onboarding_screen.dart';
import 'screens/pairing_screen.dart';
import 'services/background_session.dart';
import 'services/contacts.dart';
import 'services/launch_links.dart';
import 'services/pairing_uri.dart';
import 'services/event_log.dart';
import 'services/haptics.dart';
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
    this.contacts,
    this.callFirst = false,
    this.startInviting = false,
    this.autoAccept = false,
  });

  /// Prueba: abrir directamente la pantalla de invitar.
  final bool startInviting;

  /// Prueba: llamar al primer contacto nada más arrancar.
  final bool callFirst;

  /// Prueba: aceptar las llamadas de contactos sin preguntar.
  final bool autoAccept;

  /// Contactos y llamadas entre ellos; `null` en pruebas, que van sin ellos.
  final ContactsService? contacts;

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
  late final ContactsService _contacts =
      widget.contacts ?? ContactsService(widget.controller);
  bool _askingCall = false;

  /// Diálogo de la llamada entrante, mientras está abierto: lo cierra también
  /// la notificación ("Contestar" / "Ahora no").
  BuildContext? _callDialog;

  /// Respuesta que llegó antes de que el diálogo existiera. Con la app oculta
  /// Flutter no dibuja: el diálogo se pide al llegar la llamada, pero no se
  /// construye hasta que la actividad vuelve, y el "contestar" de la burbuja o
  /// de la notificación llega justo antes. Sin guardarla se perdía y había que
  /// aceptar otra vez dentro de la app.
  bool? _earlyAnswer;

  void _onCallAction(bool accept) {
    final dialog = _callDialog;
    if (dialog != null && dialog.mounted) {
      Navigator.pop(dialog, accept);
    } else if (_askingCall) {
      _earlyAnswer = accept;
    }
  }

  /// Contestada desde la burbuja: en cuanto la sala conecte, la app vuelve al
  /// fondo y se habla desde la burbuja, sin perder lo que se estaba haciendo.
  bool _backWhenLive = false;
  bool _sawBusy = false;

  void _onBubbleAnswer() {
    if (!_askingCall) return;
    _backWhenLive = true;
    _sawBusy = false;
    _onCallAction(true);
  }

  void _returnWhenLive() {
    if (!_backWhenLive) return;
    final c = widget.controller;
    if (c.isLive) {
      _backWhenLive = false;
      unawaited(BackgroundSession.moveToBack());
    } else if (c.busy) {
      _sawBusy = true;
    } else if (_sawBusy) {
      // No conectó: se queda a la vista, que es donde se ve el error.
      _backWhenLive = false;
    }
  }

  /// Alguien abrió una invitación de este dispositivo.
  void _onAccepted() {
    final contact = _contacts.accepted.value;
    if (contact == null) return;
    Haptics.notice();
    toastification.show(
      type: ToastificationType.success,
      title: Text('${contact.name} aceptó tu invitación'),
      description: const Text('Ya está en tus contactos.'),
      autoCloseDuration: const Duration(seconds: 4),
    );
  }

  /// Enseña la solicitud de un contacto y, si se acepta, entra en su sala.
  Future<void> _onIncomingCall() async {
    final call = _contacts.incoming.value;
    final context = _navigator.currentContext;
    if (call == null || context == null || _askingCall) return;
    _askingCall = true;
    final accepted = widget.autoAccept || call.auto || await showDialog<bool>(
      context: context,
      barrierDismissible: false,
      builder: (dialog) => AlertDialog(
        icon: Builder(builder: (inside) {
          _callDialog = inside;
          final early = _earlyAnswer;
          if (early != null) {
            _earlyAnswer = null;
            WidgetsBinding.instance.addPostFrameCallback((_) {
              if (inside.mounted) Navigator.pop(inside, early);
            });
          }
          return const Icon(Icons.record_voice_over_outlined);
        }),
        title: Text('${call.contact.name} quiere hablar'),
        content: const Text('Si aceptas, entras directamente en su sala.'),
        actions: [
          UIDialogActions(
            cancelLabel: 'Ahora no',
            onCancel: () => Navigator.pop(dialog, false),
            confirmLabel: 'Aceptar',
            onConfirm: () => Navigator.pop(dialog, true),
          ),
        ],
      ),
    ) == true;
    _callDialog = null;
    _earlyAnswer = null;
    _askingCall = false;
    if (!accepted) {
      _contacts.reject();
      return;
    }
    final invite = _contacts.accept();
    if (invite != null) await _openInvite(invite, caller: call.contact.name);
  }

  late bool _showOnboarding = widget.showOnboarding &&
      widget.initialLink == null &&
      !widget.startAsHost &&
      !widget.startInviting &&
      !widget.callFirst;

  /// El Navigator se alcanza por clave porque las invitaciones llegan desde
  /// fuera del árbol de widgets (un intent, un argumento): no hay `context`.
  final _navigator = GlobalKey<NavigatorState>();

  @override
  void initState() {
    super.initState();
    LaunchLinks.listen(_openInvite);
    _contacts.incoming.addListener(_onIncomingCall);
    _contacts.accepted.addListener(_onAccepted);
    BackgroundSession.onCallAction = _onCallAction;
    BackgroundSession.onBubbleAnswer = _onBubbleAnswer;
    widget.controller.addListener(_returnWhenLive);
    BackgroundSession.onLeave(
      () => widget.controller.reset(),
      // Desde la notificación no se puede mantener pulsado: un toque abre
      // el micrófono y otro lo cierra.
      togglePtt: () {
        final c = widget.controller;
        c.pttActive ? c.pttUp() : c.pttDown();
      },
      // La burbuja sí: mientras se mantiene pulsada, se habla.
      ptt: (down) {
        final c = widget.controller;
        down ? c.pttDown() : c.pttUp();
      },
      resume: () => widget.controller.setPaused(false),
    );
    final link = widget.initialLink;
    if (link != null) {
      WidgetsBinding.instance.addPostFrameCallback((_) => _openInvite(link));
    } else if (widget.callFirst && _contacts.contacts.isNotEmpty) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        _navigator.currentState?.push(ForwardRoute(
          builder: (_) => PairingScreen(
            mode: PairingMode.host,
            callee: _contacts.contacts.first,
          ),
        ));
      });
    } else if (widget.startInviting) {
      WidgetsBinding.instance.addPostFrameCallback((_) {
        _navigator.currentState
            ?.push(ForwardRoute(builder: (_) => const InviteScreen()));
      });
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
    widget.controller.removeListener(_returnWhenLive);
    BackgroundSession.onBubbleAnswer = null;
    _contacts.incoming.removeListener(_onIncomingCall);
    _contacts.accepted.removeListener(_onAccepted);
    BackgroundSession.onCallAction = null;
    super.dispose();
  }

  /// Entra en la sala de una invitación y, si conecta, abre la pantalla de
  /// hablar.
  Future<void> _openInvite(String link, {String? caller}) async {
    final c = widget.controller;
    // Un enlace de contacto no abre ninguna sala: guarda a quien lo envió.
    if (ContactLink.tryParse(link) != null) {
      final contact = await _contacts.addFromLink(link);
      toastification.show(
        type: contact == null
            ? ToastificationType.error
            : ToastificationType.success,
        title: Text(contact == null
            ? 'No se pudo añadir el contacto'
            : '${contact.name} añadido a tus contactos'),
        description: Text(contact == null
            ? 'El enlace es tuyo o no es válido.'
            : 'Si te está esperando, entraréis a hablar en un momento.'),
        autoCloseDuration: const Duration(seconds: 4),
      );
      return;
    }
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
    // Se entra por la misma pantalla que al escanear: muestra el progreso,
    // pasa a la de hablar al conectar y enseña el error si falla.
    final nav = _navigator.currentState;
    if (nav == null) return;
    nav.popUntil((route) => route.isFirst);
    await nav.push(ForwardRoute(
      builder: (_) => PairingScreen(
        mode: PairingMode.join,
        invite: link,
        caller: caller,
      ),
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
        ChangeNotifierProvider.value(value: _contacts),
      ],
      child: ToastificationWrapper(
        child: Consumer<SessionController>(
          builder: (context, c, _) {
            // Los cuatro temas se calculan una vez por arranque, no en cada
            // reconstrucción del árbol: `fromSeed` es puro pero no gratis.
            final themes = GravitalThemes();
            return MaterialApp(
              title: 'Gravital Talk',
              navigatorKey: _navigator,
              debugShowCheckedModeBanner: false,
              theme: themes.light,
              darkTheme: themes.dark,
              highContrastTheme: themes.highContrastLight,
              highContrastDarkTheme: themes.highContrastDark,
              themeMode: switch (c.settings.themeMode) {
                AppThemeMode.system => ThemeMode.system,
                AppThemeMode.light => ThemeMode.light,
                AppThemeMode.dark => ThemeMode.dark,
              },
            home: _showOnboarding
                  ? OnboardingScreen(onDone: _finishOnboarding)
                  : const HomeScreen(),
            );
          },
        ),
      ),
    );
  }
}
