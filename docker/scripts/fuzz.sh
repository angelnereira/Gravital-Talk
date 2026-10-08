#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Smoke de fuzzing acotado: recorre los targets de `fuzz/` con cargo-fuzz
# y un presupuesto de tiempo por target (FUZZ_SECONDS).
# ─────────────────────────────────────────────────────────────────────
set -uo pipefail

cd /src/fuzz
FUZZ_SECONDS=${FUZZ_SECONDS:-60}
FAILED=0

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo no está instalado (¿imagen correcta?)" >&2
  exit 1
fi

# El primer run compila el harness; los siguientes reusan el build.
for t in fuzz_targets/*.rs; do
  name=$(basename "$t" .rs)
  echo "===== fuzz $name (${FUZZ_SECONDS}s) ====="
  if ! timeout "$((FUZZ_SECONDS + 300))" cargo +nightly fuzz run "$name" \
      -- -max_total_time="$FUZZ_SECONDS" 2>&1; then
    # cargo-fuzz sale distinto de 0 cuando encuentra un crash: señalarlo.
    echo "[fuzz] $name: ejecución terminó con código $?"
    FAILED=1
  fi
done

if [ "$FAILED" -eq 0 ]; then
  echo "[fuzz] smoke completado sin crashes"
else
  echo "[fuzz] ¡se detectaron crashes! revisar los artifacts de fuzz/" >&2
fi
exit "$FAILED"