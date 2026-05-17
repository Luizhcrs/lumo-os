#!/usr/bin/env bash
# lumo-tty.sh - Lumo full session em TTY proprio (sem Hyprland em volta).
#
# A9 ETAPA 2A: session + libinput + DRM enumeration real (nao tem
# render path completo ainda, ver crates/compositor/lumo-wm/src/backend/drm.rs
# header). Mesmo assim ja roda em TTY3 e captura input via libinput.
#
# Preparacao (uma vez):
#   sudo pacman -S seatd libseat libinput
#   sudo systemctl enable --now seatd.socket
#   sudo usermod -aG seat,input,video,render $USER
#   sudo ./scripts/setup-autologin.sh   # autologin no TTY3
#   (relogar pra grupos surtirem efeito)
#
# Uso: rode DENTRO de um TTY livre (Ctrl+Alt+F3). NAO funciona via SSH
# (precisa de TTY real com /dev/tty*).
#
# Como SAIR se travou:
#   Ctrl+Alt+Backspace  -> exit clean do lumo-wm
#   Ctrl+Alt+F1         -> volta pro TTY1 (Hyprland host)
#   Ctrl+Alt+F2         -> volta pro TTY2 (display manager)
#   ssh de outra maquina:
#     sudo pkill -9 lumo-wm
#
# Memory feedback_validar_local_antes_push: build antes de rodar.
# Memory feedback_design_lapidado: rejeita SSH/pts pra evitar corromper
# sessao grafica ativa.

set -euo pipefail

# ============================================================
# Safety 1: rejeitar se nao for TTY real (SSH, pts, etc).
# ============================================================
if [[ ! -t 0 ]] || [[ ! -t 1 ]]; then
    echo "ERRO: stdin/stdout nao sao TTY (provavelmente SSH/script)."
    echo "Rode DENTRO de TTY fisico (Ctrl+Alt+F3 no console)."
    exit 1
fi

current_tty="$(tty 2>/dev/null || echo unknown)"
if [[ "$current_tty" =~ pts ]]; then
    echo "ERRO: tty atual = $current_tty (pty)."
    echo "Precisa de TTY real (/dev/tty3). Use Ctrl+Alt+F3."
    exit 1
fi

# ============================================================
# Safety 2: rejeitar se ja existe sessao grafica ativa.
# ============================================================
if [[ -n "${WAYLAND_DISPLAY:-}" ]] && [[ "${WAYLAND_DISPLAY}" != "wayland-lumo" ]]; then
    echo "ATENCAO: WAYLAND_DISPLAY=$WAYLAND_DISPLAY ja setado."
    echo "Aparenta que voce esta dentro de outra sessao Wayland."
    echo "Saia primeiro com Ctrl+Alt+F3 ou similar antes de continuar."
    read -p "Continuar mesmo assim? [y/N] " yn
    [[ "${yn:-N}" == "y" ]] || exit 1
fi

if [[ -n "${DISPLAY:-}" ]]; then
    echo "ATENCAO: DISPLAY=$DISPLAY ja setado (sessao X11 ativa)."
    read -p "Continuar mesmo assim? [y/N] " yn
    [[ "${yn:-N}" == "y" ]] || exit 1
fi

cd "$(dirname "$0")/.."

# ============================================================
# Build com feature drm-backend (idempotente, cache cargo).
# ============================================================
echo "[1/3] Build lumo-wm (feature drm-backend) + lumo-bar..."
cargo build --release --features lumo-wm/drm-backend --bin lumo-wm --bin lumo-bar 2>&1 | tail -6

# ============================================================
# Env.
# ============================================================
export LUMO_WM_BACKEND=drm
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
mkdir -p "$XDG_RUNTIME_DIR"
export XDG_SESSION_TYPE=wayland
export XDG_CURRENT_DESKTOP=lumo
export WAYLAND_DISPLAY=wayland-lumo
export RUST_LOG="${RUST_LOG:-lumo_wm=info,smithay=warn,wgpu=warn}"

echo "[2/3] TTY = $current_tty, user = $(id -un), groups = $(id -Gn)"
echo "[3/3] Iniciando lumo-wm DRM..."
echo ""
echo "  Sair limpo:        Ctrl+Alt+Backspace"
echo "  Voltar Hyprland:   Ctrl+Alt+F1"
echo "  Display manager:   Ctrl+Alt+F2"
echo ""

trap 'ec=$?; echo "lumo-wm saiu com code=$ec"; exit $ec' EXIT

exec ./target/release/lumo-wm
