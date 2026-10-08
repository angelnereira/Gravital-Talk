#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Cliente de la prueba e2e multi-contenedor (modo sala).
# Espera el room code del host, se une a la sala y verifica que recibe
# audio. Falla con exit != 0 si no recibe ningún frame.
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

RELAY_HOST=${RELAY_HOST:-relay}
UDP_PORT=${UDP_PORT:-9000}
OBS_PORT=${OBS_PORT:-9100}
DURATION=${DURATION:-10}
SHARED=${SHARED_DIR:-/shared}

echo "[join] esperando room code del host…"
until [ -s "$SHARED/room_code" ]; do
  sleep 1
done
CODE=$(cat "$SHARED/room_code")
echo "[join] sala $CODE — conectando a $RELAY_HOST"

OUT=$(mktemp)
STATUS=0
gs ptt \
  --relay "$RELAY_HOST" --relay-port "$UDP_PORT" --relay-obs-port "$OBS_PORT" \
  --room "$CODE" --no-audio --headless --duration "$DURATION" 2>&1 | tee "$OUT" \
  || STATUS=$?

RECEIVED=$(sed -n 's/.*recibidos=\([0-9]*\).*/\1/p' "$OUT" | tail -1)
if [ "$STATUS" -ne 0 ] || [ "${RECEIVED:-0}" -lt 1 ]; then
  echo "[join] FAIL status=$STATUS frames_recibidos=${RECEIVED:-0}" >&2
  exit 1
fi
echo "[join] OK — $RECEIVED frames recibidos"