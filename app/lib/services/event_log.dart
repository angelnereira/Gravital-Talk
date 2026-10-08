import 'package:flutter/foundation.dart';

enum LogLevel { info, success, warn, error }

class LogEntry {
  const LogEntry(this.time, this.level, this.message);

  final DateTime time;
  final LogLevel level;
  final String message;
}

/// Bitácora de eventos de la app (en memoria, últimas 200 entradas).
class EventLog extends ChangeNotifier {
  final List<LogEntry> _entries = [];

  List<LogEntry> get entries => List.unmodifiable(_entries.reversed);

  void info(String message) => _add(LogLevel.info, message);
  void success(String message) => _add(LogLevel.success, message);
  void warn(String message) => _add(LogLevel.warn, message);
  void error(String message) => _add(LogLevel.error, message);

  void _add(LogLevel level, String message) {
    _entries.add(LogEntry(DateTime.now(), level, message));
    if (_entries.length > 200) {
      _entries.removeAt(0);
    }
    notifyListeners();
  }

  void clear() {
    _entries.clear();
    notifyListeners();
  }
}
