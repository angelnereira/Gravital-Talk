#!/usr/bin/env bash
# build-android.sh — Compila el APK de Gravital Talk para Android
#
# Instala las dependencias necesarias si no están presentes y produce:
#   android/app/build/outputs/apk/debug/app-debug.apk
#
# Requisitos del sistema:
#   - Linux (Ubuntu/Debian) o macOS
#   - Java 17+
#   - Android SDK con build-tools 35 (ver instrucciones abajo)
#   - Rust (instalar en https://rustup.rs/)
#
# Uso:
#   ./scripts/build-android.sh              # solo arm64-v8a (más rápido)
#   ./scripts/build-android.sh --all-abis   # arm64 + armv7 + x86_64
#   ./scripts/build-android.sh --release    # APK de release (unsigned)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$SCRIPT_DIR/.."
ANDROID_DIR="$ROOT/android"
JNILIBS="$ANDROID_DIR/app/src/main/jniLibs"

NDK_VERSION="26.3.11579264"
ALL_ABIS=false
RELEASE=false

# ── Parse args ──────────────────────────────────────────────────────────────
for arg in "$@"; do
  case $arg in
    --all-abis) ALL_ABIS=true ;;
    --release)  RELEASE=true  ;;
    --help|-h)
      echo "Usage: $0 [--all-abis] [--release]"
      exit 0
      ;;
  esac
done

# ── Colors ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'
info()  { echo -e "${GREEN}[build-android]${NC} $*"; }
warn()  { echo -e "${YELLOW}[warn]${NC} $*"; }
error() { echo -e "${RED}[error]${NC} $*"; exit 1; }

# ── Check Java ───────────────────────────────────────────────────────────────
if ! command -v java &>/dev/null; then
  error "Java 17+ is required. Install: https://adoptium.net/"
fi
JAVA_VER=$(java -version 2>&1 | awk -F '"' '/version/ {print $2}' | cut -d. -f1)
[[ "$JAVA_VER" -lt 17 ]] && error "Java 17+ required (found $JAVA_VER)"
info "Java $JAVA_VER OK"

# ── Check ANDROID_HOME ───────────────────────────────────────────────────────
if [[ -z "${ANDROID_HOME:-}" && -z "${ANDROID_SDK_ROOT:-}" ]]; then
  # Common locations
  if [[ -d "$HOME/Library/Android/sdk" ]]; then
    export ANDROID_HOME="$HOME/Library/Android/sdk"
  elif [[ -d "$HOME/Android/Sdk" ]]; then
    export ANDROID_HOME="$HOME/Android/Sdk"
  else
    error "ANDROID_HOME not set. Install Android Studio or set ANDROID_HOME to your SDK path.
  On macOS: export ANDROID_HOME=\$HOME/Library/Android/sdk
  On Linux: export ANDROID_HOME=\$HOME/Android/Sdk"
  fi
fi
ANDROID_HOME="${ANDROID_HOME:-$ANDROID_SDK_ROOT}"
info "Android SDK: $ANDROID_HOME"

# ── Install NDK if missing ────────────────────────────────────────────────────
SDKMANAGER="$ANDROID_HOME/cmdline-tools/latest/bin/sdkmanager"
if [[ ! -f "$SDKMANAGER" ]]; then
  SDKMANAGER="$ANDROID_HOME/tools/bin/sdkmanager"
fi
NDK_DIR="$ANDROID_HOME/ndk/$NDK_VERSION"
if [[ ! -d "$NDK_DIR" ]]; then
  info "Installing Android NDK $NDK_VERSION..."
  if [[ -f "$SDKMANAGER" ]]; then
    "$SDKMANAGER" "ndk;$NDK_VERSION"
  else
    error "sdkmanager not found. Install NDK manually via Android Studio > SDK Manager > SDK Tools > NDK."
  fi
fi
export ANDROID_NDK_HOME="$NDK_DIR"
export NDK_HOME="$NDK_DIR"
info "NDK: $NDK_DIR"

# ── Check Rust ───────────────────────────────────────────────────────────────
if ! command -v cargo &>/dev/null; then
  error "Rust not found. Install: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
fi

# ── Install cargo-ndk ────────────────────────────────────────────────────────
if ! command -v cargo-ndk &>/dev/null; then
  info "Installing cargo-ndk..."
  cargo install cargo-ndk --locked
fi

# ── Add Rust Android targets ──────────────────────────────────────────────────
TARGETS=("aarch64-linux-android")
if $ALL_ABIS; then
  TARGETS+=("armv7-linux-androideabi" "x86_64-linux-android")
fi
for t in "${TARGETS[@]}"; do
  if ! rustup target list --installed | grep -q "$t"; then
    info "Adding Rust target: $t"
    rustup target add "$t"
  fi
done

# ── Compile .so files ─────────────────────────────────────────────────────────
info "Compiling Rust FFI libraries..."
CARGO_FLAGS="--features android"
if $RELEASE; then CARGO_FLAGS="$CARGO_FLAGS --release"; fi

cd "$ROOT"
for target in "${TARGETS[@]}"; do
  info "  Building for $target..."
  cargo ndk -t "$target" -o "$JNILIBS" build -p gravital-talk-ffi $CARGO_FLAGS
done

echo ""
info "Built .so files:"
find "$JNILIBS" -name "*.so" -exec ls -lh {} \;

# ── Build APK ─────────────────────────────────────────────────────────────────
cd "$ANDROID_DIR"
chmod +x gradlew

if $RELEASE; then
  info "Building release APK..."
  ./gradlew assembleRelease --no-daemon
  APK_PATH="app/build/outputs/apk/release/app-release-unsigned.apk"
else
  info "Building debug APK..."
  ./gradlew assembleDebug --no-daemon
  APK_PATH="app/build/outputs/apk/debug/app-debug.apk"
fi

echo ""
echo -e "${GREEN}══════════════════════════════════════════════${NC}"
echo -e "${GREEN}  BUILD SUCCESSFUL${NC}"
echo -e "${GREEN}══════════════════════════════════════════════${NC}"
echo "  APK: $ANDROID_DIR/$APK_PATH"
echo ""
echo "Para instalar en un dispositivo conectado:"
echo "  adb install -r $ANDROID_DIR/$APK_PATH"
echo ""
echo "Para instalar y lanzar:"
echo "  adb install -r $ANDROID_DIR/$APK_PATH && \\"
echo "  adb shell am start -n com.gravitaltalk/.MainActivity"
