#!/usr/bin/env bash
# lumo-tty.sh - Lumo full session em TTY proprio (sem Hyprland em volta).
#
# Preparacao (uma vez):
#   sudo pacman -S seatd libseat libinput
#   sudo systemctl enable --now seatd.socket
#   sudo usermod -aG seat,input,video,render $USER
#   (relogar pra grupos surtirem efeito)
#
# Uso: rode DENTRO de um TTY livre (Ctrl+Alt+F3 num login do Galaxy
# Book 4). NAO funciona via SSH (precisa de TTY real com /dev/tty*).
#
# Como SAIR se travou:
#   Ctrl+Alt+Backspace  -> exit clean do lumo-wm (cuidado: nao tem
#                          confirmacao; salve antes)
#   Ctrl+Alt+F1         -> volta pro TTY1 (Hyprland host normalmente)
#   Ctrl+Alt+F2         -> volta pro TTY2 (display manager)
#   ssh de outra maquina:
#     sudo systemctl restart display-manager
#     OU
#     sudo pkill -9 lumo-wm
#
# Memory feedback_validar_local_antes_push: este script faz build
# antes de rodar (cargo build --release com feature drm-backend).
set -euo pipefail

cd ~/Projects/lumo-shell

if [ ! -t 0 ] || [ ! -t 1 ]; then
    echo "ERRO: precisa rodar DENTRO de TTY fisico (Ctrl+Alt+F3)."
    echo "Detectado: stdin/stdout nao sao TTY (provavelmente SSH/script)."
    exit 1
fi

# Sanity: nao roda dentro do Hyprland host (corrompe sessao).
if [ -n "${WAYLAND_DISPLAY:-}" ] && [ "${WAYLAND_DISPLAY}" != "wayland-lumo" ]; then
    echo "ATENCAO: WAYLAND_DISPLAY=$WAYLAND_DISPLAY ja setado."
    echo "Aparenta que voce esta dentro de outra sessao Wayland."
    echo "Saia primeiro com Ctrl+Alt+F3 ou similar antes de continuar."
    read -p "Continuar mesmo assim? [y/N] " yn
    [ "${yn:-N}" = "y" ] || exit 1
fi

echo "[1/3] Build lumo-wm (feature drm-backend) + lumo-bar..."
cargo build --release --features lumo-wm/drm-backend --bin lumo-wm --bin lumo-bar 2>&1 | tail -10

# Ambiente. XDG_RUNTIME_DIR vem do logind; fallback explicito caso
# tty puro sem session manager.
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"

export LUMO_WM_BACKEND=drm
export WAYLAND_DISPLAY=wayland-lumo
export RUST_LOG="${RUST_LOG:-lumo_wm=info,smithay=warn}"

LOG="$XDG_RUNTIME_DIR/lumo-wm-tty.log"
> "$LOG"

echo "[2/3] Lancando lumo-wm em DRM (output em $LOG)..."
echo "[3/3] Se a tela ficar preta > 5s, o watchdog mata o processo."
echo

# exec pra que sinais (Ctrl+C) cheguem direto ao binario.
exec ./target/release/lumo-wm 2>&1 | tee "$LOG"
