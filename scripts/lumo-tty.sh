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

set -uo pipefail
# NOTA: NAO usar set -e global. Build quebrado ou binario faltando
# devem logar erro e dormir antes de sair — senao o getty reinicia
# em loop (start-limit-hit).


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
# A11: forca liberacao DRM master.
#
# Outros compositores ou display managers podem segurar /dev/dri/card0.
# Lumo precisa de master pra page-flip funcionar.
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
# W34.24: skip rebuild se binaries fresh (Iced cold start nao precisa re-compile cada login).
# Verifica tambem se binarios sao executaveis.
NEED_BUILD=false
for bin in ./target/release/lumo-wm ./target/release/lumo-bar ./target/release/lumo-desktop; do
    if [[ ! -x "$bin" ]]; then
        NEED_BUILD=true
        break
    fi
done

if [[ "$NEED_BUILD" == true ]]; then
    echo "[1/3] Build lumo-wm (feature drm-backend) + lumo-bar..."
    set +e
    cargo build --release --features lumo-wm/drm-backend --bin lumo-wm --bin lumo-bar 2>&1 | tee /tmp/lumo-build.log
    BUILD_EC=${PIPESTATUS[0]}
    set -e
    if [[ "$BUILD_EC" -ne 0 ]]; then
        echo ""
        echo "================================================================"
        echo "ERRO: Build falhou (exit=$BUILD_EC). Ver /tmp/lumo-build.log"
        echo "Dormindo 10s antes de sair (evita loop de getty)..."
        echo "================================================================"
        sleep 10
        exit 1
    fi
else
    echo "[1/3] Skipping build, binaries already present"
fi

# Verifica se binarios existem apos build (ou skip)
for bin in ./target/release/lumo-wm ./target/release/lumo-bar ./target/release/lumo-desktop; do
    if [[ ! -x "$bin" ]]; then
        echo "ERRO: binario nao encontrado ou nao executavel: $bin"
        sleep 10
        exit 1
    fi
done

# ============================================================
# Env (source de lumo-env.conf).
# Override individual: export VAR=value antes deste script.
# ============================================================
export XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
mkdir -p "$XDG_RUNTIME_DIR"
REPO="$(cd "$(dirname "$0")/.." && pwd)"
ENV_FILE="$HOME/.config/lumo/env.conf"
[ -f "$ENV_FILE" ] || ENV_FILE="$REPO/scripts/install/lumo-env.conf"
set -a; source "$ENV_FILE"; set +a
# A13: LUMO_THEME default light; override: LUMO_THEME=dark ./scripts/lumo-tty.sh
export LUMO_THEME="${LUMO_THEME:-dark}"

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
    echo "Log completo: /tmp/lumo-wm-tty.log"
    echo "================================================================"
    exit $ec
}
trap post_exit EXIT

# W34.25: respawn watchdog lumo-appsd (W34.21 lazy-exit + auto-respawn = reopen rapido).
APPSD_BIN="$REPO/target/release/lumo-appsd"
(
    sleep 5
    W_SOCKET=$(ls -t /run/user/$(id -u)/wayland-* 2>/dev/null | grep -v ".lock" | head -n 1 | xargs basename)
    export WAYLAND_DISPLAY="${W_SOCKET:-wayland-0}"
    export ICED_BACKEND=tiny-skia
    export XDG_RUNTIME_DIR="/run/user/$(id -u)"
    while true; do
        if ! pgrep -f "$APPSD_BIN" > /dev/null; then
            rm -f "/run/user/$(id -u)/lumo-appsd.sock"
            echo "[watchdog] spawning lumo-appsd" >> /tmp/lumo-appsd.log
            nohup "$APPSD_BIN" >> /tmp/lumo-appsd.log 2>&1 < /dev/null &
        fi
        sleep 2
    done
) &

# Garante lumo-appctl no path pro menu Lumo funcionar
export PATH="$PATH:$REPO/target/release"

while true; do
    ./target/release/lumo-wm 2>&1 | tee /tmp/lumo-wm-tty.log
    LUMO_EC=${PIPESTATUS[0]}
    echo "[hot-restart] lumo-wm exit code=$LUMO_EC"
    if [[ -f /tmp/lumo-no-restart ]]; then
        rm -f /tmp/lumo-no-restart
        echo "[stop] /tmp/lumo-no-restart presente; encerrando loop"
        break
    fi
    sleep 0.5
done
echo "[final] loop encerrado"
