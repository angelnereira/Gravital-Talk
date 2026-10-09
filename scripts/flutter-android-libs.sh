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

# Defaults para que el script funcione sin configurar nada, que es el caso
# normal cuando lo invoca Gradle: la tarea `buildRustLibs` ejecuta este script
# con un entorno limpio que NO hereda el ANDROID_HOME del terminal. Antes eso
# moría con "ANDROID_HOME: unbound variable" (por `set -u`), un error que no
# explica la causa.
export ANDROID_HOME="${ANDROID_HOME:-$HOME/Android/Sdk}"

# El NDK se detecta solo: sus nombres no son estables entre versiones y puede
# haber varios instalados. Se toma el más reciente por orden de versión.
if [ -z "${ANDROID_NDK_HOME:-}" ] && [ -z "${NDK_HOME:-}" ]; then
  ANDROID_NDK_HOME="$(ls -d "$ANDROID_HOME"/ndk/* 2>/dev/null | sort -V | tail -1 || true)"
  if [ -z "$ANDROID_NDK_HOME" ]; then
    echo "NDK no encontrado en $ANDROID_HOME/ndk. Instálalo con sdkmanager o define ANDROID_NDK_HOME" >&2
    exit 1
  fi
  export ANDROID_NDK_HOME
fi

if ! command -v cargo-ndk >/dev/null 2>&1; then
  echo "cargo-ndk no está instalado: cargo install cargo-ndk --locked" >&2
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