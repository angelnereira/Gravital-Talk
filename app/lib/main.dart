import 'package:flutter/material.dart';

import 'app.dart';
import 'services/engine.dart';
import 'services/event_log.dart';
import 'services/grpc_room_api.dart';
import 'services/room_api.dart';
import 'services/room_events.dart';
import 'services/session_controller.dart';
import 'services/settings_store.dart';
import 'package:shared_preferences/shared_preferences.dart';

Future<void> main() async {
  WidgetsFlutterBinding.ensureInitialized();

  final log = EventLog();
  final store = SettingsStore();

  // Motor nativo si la librería Rust está disponible; demo si no.
  final engine = FfiSessionEngine.tryCreate() ?? DemoSessionEngine();
  log.info('Motor seleccionado: ${engine.kind.label} (${engine.detail})');

  // Plano de control: gRPC (nativo o grpc-web) con fallback a REST.
  final rest = RoomApi();
  final RoomControlApi roomApi = FallbackRoomApi(grpc: GrpcRoomApi(), rest: rest);

  // Estado de los participantes, alimentado por `WatchRoom` del relay.
  // Se crea ANTES del controller, que es quien suscribe y desuscribe el stream
  // según el ciclo de vida de la conexión.
  final roomEvents = RoomEvents();

  final controller = SessionController(
    engine,
    roomApi,
    log,
    store,
    roomEvents,
  );
  await controller.init();

  // Onboarding solo la primera vez.
  final prefs = await SharedPreferences.getInstance();
  final seenOnboarding = prefs.getBool('onboarding.done') ?? false;

  runApp(GravitalTalkApp(
    controller: controller,
    log: log,
    roomEvents: roomEvents,
    showOnboarding: !seenOnboarding,
    onOnboardingDone: () => prefs.setBool('onboarding.done', true),
  ));
}
