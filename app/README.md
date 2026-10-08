# App Flutter — notas de empaquetado nativo

## Librería Rust (`libgravital_talk_ffi`)

La app usa el motor FFI real cuando encuentra la librería nativa; si no,
cae al motor demo (UI completa, audio senoidal). Para empaquetar los
binarios nativos en cada plataforma:

### Android

```bash
# Requisitos: NDK + cargo-ndk
cargo install cargo-ndk --locked
sdkmanager "ndk;26.3.11579264"

# Compila los .so (aarch64, armv7, x86_64) a android/app/src/main/jniLibs/
scripts/flutter-android-libs.sh            # debug
scripts/flutter-android-libs.sh release    # release (LTO + strip)
```

El `build.gradle.kts` tiene una tarea `buildRustLibs` que ejecuta el
script automáticamente antes del build **si hay NDK** (si no, compila sin
`.so` y la app usa el motor demo). CI: `.github/workflows/flutter-android.yml`.

### iOS

```bash
# Compila libgravital_talk_ffi.a (staticlib) para aarch64-apple-ios
scripts/flutter-ios-lib.sh
```

El script deja la librería en `app/ios/rust/`. Para enlazarla en Xcode:

1. Build Phases → + → New Run Script Phase (antes de "Compile Sources"):
   ```sh
   cd "$PROJECT_DIR/../../scripts" && ./flutter-ios-lib.sh
   ```
2. Build Settings:
   - `OTHER_LDFLAGS = -lgravital_talk_ffi`
   - `LIBRARY_SEARCH_PATHS = $(PROJECT_DIR)/rust`
   - `ENABLE_BITCODE = NO`

> Nota: los hooks de gradle/Xcode son opcionales; la app funciona sin
> ellos (modo demo). El workflow de CI cubre Android end-to-end.

### Desktop / Linux (desarrollo)

`flutter run` desde `app/` busca `../target/release|debug/libgravital_talk_ffi.so`:
```bash
cd .. && cargo build -p gravital-talk-ffi --release
```