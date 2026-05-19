#!/usr/bin/env bash
# lumo-dev.sh - Dev rapido: lumo-wm nested winit dentro Hyprland host.
#
# A19: substitui ciclo de TTY switching pra iterar visual rapido. Aceita
# perda de fidelidade DRM (cor pipeline pode diferir, HDR/dither do output
# real ficam fora) em troca de feedback em segundos.
#
# Workflow dual:
#   ./scripts/lumo-dev.sh   -- debug live nested (este)
#   ./scripts/lumo-tty.sh   -- polish final, DRM real em TTY3
#
# Uso: rodar em QUALQUER terminal dentro do Hyprland host.
# Sair: Ctrl+C no terminal mata lumo-wm.

set -euo pipefail

cd "$(dirname "$0")/.."

echo "[lumo-dev] build release..."
cargo build --release --bin lumo-wm --bin lumo-bar 2>&1 | tail -3

echo "[lumo-dev] subindo lumo-wm winit (janela ~1280x720 dentro Hyprland)..."
export LUMO_WM_BACKEND=winit
export RUST_LOG="${RUST_LOG:-lumo_wm=info,smithay=warn}"
export LUMO_THEME="${LUMO_THEME:-dark}"

# Cleanup eventual instancia anterior pra evitar zombie nested.
pkill -x lumo-wm 2>/dev/null || true
pkill -x lumo-bar 2>/dev/null || true
sleep 0.3

./target/release/lumo-wm 2>&1 | tee /tmp/lumo-dev.log &
LUMO_WM_PID=$!

# Aguarda socket wayland do lumo-wm aparecer (winit cria wayland-N).
SOCKET=""
for i in {1..20}; do
    if compgen -G "$XDG_RUNTIME_DIR/wayland-*" > /dev/null; then
        SOCKET=$(ls -t $XDG_RUNTIME_DIR/wayland-* 2>/dev/null | grep -v '.lock' | head -1 | xargs basename)
        if [[ -n "$SOCKET" ]]; then
            echo "[lumo-dev] socket pronto: $SOCKET"
            break
        fi
    fi
    sleep 0.2
done

if [[ -z "$SOCKET" ]]; then
    echo "[lumo-dev] [warn] socket nao detectado em 4s -- lumo-wm pode ter morrido. Veja /tmp/lumo-dev.log"
fi

echo "[lumo-dev] lumo-wm rodando pid=$LUMO_WM_PID, socket=$SOCKET"
echo "[lumo-dev] Pra spawnar foot dentro: WAYLAND_DISPLAY=$SOCKET foot &"
echo "[lumo-dev] Ctrl+C aqui mata tudo."

trap 'echo "[lumo-dev] cleanup..."; pkill -P $$ 2>/dev/null || true; kill $LUMO_WM_PID 2>/dev/null || true' EXIT INT TERM
wait $LUMO_WM_PID
