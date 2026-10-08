/// Puente FFI multiplataforma.
///
/// - En VM/nativo (desktop, Android, iOS): implementación real
///   [`NativeBridge`](native_bridge_io.dart) sobre `dart:ffi`.
/// - En web: stub que reporta `null` en `tryLoad()` (la app usa el motor demo).
library;

export 'ffi_common.dart';
export 'native_bridge_stub.dart'
    if (dart.library.ffi) 'native_bridge_io.dart';