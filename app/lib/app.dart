import 'package:flutter/material.dart';
import 'package:provider/provider.dart';

import 'core/theme.dart';
import 'models/connection.dart';
import 'screens/home_screen.dart';
import 'services/event_log.dart';
import 'services/session_controller.dart';

/// Raíz de la aplicación.
class GravitalTalkApp extends StatelessWidget {
  const GravitalTalkApp({
    super.key,
    required this.controller,
    required this.log,
  });

  final SessionController controller;
  final EventLog log;

  @override
  Widget build(BuildContext context) {
    return MultiProvider(
      providers: [
        ChangeNotifierProvider.value(value: controller),
        ChangeNotifierProvider.value(value: log),
      ],
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
          home: const HomeScreen(),
        ),
      ),
    );
  }
}
