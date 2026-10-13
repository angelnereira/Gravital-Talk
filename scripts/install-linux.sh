#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────
# Instala en este equipo la app de escritorio ya compilada.
#
#   make app-linux && scripts/install-linux.sh
#   make install-linux                          # las dos cosas
#
# Copia el bundle a ~/.local/opt/gravital-talk y deja la entrada del menú
# de aplicaciones apuntando ahí (la misma que escribe la app al arrancar,
# para que no la pise). No necesita permisos de administrador.
# ─────────────────────────────────────────────────────────────────────
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUNDLE="$ROOT/app/build/linux/x64/release/bundle"
DEST="${GRAVITAL_TALK_PREFIX:-$HOME/.local/opt/gravital-talk}"
DATA="${XDG_DATA_HOME:-$HOME/.local/share}"

if [ ! -x "$BUNDLE/gravital_talk_app" ]; then
  echo "No hay bundle compilado en $BUNDLE. Ejecuta antes: make app-linux" >&2
  exit 1
fi
if pgrep -x gravital_talk_a >/dev/null; then
  echo "Gravital Talk está abierta. Ciérrala y vuelve a ejecutar esto." >&2
  exit 1
fi

rm -rf "$DEST.new"
mkdir -p "$(dirname "$DEST")"
cp -a "$BUNDLE" "$DEST.new"
rm -rf "$DEST"
mv "$DEST.new" "$DEST"

mkdir -p "$DATA/applications"
printf '%s\n' \
  '[Desktop Entry]' \
  'Type=Application' \
  'Name=Gravital Talk' \
  "Exec=\"$DEST/gravital_talk_app\" %u" \
  'Terminal=false' \
  'Categories=Network;AudioVideo;' \
  'MimeType=x-scheme-handler/gravital-talk;' \
  > "$DATA/applications/gravital-talk.desktop"
xdg-mime default gravital-talk.desktop x-scheme-handler/gravital-talk 2>/dev/null || true
update-desktop-database "$DATA/applications" 2>/dev/null || true

SHA="$(git -C "$ROOT" rev-parse --short HEAD 2>/dev/null || echo '?')"
DIRTY="$(git -C "$ROOT" status --porcelain 2>/dev/null | grep -q . && echo ' + cambios sin commit' || true)"
echo "Gravital Talk instalada en $DEST (commit $SHA$DIRTY)."
echo "Ábrela desde el menú de aplicaciones o con: $DEST/gravital_talk_app"
