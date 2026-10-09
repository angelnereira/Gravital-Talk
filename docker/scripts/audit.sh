#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# Auditoría end-to-end del harness `gs-audit` dentro del contenedor `cli`.
#
# A diferencia de room-host.sh / room-join.sh (que sólo comprueban que llega
# un frame), este script verifica el CONTENIDO del audio: frecuencia medida,
# RMS y saturación. Un flujo de bytes corruptos no pasa estos criterios.
#
# Escenarios:
#   p2p      — par directo 1:1, turnos partidos, audio en ambas direcciones
#   parallel — N sesiones P2P simultáneas con frecuencias distintas
#   room     — par vía relay usando un código de sala, turnos partidos
#
# Variables de entorno:
#   SCENARIOS   lista separada por espacios (default: "p2p parallel")
#   SESSIONS    sesiones simultáneas en el escenario `parallel` (default: 3)
#   DURATION_MS duración de cada corrida (default: 4000)
#   FRAME_MS    duración del frame de audio (default: 10, ver nota PCM)
#   CODEC       pcm|opus (default: pcm)
#   OUT_DIR     dónde escribir los JSON y WAV (default: /shared/audit)
#   BASE_PORT   puerto base del host (default: 34100)
#
# Nota sobre el escenario `room`: se conecta como JOIN a la sala que crea
# `room-host.sh`, y el códec lo negocia el primer cliente. Si el ancla usa Opus
# (su default), hay que invocar este script con CODEC=opus o la negociación
# falla con CodecMismatch.
#
# Nota sobre PCM: el tope duro de payload del core es 1176 bytes, así que un
# frame PCM de 20 ms (1920 B) NO cabe. Con `--frame-ms 20` el harness falla
# con un mensaje explícito en vez de fallar frame a frame.
# ─────────────────────────────────────────────────────────────────────────────
set -uo pipefail

SCENARIOS=${SCENARIOS:-"p2p parallel room"}
SESSIONS=${SESSIONS:-3}
DURATION_MS=${DURATION_MS:-4000}
FRAME_MS=${FRAME_MS:-10}
CODEC=${CODEC:-pcm}
OUT_DIR=${OUT_DIR:-/shared/audit}
BASE_PORT=${BASE_PORT:-34100}
RELAY_HOST=${RELAY_HOST:-relay}
RELAY_UDP_PORT=${RELAY_UDP_PORT:-9000}
RELAY_OBS_PORT=${RELAY_OBS_PORT:-9100}

mkdir -p "$OUT_DIR"

FAILED=()
PASSED=()

# run_scenario <nombre> <args...>
run_scenario() {
  local name="$1"; shift
  local json="$OUT_DIR/$name.json"
  echo "─── escenario: $name (codec=$CODEC frame=${FRAME_MS}ms dur=${DURATION_MS}ms)"
  if gs-audit \
        --scenario "$name" \
        --codec "$CODEC" \
        --frame-ms "$FRAME_MS" \
        --duration-ms "$DURATION_MS" \
        --verify-audio \
        --expect-rx-frames 5 \
        --expect-peer-ptt 1 \
        --json "$json" \
        --dump-rx-wav "$OUT_DIR" \
        "$@"; then
    PASSED+=("$name")
  else
    FAILED+=("$name")
  fi
  echo
}

for scenario in $SCENARIOS; do
  case "$scenario" in
    p2p)
      run_scenario "p2p-$CODEC" \
        --mode p2p --port "$BASE_PORT" --split-turns
      ;;
    parallel)
      run_scenario "parallel-$CODEC" \
        --mode p2p --port "$BASE_PORT" --sessions "$SESSIONS"
      ;;
    room)
      # El código de sala lo crea el host ancla (room-host.sh). El audit se
      # conecta como JOIN: el ancla ya ocupa el rol de servidor de la sala, así
      # que arrancar otro host aquí haría dos servidores compitiendo por el
      # mismo session_id.
      #
      # OJO: el códec lo fija el PRIMER cliente que se conecta a la sala. El
      # ancla usa el de `gs ptt` (opus por defecto), así que si el audit pide
      # pcm la negociación falla con CodecMismatch. Por eso AUDIT_CODEC debe
      # coincidir con el códec del ancla.
      SHARED="${SHARED_DIR:-/shared}"
      if [ ! -s "$SHARED/room_code" ]; then
        echo "─── escenario: room — OMITIDO (no hay room_code; lo crea el host ancla)"
        echo
        continue
      fi
      CODE=$(cat "$SHARED/room_code")
      echo "─── escenario: room — sala $CODE (codec=$CODEC)"
      if gs-audit \
            --role join \
            --mode room \
            --relay "$RELAY_HOST" \
            --relay-udp-port "$RELAY_UDP_PORT" \
            --relay-obs-port "$RELAY_OBS_PORT" \
            --room "$CODE" \
            --port "$((BASE_PORT + 20))" \
            --codec "$CODEC" \
            --frame-ms "$FRAME_MS" \
            --duration-ms "$DURATION_MS" \
            --verify-audio \
            --expect-rx-frames 5 \
            --json "$OUT_DIR/room-$CODEC.json" \
            --dump-rx-wav "$OUT_DIR/room-$CODEC-rx.wav"; then
        PASSED+=("room-$CODEC")
      else
        FAILED+=("room-$CODEC")
      fi
      echo
      ;;
    *)
      echo "escenario desconocido: $scenario" >&2
      exit 2
      ;;
  esac
done

echo "═══════════════════════════════════════════════"
for name in "${PASSED[@]:-}"; do [ -n "$name" ] && echo "PASS  $name"; done
for name in "${FAILED[@]:-}"; do [ -n "$name" ] && echo "FAIL  $name"; done
echo "informes: $OUT_DIR"

if [ "${#FAILED[@]}" -gt 0 ]; then
  echo "AUDITORIA FALLIDA: ${#FAILED[@]} escenario(s) con criterios incumplidos" >&2
  exit 1
fi
echo "AUDITORIA OK: ${#PASSED[@]} escenario(s) pasaron"