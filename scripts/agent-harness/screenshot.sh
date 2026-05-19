#!/usr/bin/env bash
# screenshot.sh - captura tela via grim (wlr-screencopy)
#
# Uso:
#   screenshot.sh              -> /tmp/lumo-agent/<unix_ns>.png, imprime path
#   screenshot.sh nome.png     -> /tmp/lumo-agent/nome.png
#
# Requer env.sh gerado por start-lumo-nested.sh.
set -euo pipefail

HARNESS_DIR="/tmp/lumo-agent"
mkdir -p "$HARNESS_DIR"

if [ -f "$HARNESS_DIR/env.sh" ]; then
    # shellcheck disable=SC1091
    . "$HARNESS_DIR/env.sh"
fi

NAME="${1:-$(date +%s%N).png}"
OUT="$HARNESS_DIR/$NAME"

grim "$OUT"
echo "$OUT"
