#!/usr/bin/env bash
# Publica los instalables compilados en esta máquina en el release `builds`.
#
# Es el flujo de publicación del proyecto: los binarios no se versionan en git
# y no dependen del CI. Se compilan aquí y se suben con este script.
#
#   scripts/publish-builds.sh            # sube lo último ya compilado
#   scripts/publish-builds.sh --build    # compila Android y Linux antes
#
# Deja en el release tres ficheros de nombre fijo, para que los enlaces de
# descarga no cambien nunca:
#
#   gravital-talk-latest.apk               Android, el que hay que instalar
#   gravital-talk-previous.apk             Android, el anterior (para volver atrás)
#   gravital-talk-latest-linux-x64.tar.gz  Linux x64
#
# Requisitos: `gh` autenticado con permiso de escritura en el repo.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

TAG="builds"
BUILD=0
[ "${1:-}" = "--build" ] && BUILD=1

command -v gh >/dev/null || { echo "falta gh (GitHub CLI)" >&2; exit 2; }
REPO="$(gh repo view --json nameWithOwner -q .nameWithOwner)"
SHA="$(git rev-parse --short HEAD)"
VER="$(grep -m1 '^version:' app/pubspec.yaml | awk '{print $2}' | cut -d+ -f1)"

if [ "$BUILD" -eq 1 ]; then
  echo "[1/4] compilando Linux y Android"
  make app-linux
  ./scripts/build-apk.sh
else
  echo "[1/4] usando lo ya compilado (pasa --build para recompilar)"
fi

APK="outputs/android/release/$(cat outputs/android/release/LATEST.txt 2>/dev/null || true)"
BUNDLE="app/build/linux/x64/release/bundle"
[ -f "$APK" ] || { echo "no hay APK compilado: ejecuta scripts/build-apk.sh" >&2; exit 1; }
[ -x "$BUNDLE/gravital_talk_app" ] || { echo "no hay bundle de Linux: ejecuta make app-linux" >&2; exit 1; }

# Lo que se publica tiene que corresponder al código actual. Subir un APK de
# otro commit con la etiqueta de éste es justo el error que deja a un
# dispositivo "actualizado" con código viejo. Se admite que el APK sea de un
# commit anterior sólo si desde entonces no ha cambiado nada de lo que lleva
# dentro (por ejemplo, tras un commit de documentación).
APK_SHA="$(basename "$APK" | sed -E 's/.*-([0-9a-f]{7,})-release.*/\1/')"
if ! git diff --quiet "$APK_SHA" HEAD -- app crates 2>/dev/null; then
  echo "el APK ($(basename "$APK")) es de $APK_SHA y el código ha cambiado desde entonces: recompila con --build" >&2
  exit 1
fi
SHA="$APK_SHA"
if [ -n "$(git status --porcelain -- app crates)" ]; then
  echo "aviso: hay cambios sin commitear en app/ o crates/; lo publicado puede no coincidir con $SHA" >&2
fi
# La app de escritorio sin motor de audio arranca, pero muda.
# Sin `grep -q`: corta la tubería al primer acierto, `nm` muere por SIGPIPE y
# con `pipefail` eso se leía como "no lleva audio".
if [ "$(nm -D "$BUNDLE/lib/libgravital_talk_ffi.so" 2>/dev/null | grep -c snd_pcm_open)" = 0 ]; then
  echo "el bundle de Linux no lleva audio nativo: recompila con make app-linux" >&2; exit 1
fi

echo "[2/4] empaquetando"
STAGE="$(mktemp -d)"
trap 'rm -rf "$STAGE"' EXIT
mkdir -p outputs/linux
TAR="outputs/linux/gravital-talk-$VER-$SHA-linux-x64.tar.gz"
tar -czf "$TAR" -C "$(dirname "$BUNDLE")" --transform 's,^bundle,gravital-talk,' bundle
basename "$TAR" > outputs/linux/LATEST.txt
cp "$APK" "$STAGE/gravital-talk-latest.apk"
cp "$TAR" "$STAGE/gravital-talk-latest-linux-x64.tar.gz"

echo "[3/4] subiendo a $REPO, release $TAG"
asset_id() { gh api "repos/$REPO/releases/tags/$TAG" -q ".assets[] | select(.name==\"$1\") | .id"; }
LOCAL_SHA="$(sha256sum "$STAGE/gravital-talk-latest.apk" | cut -d' ' -f1)"
REMOTE_SHA="$(gh api "repos/$REPO/releases/tags/$TAG" -q '.assets[] | select(.name=="gravital-talk-latest.apk") | .digest' | sed 's/^sha256://')"
if [ "$LOCAL_SHA" = "$REMOTE_SHA" ]; then
  # Ya publicado: rotar otra vez dejaría `previous` igual que `latest` y se
  # perdería la versión a la que volver.
  echo "  el APK publicado ya es éste: no se rota el anterior"
else
  PREV="$(asset_id gravital-talk-previous.apk)"
  LATEST="$(asset_id gravital-talk-latest.apk)"
  [ -n "$PREV" ] && gh api -X DELETE "repos/$REPO/releases/assets/$PREV" >/dev/null
  [ -n "$LATEST" ] && gh api -X PATCH "repos/$REPO/releases/assets/$LATEST" \
    -f name=gravital-talk-previous.apk >/dev/null
fi
gh release upload "$TAG" "$STAGE/gravital-talk-latest.apk" \
  "$STAGE/gravital-talk-latest-linux-x64.tar.gz" --clobber
gh release edit "$TAG" --notes "Builds instalables de Gravital Talk. Se reemplazan en cada publicación.

| Fichero | Qué es |
|---|---|
| \`gravital-talk-latest.apk\` | Android, último build (commit \`$SHA\`) |
| \`gravital-talk-previous.apk\` | Android, el anterior, para volver atrás |
| \`gravital-talk-latest-linux-x64.tar.gz\` | Linux x64: descomprimir y ejecutar \`gravital-talk/gravital_talk_app\` |

Los dos dispositivos de una conversación tienen que tener el mismo build. El APK va firmado con la clave de depuración." >/dev/null

echo "[4/4] verificando"
PUBLISHED="$(gh api "repos/$REPO/releases/tags/$TAG" -q '.assets[] | select(.name=="gravital-talk-latest.apk") | .digest' | sed 's/^sha256://')"
[ "$PUBLISHED" = "$LOCAL_SHA" ] || { echo "el APK publicado no coincide con el local" >&2; exit 1; }
echo "publicado $SHA:"
gh api "repos/$REPO/releases/tags/$TAG" -q '.assets[] | "  \(.name)  \(.size) B"'
echo "descarga: https://github.com/$REPO/releases/download/$TAG/gravital-talk-latest.apk"
