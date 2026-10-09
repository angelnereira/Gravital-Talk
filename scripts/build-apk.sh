#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Compila la app Flutter y deja el APK en `outputs/` para instalarlo.
#
# Es la regla de desarrollo del proyecto: cada cambio que toque Dart o el
# FFI debe dejar aquí un APK instalable, de modo que cualquier persona pueda
# probar el último estado sin compilar nada. Ver AGENTS.md.
#
# Uso:
#   scripts/build-apk.sh           # release unsigned (por defecto)
#   scripts/build-apk.sh debug     # debug (con símbolos, más grande)
#   scripts/build-apk.sh release
#
# Requisitos (los tiene la máquina de desarrollo):
#   - Rust con los targets android:      rustup target add aarch64-linux-android \
#                                          armv7-linux-androideabi x86_64-linux-android
#   - Android SDK + NDK en ~/Android/Sdk (o ANDROID_HOME/ANDROID_NDK_HOME)
#   - cargo-ndk:                          cargo install cargo-ndk --locked
#   - Java 17+
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

PROFILE="${1:-release}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

# ANDROID_HOME por defecto: la ruta estándar de la máquina de desarrollo.
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"
# El NDK se detecta solo: puede haber varios y sus nombres no son estables.
if [ -z "${ANDROID_NDK_HOME:-}" ]; then
  ANDROID_NDK_HOME="$(ls -d "$ANDROID_HOME"/ndk/* 2>/dev/null | sort -V | tail -1 || true)"
  export ANDROID_NDK_HOME
fi

if [ ! -d "$ANDROID_HOME" ]; then
  echo "No encuentro Android SDK en $ANDROID_HOME. Define ANDROID_HOME." >&2
  exit 1
fi

VER="$(grep -m1 '^version:' app/pubspec.yaml | awk '{print $2}')"
SHA="$(git rev-parse --short HEAD 2>/dev/null || echo 'noci')"
DATE="$(date -u +%Y%m%d)"

echo "═══════════════════════════════════════════════════════════"
echo " Gravital Talk · APK $PROFILE"
echo " versión $VER · commit $SHA"
echo " NDK $ANDROID_NDK_HOME"
echo "═══════════════════════════════════════════════════════════"

echo "[1/4] libs nativas FFI (3 ABIs)"
# El script espera 'debug' o 'release'.
./scripts/flutter-android-libs.sh "$PROFILE"

echo "[2/4] flutter build apk --$PROFILE"
cd app
flutter pub get >/dev/null
if [ "$PROFILE" = "release" ]; then
  flutter build apk --release
  SRC="build/app/outputs/flutter-apk/app-release.apk"
else
  flutter build apk --debug
  SRC="build/app/outputs/flutter-apk/app-debug.apk"
fi

echo "[3/4] copiando a outputs/"
DIR="$ROOT/outputs/android/$PROFILE"
mkdir -p "$DIR"
# Unsigned en release: el APK se firma al publicar, no en desarrollo.
SUFFIX=""
[ "$PROFILE" = "release" ] && SUFFIX="-unsigned"
OUT="$DIR/gravital-talk-$VER-$SHA-$PROFILE$SUFFIX.apk"
cp "$SRC" "$OUT"

echo "[4/4] registro"
echo "gravital-talk-$VER-$SHA-$PROFILE$SUFFIX.apk" > "$DIR/LATEST.txt"

SIZE="$(du -h "$OUT" | cut -f1)"
ABIS="$(unzip -l "$OUT" | grep -c 'libgravital_talk_ffi.so')"

echo
echo "APK listo: $OUT"
echo "  tamaño: $SIZE · ABIs: $ABIS"
echo "  instalar:"
echo "    adb install -r \"$OUT\""
echo
echo "outputs/android/$PROFILE/LATEST.txt apunta al último."
