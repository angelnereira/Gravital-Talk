#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Publica los crates del workspace en orden de dependencias (crates.io).
#
#   scripts/publish-order.sh --dry-run   # valida empaquetado (parcial)
#   scripts/publish-order.sh             # publica de verdad
#
# Notas:
# - `gravital-talk-core` y `gravital-talk-io` no dependen de otros crates
#   del workspace y se pueden validar con `--dry-run` en cualquier momento.
# - El resto depende de crates aún no publicados: `cargo publish` real
#   funciona en orden (cargo resuelve los recién publicados), pero
#   `--dry-run` fallará hasta que las dependencias estén en crates.io.
# - Requiere `cargo login` y versión del workspace sin cambios.
set -euo pipefail

DRY=""
if [ "${1:-}" = "--dry-run" ]; then
  DRY="--dry-run"
fi

# Orden topológico (cada crate tras sus dependencias internas).
CRATES=(
  gravital-talk-core
  gravital-talk-io
  gravital-talk-metrics
  gravital-talk-codec
  gravital-talk-transport
  gravital-talk
  gravital-talk-ffi
  gravital-talk-cli
  gravital-talk-relay
)

for crate in "${CRATES[@]}"; do
  echo "==== publicando $crate ${DRY:+(dry-run)} ===="
  cargo publish -p "$crate" $DRY
done

echo "== listo: SDKs Python/Web se publican aparte (maturin / wasm-pack) =="