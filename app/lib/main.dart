import 'package:flutter/material.dart';

import 'app.dart';
import 'services/engine.dart';
import 'services/event_log.dart';
import 'services/room_api.dart';
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

  final controller = SessionController(engine, RoomApi(), log, store);
  await controller.init();

  // Onboarding solo la primera vez.
  final prefs = await SharedPreferences.getInstance();
  final seenOnboarding = prefs.getBool('onboarding.done') ?? false;

  runApp(GravitalTalkApp(
    controller: controller,
    log: log,
    showOnboarding: !seenOnboarding,
    onOnboardingDone: () => prefs.setBool('onboarding.done', true),
  ));
}
