#!/usr/bin/env bash
# Perforación de NAT contra el NAT real del kernel.
#
# Monta, con network namespaces, dos redes privadas detrás de dos routers que
# hacen NAT (MASQUERADE + filtrado por conntrack, como un router doméstico) y
# una "internet" entre ambos donde corre el relay como servidor de encuentro:
#
#   gt-a 10.1.0.2 ── gt-nata 203.0.113.2 ─┐
#                                          ├─ gt-inet 203.0.113.10 (gs-relay)
#   gt-b 10.2.0.2 ── gt-natb 203.0.113.3 ─┘
#
# Y comprueba dos cosas:
#
#   1. CONTROL — sin servidor de encuentro, el invitado NO llega al anfitrión
#      aunque conozca su dirección pública. Si esto conectara, el NAT simulado
#      no estaría filtrando y la prueba 2 no demostraría nada.
#   2. PERFORACIÓN — con servidor de encuentro conectan directo y circula audio
#      en los dos sentidos.
#
# Uso:
#   cargo build --release -p gravital-talk-cli -p gravital-talk-relay
#   sudo scripts/nat-sim.sh
#
# Necesita root (crea namespaces e instala reglas de iptables dentro de
# ellos). No toca la red del sistema: todo vive en namespaces `gt-*` que se
# borran al salir.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GS="${GS:-$ROOT/target/release/gs}"
RELAY="${RELAY:-$ROOT/target/release/gs-relay}"
TOKEN="token-de-la-simulacion"
OUT="$(mktemp -d)"
NAMESPACES=(gt-inet gt-nata gt-natb gt-a gt-b)

[ "$(id -u)" -eq 0 ] || { echo "hace falta root: sudo $0" >&2; exit 2; }
for bin in "$GS" "$RELAY"; do
  [ -x "$bin" ] || { echo "falta $bin: compílalo antes (ver cabecera)" >&2; exit 2; }
done

cleanup() {
  # Matar lo que quede dentro de los namespaces antes de borrarlos.
  for ns in "${NAMESPACES[@]}"; do
    ip netns pids "$ns" 2>/dev/null | xargs -r kill 2>/dev/null || true
    ip netns del "$ns" 2>/dev/null || true
  done
  rm -rf "$OUT"
}
trap cleanup EXIT
cleanup_stale() { for ns in "${NAMESPACES[@]}"; do ip netns del "$ns" 2>/dev/null || true; done; }
cleanup_stale

for ns in "${NAMESPACES[@]}"; do
  ip netns add "$ns"
  ip -n "$ns" link set lo up
done

# "Internet": un bridge con el relay colgando de él.
ip -n gt-inet link add br0 type bridge
ip -n gt-inet addr add 203.0.113.10/24 dev br0
ip -n gt-inet link set br0 up

# Una red privada detrás de un router con NAT.
#   $1 = sufijo (a|b)   $2 = IP pública del router   $3 = prefijo de la LAN
make_lan() {
  local x="$1" wan_ip="$2" lan="$3"
  ip link add "wan$x" netns "gt-nat$x" type veth peer name "p$x" netns gt-inet
  ip -n gt-inet link set "p$x" master br0
  ip -n gt-inet link set "p$x" up
  ip -n "gt-nat$x" addr add "$wan_ip/24" dev "wan$x"
  ip -n "gt-nat$x" link set "wan$x" up

  ip link add "lan$x" netns "gt-nat$x" type veth peer name eth0 netns "gt-$x"
  ip -n "gt-nat$x" addr add "$lan.1/24" dev "lan$x"
  ip -n "gt-nat$x" link set "lan$x" up
  ip -n "gt-$x" addr add "$lan.2/24" dev eth0
  ip -n "gt-$x" link set eth0 up
  ip -n "gt-$x" route add default via "$lan.1"

  ip netns exec "gt-nat$x" sysctl -qw net.ipv4.ip_forward=1
  # Lo que hace un router doméstico: traducir lo que sale y dejar entrar sólo
  # lo que responde a una conexión que ya salió.
  ip netns exec "gt-nat$x" iptables -t nat -A POSTROUTING -o "wan$x" -j MASQUERADE
  ip netns exec "gt-nat$x" iptables -A FORWARD -i "wan$x" -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT
  ip netns exec "gt-nat$x" iptables -A FORWARD -i "wan$x" -j DROP
  # Y tirar lo que llega al propio router sin haberlo pedido, que es el
  # cortafuegos por defecto de cualquier router. No es un detalle: si el router
  # aceptara ese tráfico, conntrack daría por buena la conexión entrante y al
  # salir después el equipo de dentro le cambiaría el puerto público, con lo
  # que la perforación fallaría. Activable con PERMISSIVE_INPUT=1 para ver ese
  # caso.
  if [ "${PERMISSIVE_INPUT:-0}" != 1 ]; then
    ip netns exec "gt-nat$x" iptables -A INPUT -i "wan$x" -m conntrack --ctstate ESTABLISHED,RELATED -j ACCEPT
    ip netns exec "gt-nat$x" iptables -A INPUT -i "wan$x" -j DROP
  fi
}
make_lan a 203.0.113.2 10.1.0
make_lan b 203.0.113.3 10.2.0

