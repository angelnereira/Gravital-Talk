#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Cliente de rendimiento: aplica red simulada con `tc netem` en la
# interfaz por defecto y después ejecuta el flujo e2e de sala (join).
# Permite medir RTT/jitter/pérdida/MOS reales bajo condiciones de red.
#
# Variables: DELAY_MS, JITTER_MS, LOSS_PERCENT (env del servicio).
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

DELAY_MS=${DELAY_MS:-30}
JITTER_MS=${JITTER_MS:-5}
LOSS_PERCENT=${LOSS_PERCENT:-1}

IFACE=$(ip route | awk '/default/ {print $5; exit}')
if [ -z "$IFACE" ]; then
  echo "[perf] no default iface, saltando netem" >&2
else
  tc qdisc add dev "$IFACE" root netem \
      delay "${DELAY_MS}ms" "${JITTER_MS}ms" \
      loss "${LOSS_PERCENT}%" 2>/dev/null \
    && echo "[perf] netem en $IFACE: ${DELAY_MS}ms±${JITTER_MS}ms pérdida ${LOSS_PERCENT}%" \
    || echo "[perf] AVISO: no se pudo aplicar netem"
fi

exec bash /scripts/room-join.sh