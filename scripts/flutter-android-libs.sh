#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Compila `libgravital_talk_ffi.so` para Android (cargo-ndk) y lo coloca
# en `app/android/app/src/main/jniLibs/{abi}/`.
#
# Requisitos:
#   - NDK instalado (sdkmanager "ndk;…" o ANDROID_NDK_HOME/NDK_HOME).
#   - cargo-ndk instalado (cargo install cargo-ndk --locked).
#   - Rust con targets android (las instala cargo-ndk automáticamente).
#
# Uso:
#   scripts/flutter-android-libs.sh          # debug
#   scripts/flutter-android-libs.sh release  # release (LTO, strip)
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

PROFILE="${1:-debug}"
ABIS=(aarch64-linux-android armv7-linux-androideabi x86_64-linux-android)
OUT="app/android/app/src/main/jniLibs"

if ! command -v cargo-ndk >/dev/null 2>&1; then
  echo "cargo-ndk no está instalado: cargo install cargo-ndk --locked" >&2
  exit 1
fi
if [ -z "${ANDROID_NDK_HOME:-}${NDK_HOME:-}" ] && [ ! -d "$ANDROID_HOME/ndk" ]; then
  echo "NDK no encontrado. Instala con sdkmanager o define ANDROID_NDK_HOME" >&2
  exit 1
fi

echo "[rust] compilando FFI (${PROFILE}) para ${ABIS[*]}"
mkdir -p "$OUT"
for abi in "${ABIS[@]}"; do
  # `--debug` no es un argumento válido de cargo (debug es el perfil por
  # defecto): sólo se pasa la bandera cuando el perfil es release.
  if [ "$PROFILE" = "release" ]; then
    cargo ndk -t "$abi" -o "$OUT" build -p gravital-talk-ffi --release
  else
    cargo ndk -t "$abi" -o "$OUT" build -p gravital-talk-ffi
  fi
done

for abi in aarch64 armv7 x86_64; do
  find "$OUT" -name "*.so" -path "*$abi*" -exec ls -la {} \;
done
echo "[rust] libs Android en $OUT"