ip netns exec gt-inet "$RELAY" --udp-bind 203.0.113.10:9000 \
  --ws-bind 203.0.113.10:9090 --observability-bind 203.0.113.10:9100 \
  >"$OUT/relay.log" 2>&1 &
sleep 0.5

# Sanidad: cada red privada llega al relay, y no se ven entre sí.
ip netns exec gt-a ping -c1 -W1 203.0.113.10 >/dev/null || { echo "gt-a no llega a internet" >&2; exit 1; }
ip netns exec gt-b ping -c1 -W1 203.0.113.10 >/dev/null || { echo "gt-b no llega a internet" >&2; exit 1; }
if ip netns exec gt-b ping -c1 -W1 10.1.0.2 >/dev/null 2>&1; then
  echo "las redes privadas se ven entre sí: la simulación no vale" >&2; exit 1
fi

# Lanza anfitrión (gt-a) e invitado (gt-b) y deja sus salidas en $OUT/$1-*.log.
#   $1 = etiqueta   $2 = args extra del anfitrión   $3 = args del invitado
#   $4 = puerto del anfitrión (distinto en cada prueba, para que una no herede
#        el estado de conntrack de la anterior)
run_pair() {
  local tag="$1" host_args="$2" guest_args="$3" port="$4"
  # shellcheck disable=SC2086
  ip netns exec gt-a timeout 25 "$GS" peer --bind 0.0.0.0 --bind-port "$port" \
    --token "$TOKEN" --duration 4 $host_args >"$OUT/$tag-host.log" 2>&1 &
  local host_pid=$!
  sleep 1
  # shellcheck disable=SC2086
  ip netns exec gt-b timeout 25 "$GS" peer --token "$TOKEN" --duration 4 \
    $guest_args >"$OUT/$tag-guest.log" 2>&1 || true
  # El anfitrión espera indefinidamente si nadie entra: se le corta.
  sleep 1
  if grep -q "handshake… OK" "$OUT/$tag-host.log"; then wait "$host_pid" || true; else kill "$host_pid" 2>/dev/null || true; fi
}

fail=0
show() { sed 's/^/      /' "$1" | grep -E "rol|encuentro|handshake|frames|RESULTADO" || true; }

echo
echo "1. CONTROL: sin servidor de encuentro (el invitado conoce la IP pública)"
run_pair control "" "--host 203.0.113.2 --port 40000" 40000
show "$OUT/control-guest.log"
if grep -q "handshake… OK" "$OUT/control-guest.log"; then
  echo "   FALLO: conectó sin perforar; el NAT simulado no filtra."; fail=1
else
  echo "   OK: el NAT del anfitrión descarta al invitado, como se esperaba."
fi

echo
echo "2. PERFORACIÓN: con servidor de encuentro en 203.0.113.10:9000"
run_pair punch "--rendezvous 203.0.113.10:9000" "--join --rendezvous 203.0.113.10:9000" 41000
echo "   anfitrión:"; show "$OUT/punch-host.log"
echo "   invitado:";  show "$OUT/punch-guest.log"
for side in host guest; do
  grep -q "handshake… OK" "$OUT/punch-$side.log" || { echo "   FALLO: $side sin handshake"; fail=1; }
  grep -q "audio real recibido" "$OUT/punch-$side.log" || { echo "   FALLO: $side sin audio"; fail=1; }
done
# Directo de verdad: cada uno ve al otro en la IP pública de su router, no en
# la del relay.
grep -q "peer=\[203.0.113.3:" "$OUT/punch-host.log"  || { echo "   FALLO: el anfitrión no habla con el router del invitado"; fail=1; }
grep -q "peer=\[203.0.113.2:" "$OUT/punch-guest.log" || { echo "   FALLO: el invitado no habla con el router del anfitrión"; fail=1; }

echo
if [ "$fail" -eq 0 ]; then
  echo "RESULTADO: perforación de NAT verificada contra el NAT del kernel."
else
  echo "RESULTADO: FALLO. Logs del relay:"; sed 's/^/   /' "$OUT/relay.log" | tail -20
fi
exit "$fail"
