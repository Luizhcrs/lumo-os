#!/usr/bin/env bash
# click.sh - injeta click em coordenadas absolutas via ydotool/uinput
#
# Uso:
#   click.sh <x> <y> [button]
#   button: left (default) | right | middle
#
# Implementacao:
# - ydotool mousemove --absolute manda evento uinput EV_ABS direto pro kernel.
# - lumo-wm (smithay backend_drm + libinput) pega o evento de volta como
#   pointer real, sem precisar de IPC custom.
# - Pra winit nested rodando em outro host, ydotool tambem injeta no host ->
#   funciona se a janela nested ocupar a regiao clicada (NAO testado ainda).
#
# Decisao de design (2026-05-19):
# Considerei adicionar LumoCommand::SyntheticPointer no ipc.rs. NAO necessario:
# ydotool ja resolve. Se um dia precisar mover mouse SEM virtual device (ex:
# CI headless sem /dev/uinput), ai vale a pena patch IPC + crates/compositor/
# lumo-wm/src/input/ pra aceitar eventos sinteticos no SeatState.
set -euo pipefail

HARNESS_DIR="/tmp/lumo-agent"
if [ -f "$HARNESS_DIR/env.sh" ]; then
    # shellcheck disable=SC1091
    . "$HARNESS_DIR/env.sh"
fi

if [ "$#" -lt 2 ]; then
    echo "uso: click.sh <x> <y> [left|right|middle]" >&2
    exit 1
fi

X="$1"
Y="$2"
BTN="${3:-left}"

case "$BTN" in
    left)   CODE="C0"  ;;  # 0x00 = LEFT em ydotool click
    right)  CODE="C1"  ;;
    middle) CODE="C2"  ;;
    *)      echo "botao invalido: $BTN" >&2; exit 1 ;;
esac

# Move primeiro, depois click. mousemove --absolute aceita -x -y diretos.
ydotool mousemove --absolute -x "$X" -y "$Y"
sleep 0.05
# ydotool click: 0xC0=left down+up, 0xC1=right, 0xC2=middle.
ydotool click "$CODE"
