#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Compila `libgravital_talk_ffi.a` estática para iOS (aarch64 + simulador)
# y la deja en build/rust/ios para que la fase de build de Xcode la
# enlace. Ver docs en app/ios/README.me (fase "Run Script").
#
# Requisitos: cargo + targets de iOS (rustup target add …).
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

TARGET_DIR="${CARGO_TARGET_DIR:-target}"
OUT="app/ios/rust"

rustup target add aarch64-apple-ios \
  x86_64-apple-ios aarch64-apple-ios-sim 2>/dev/null || true

echo "[rust] compilando gravital-talk-ffi (staticlib) para iOS…"
cargo build -p gravital-talk-ffi --release \
  --target aarch64-apple-ios

mkdir -p "$OUT"
cp "$TARGET_DIR/aarch64-apple-ios/release/libgravital_talk_ffi.a" "$OUT/"
echo "[rust] lib iOS estática en $OUT/libgravital_talk_ffi.a"
echo "   → en Xcode: Build Phases → Run Script, con este hook:" 
echo '     cargo build ... ;   # y LIBRARY_SEARCH_PATHS=$(PROJECT_DIR)/rust'