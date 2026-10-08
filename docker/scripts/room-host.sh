#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Host de la prueba e2e multi-contenedor (modo sala).
# Crea la sala en el relay, publica el room code en el volumen compartido
# y transmite audio senoidal headless durante $DURATION segundos.
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

RELAY_HOST=${RELAY_HOST:-relay}
UDP_PORT=${UDP_PORT:-9000}
OBS_PORT=${OBS_PORT:-9100}
SESSION_ID=${SESSION_ID:-305419896}
DURATION=${DURATION:-25}
SHARED=${SHARED_DIR:-/shared}

mkdir -p "$SHARED"
rm -f "$SHARED/room_code"

echo "[host] esperando relay $RELAY_HOST…"
until curl -fsS "http://${RELAY_HOST}:${OBS_PORT}/healthz" >/dev/null 2>&1; do
  sleep 1
done

echo "[host] creando sala…"
RESP=$(gs room create --relay "$RELAY_HOST" --obs-port "$OBS_PORT" --session-id "$SESSION_ID")
CODE=$(echo "$RESP" | sed -n 's/.*"code": *"\([^"]*\)".*/\1/p')
if [ -z "$CODE" ]; then
  echo "[host] ERROR creando sala: $RESP" >&2
  exit 1
fi
echo "$CODE" > "$SHARED/room_code"
echo "[host] sala $CODE (session_id=$SESSION_ID) — esperando peer"

exec gs ptt \
  --relay "$RELAY_HOST" --relay-port "$UDP_PORT" --relay-obs-port "$OBS_PORT" \
  --room "$CODE" --listen --no-audio --headless --duration "$DURATION"