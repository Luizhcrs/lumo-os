#!/usr/bin/env bash
# lumo-tty.sh - Lumo full session em TTY proprio (sem Hyprland em volta).
#
# A9 ETAPA 2B: render path real ligado. Lumo precisa ser DRM master --
# Hyprland host nao pode rodar simultaneo. Este script detecta Hyprland
# em execucao e pede pra terminar antes de subir Lumo. Quando Lumo sair,
# da hint pra usuario reabrir Hyprland.
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
#   Ctrl+Alt+F1         -> volta pro TTY1 (Hyprland host -- mas voce
#                          matou ele, vai cair em console; relogue)
#   Ctrl+Alt+F2         -> display manager
#   ssh de outra maquina:
#     sudo pkill -9 lumo-wm
#
# Memory feedback_validar_local_antes_push: build antes de rodar.
# Memory feedback_design_lapidado: rejeita SSH/pts; warning explicito
# antes de matar Hyprland (3s pra cancelar).

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
# Safety 2: detecta Hyprland host e oferece matar.
# Lumo precisa DRM master; se outro compositor segura, render falha.
# ============================================================
HYPRLAND_PID=$(pgrep -x Hyprland || true)
HYPR_WAS_RUNNING=false

if [[ -n "$HYPRLAND_PID" ]]; then
    echo ""
    echo "================================================================"
    echo " AVISO: Hyprland host detectado (PID $HYPRLAND_PID)."
    echo " Lumo precisa de DRM master; Hyprland esta segurando."
    echo " Vou matar Hyprland em 3s pra subir Lumo."
    echo " Cancele com Ctrl+C agora se nao quiser perder o estado."
    echo "================================================================"
    sleep 3

    HYPR_WAS_RUNNING=true

    # Tentativa 1: hyprctl exit (saida limpa, salva estado).
    if command -v hyprctl >/dev/null && hyprctl dispatch exit 2>/dev/null; then
        echo "[info] hyprctl exit chamado"
    else
        # Tentativa 2: SIGTERM (default graceful shutdown).
        kill "$HYPRLAND_PID" 2>/dev/null || true
        echo "[info] SIGTERM enviado pra PID $HYPRLAND_PID"
    fi

    # Aguarda Hyprland sair (ate 5s).
    for i in {1..10}; do
        if ! pgrep -x Hyprland >/dev/null; then
            echo "[info] Hyprland encerrado"
            break
        fi
        sleep 0.5
    done

    # Tentativa 3: SIGKILL (forca bruta).
    if pgrep -x Hyprland >/dev/null; then
        echo "[warn] Hyprland resistente, SIGKILL"
        pkill -KILL -x Hyprland || true
        sleep 1
    fi
fi

# ============================================================
# A11: forca liberacao DRM master.
#
# Mesmo apos Hyprland sair, /dev/dri/card0 pode ficar segurado por
# processos zombies, seatd cache, ou display manager. Lumo precisa
# de master pra page-flip funcionar; sem ele = tela preta.
#
# Fix: lista quem usa, mata todos os processos com fd aberto, espera.
# ============================================================
# Terminar sessoes logind de outros TTYs (Hyprland zombie etc)
echo "[info] terminando sessoes logind de outros TTYs..."
MY_TTY=$(tty | sed "s|/dev/||")
echo "  MY_TTY=$MY_TTY"
# Pega session id via loginctl show-session por SID, evita awk de colunas
# Ativar a propria sessao no logind (libseat depende disso pra DRM master)
MY_SID=$(loginctl list-sessions --no-legend | awk '{print $1}' | while read s; do
    t=$(loginctl show-session "$s" --property=TTY --value 2>/dev/null)
    if [[ "$t" == "$MY_TTY" ]]; then echo "$s"; break; fi
done)
if [[ -n "$MY_SID" ]]; then
    MY_ACTIVE=$(loginctl show-session "$MY_SID" --property=Active --value 2>/dev/null)
    echo "  minha session=$MY_SID active=$MY_ACTIVE"
    if [[ "$MY_ACTIVE" != "yes" ]]; then
        echo "  [warn] session inativa, ativando..."
        sudo loginctl activate "$MY_SID" 2>&1 || echo "  (activate falhou)"
        sleep 1
        MY_ACTIVE=$(loginctl show-session "$MY_SID" --property=Active --value 2>/dev/null)
        echo "  apos activate: active=$MY_ACTIVE"
    fi
