#!/bin/bash
# Lumo OS - start Hyprland session no TTY3.
# Pivot 2026-05-23: substitui lumo-tty.sh / lumo-wm.

set -euo pipefail

# Safety: precisa TTY real, nao SSH.
if [[ ! -t 0 ]] || [[ ! -t 1 ]]; then
    echo "ERRO: rode DENTRO de TTY fisico (Ctrl+Alt+F3)."
    exit 1
fi

# Kill old lumo-wm + bar if running.
pkill -f "target/release/lumo-wm" 2>/dev/null || true
pkill -f "target/release/lumo-bar" 2>/dev/null || true
pkill -f "target/release/lumo-desktop" 2>/dev/null || true
pkill -f "target/release/lumo-osd" 2>/dev/null || true
sleep 1

# Env.
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
export XDG_CURRENT_DESKTOP=Hyprland
export XDG_SESSION_TYPE=wayland
export XDG_SESSION_DESKTOP=Hyprland
export QT_QPA_PLATFORM=wayland
export GDK_BACKEND=wayland
export MOZ_ENABLE_WAYLAND=1

# Hyprland precisa wlr-randr ou similar pra DRM.
# Reuse existing logind session.

echo "[lumo] Starting Hyprland..."
exec Hyprland
