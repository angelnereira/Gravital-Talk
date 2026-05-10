# Makefile — Build nativo de Gravital Talk para Android
#
# Requisitos:
#   - Rust toolchain + cargo
#   - cargo-ndk: cargo install cargo-ndk
#   - Android NDK instalado (NDK_HOME o ANDROID_NDK_HOME configurado)
#   - Android SDK con build-tools y platform-tools
#   - ANDROID_HOME configurado
#
# Targets:
#   make android-libs    — Compila .so para arm64-v8a, armeabi-v7a y x86_64
#   make apk-debug       — Compila + genera APK de depuración
#   make apk-release     — Compila + genera APK de release (requiere keystore)
#   make install         — Instala el APK debug en el dispositivo conectado
#   make clean           — Limpia artefactos

RUST_CRATE   := gravital-talk-ffi
RUST_LIB     := libgravital_talk_ffi.so
JNILIBS_DIR  := app/src/main/jniLibs
CARGO_FLAGS  := --features android --release

TARGETS := arm64-v8a armeabi-v7a x86_64

RUST_TRIPLE_arm64-v8a   := aarch64-linux-android
RUST_TRIPLE_armeabi-v7a := armv7-linux-androideabi
RUST_TRIPLE_x86_64      := x86_64-linux-android

.PHONY: android-libs apk-debug apk-release install clean

# ─── Rust → .so ──────────────────────────────────────────────────────────────

android-libs: $(foreach abi,$(TARGETS),$(JNILIBS_DIR)/$(abi)/$(RUST_LIB))

$(JNILIBS_DIR)/arm64-v8a/$(RUST_LIB):
	@mkdir -p $(JNILIBS_DIR)/arm64-v8a
	cd .. && cargo ndk -t $(RUST_TRIPLE_arm64-v8a) -o android/$(JNILIBS_DIR) \
		build -p $(RUST_CRATE) $(CARGO_FLAGS)

$(JNILIBS_DIR)/armeabi-v7a/$(RUST_LIB):
	@mkdir -p $(JNILIBS_DIR)/armeabi-v7a
	cd .. && cargo ndk -t $(RUST_TRIPLE_armeabi-v7a) -o android/$(JNILIBS_DIR) \
		build -p $(RUST_CRATE) $(CARGO_FLAGS)

$(JNILIBS_DIR)/x86_64/$(RUST_LIB):
	@mkdir -p $(JNILIBS_DIR)/x86_64
	cd .. && cargo ndk -t $(RUST_TRIPLE_x86_64) -o android/$(JNILIBS_DIR) \
		build -p $(RUST_CRATE) $(CARGO_FLAGS)

# ─── APK ─────────────────────────────────────────────────────────────────────

apk-debug: android-libs
	./gradlew assembleDebug

apk-release: android-libs
	./gradlew assembleRelease

# ─── Dispositivo ─────────────────────────────────────────────────────────────

install: apk-debug
	adb install -r app/build/outputs/apk/debug/app-debug.apk
	@echo "Instalado. Lanzando actividad principal..."
	adb shell am start -n com.gravitaltalk/.MainActivity

# ─── Limpieza ─────────────────────────────────────────────────────────────────

clean:
	./gradlew clean
	rm -rf $(JNILIBS_DIR)/arm64-v8a/$(RUST_LIB) \
	       $(JNILIBS_DIR)/armeabi-v7a/$(RUST_LIB) \
	       $(JNILIBS_DIR)/x86_64/$(RUST_LIB)
