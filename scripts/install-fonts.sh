#!/usr/bin/env bash
# install-fonts.sh - instala Geist Sans + Geist Mono (Vercel open source).
#
# Lumo OS UI usa Geist Sans em text/menus/dropdowns e Geist Mono em
# clock + numeros (calendar, workspace). Fontes instaladas em
# ~/.local/share/fonts pra ficar user-local (sem sudo).

set -euo pipefail

FONTS_DIR="$HOME/.local/share/fonts"
GEIST_DIR="$FONTS_DIR/geist"
mkdir -p "$GEIST_DIR"

TMP_DIR="$(mktemp -d)"
trap "rm -rf $TMP_DIR" EXIT

echo "[install-fonts] resolvendo release Vercel..."
ASSET_URL="$(curl -sL https://api.github.com/repos/vercel/geist-font/releases/latest \
  | grep -oE "https://[^\"]+geist-font-[0-9.]+\.zip" | head -1)"
if [ -z "$ASSET_URL" ]; then
  echo "[install-fonts] erro: nao achei asset zip no release latest" >&2
  exit 1
fi
echo "[install-fonts] baixando $ASSET_URL..."

cd "$TMP_DIR"
curl -fL --retry 3 -o geist.zip "$ASSET_URL"

echo "[install-fonts] descompactando em $GEIST_DIR..."
unzip -o -q geist.zip -d "$GEIST_DIR"

echo "[install-fonts] reindex fc-cache..."
fc-cache -f "$FONTS_DIR" >/dev/null

echo "[install-fonts] instaladas. Faces detectadas:"
fc-list | grep -i geist | head -10