else
    echo "  [warn] nao encontrou session id pra $MY_TTY"
fi

for SID in $(loginctl list-sessions --no-legend | awk '{print $1}'); do
    STTY=$(loginctl show-session "$SID" --property=TTY --value 2>/dev/null)
    if [[ -n "$STTY" && "$STTY" != "$MY_TTY" ]]; then
        echo "  terminando session $SID (tty=$STTY)"
        sudo loginctl terminate-session "$SID" 2>/dev/null || true
    else
        echo "  preservando session $SID (tty=$STTY, my_tty=$MY_TTY)"
    fi
done
sleep 1

# Auto-detecta cards existentes (Galaxy so tem card1, outros podem ter card0)
DRM_CARDS=$(ls /dev/dri/card* 2>/dev/null | tr "\n" " ")
if [[ -n "$DRM_CARDS" ]] && command -v lsof >/dev/null && command -v fuser >/dev/null; then
    echo "[info] verificando quem segura $DRM_CARDS..."
    LSOF_BEFORE=$(sudo lsof $DRM_CARDS 2>/dev/null | tail -n +2 || true)
    if [[ -n "$LSOF_BEFORE" ]]; then
        echo "$LSOF_BEFORE"
        echo "[warn] processos seguram DRM. Forcando libera..."
        sudo fuser -k $DRM_CARDS 2>/dev/null || true
        sleep 1
    else
        echo "[info] DRM cards livres"
    fi
fi

# ============================================================
# Safety 3: warn se ainda tem outras sessoes wayland/x11 ativas.
# ============================================================
if [[ -n "${WAYLAND_DISPLAY:-}" ]] && [[ "${WAYLAND_DISPLAY}" != "wayland-lumo" ]]; then
    echo "[warn] WAYLAND_DISPLAY=$WAYLAND_DISPLAY ainda setado (limpando)."
    unset WAYLAND_DISPLAY
fi

if [[ -n "${DISPLAY:-}" ]]; then
    echo "[warn] DISPLAY=$DISPLAY (X11 antigo, limpando)."
    unset DISPLAY
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

# A13: theme default LIGHT (Luiz pediu). Override com:
#   LUMO_THEME=dark ./scripts/lumo-tty.sh
export LUMO_THEME="${LUMO_THEME:-light}"

echo "[2/3] TTY = $current_tty, user = $(id -un)"
echo "[3/3] Iniciando lumo-wm DRM..."
echo ""
echo "  Sair limpo:           Ctrl+Alt+Backspace"
echo "  Voltar TTY1:          Ctrl+Alt+F1 (Hyprland esta morto, vai dar console)"
echo "  Display manager:      Ctrl+Alt+F2"
echo ""

# ============================================================
# Trap de saida: reabrir Hyprland se foi morto, OU pelo menos avisar.
# ============================================================
post_exit() {
    ec=$?
    echo ""
    echo "================================================================"
    echo "lumo-wm saiu com code=$ec"
    if [[ "$HYPR_WAS_RUNNING" == "true" ]]; then
        echo ""
        echo " Hyprland foi morto pra subir Lumo. Pra reabrir:"
        echo "   1. Ctrl+Alt+F1 + login fresh (recomendado, sessao limpa)"
        echo "   2. OU rode: nohup Hyprland > /tmp/hypr.log 2>&1 &"
        echo ""
    fi
    echo "Log completo: /tmp/lumo-wm-tty.log"
    echo "================================================================"
    exit $ec
}
trap post_exit EXIT

./target/release/lumo-wm 2>&1 | tee /tmp/lumo-wm-tty.log
LUMO_EC=${PIPESTATUS[0]}
echo "[final] lumo-wm exit code=$LUMO_EC"
