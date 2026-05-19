#!/usr/bin/env bash
# start-lumo-nested.sh - garante que lumo-wm esta acessivel pra agente LLM
#
# Contexto importante (2026-05-19):
# - lumo-wm tem 2 backends: winit (nested) e drm (TTY direto).
# - O Cargo.toml NAO tem feature "winit-backend" -- winit eh o default (sempre
#   compilado). O LUMO_WM_BACKEND=winit roda nested em outro Wayland host.
# - Nested winit precisa de WAYLAND_DISPLAY do host. Hoje (2026-05-19) NAO ha
#   sessao Wayland de host: a propria Lumo eh a sessao ativa (DRM em tty3).
# - Por isso esse script detecta a sessao live e exporta WAYLAND_DISPLAY pra
#   bater no socket dela (wayland-1). Se um dia rodar Hyprland/Sway por baixo,
#   adicionar branch nested via LUMO_HARNESS_MODE=nested.
#
# Saidas:
#   /tmp/lumo-agent/env.sh   -- exporta WAYLAND_DISPLAY e XDG_RUNTIME_DIR
#   stdout                    -- socket e PID
set -euo pipefail

HARNESS_DIR="/tmp/lumo-agent"
mkdir -p "$HARNESS_DIR"

MODE="${LUMO_HARNESS_MODE:-live}"
RUNTIME="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"

case "$MODE" in
    live)
        WM_PID=$(pgrep -u "$USER" -f 'target/release/lumo-wm' | head -1 || true)
        if [ -z "$WM_PID" ]; then
            echo "ERRO: nenhum lumo-wm rodando. Inicie no tty3 com lumo-tty.sh ou rode com LUMO_HARNESS_MODE=nested" >&2
            exit 1
        fi
        SOCKET=""
        for sock in "$RUNTIME"/wayland-*; do
            [ -S "$sock" ] || continue
            case "$sock" in *.lock) continue;; esac
            SOCKET=$(basename "$sock")
            break
        done
        if [ -z "$SOCKET" ]; then
            echo "ERRO: nao achei socket wayland em $RUNTIME" >&2
            exit 1
        fi
        ;;
    nested)
        echo "ERRO: modo nested ainda nao implementado." >&2
        echo "Bloqueio: lumo-wm.rs faz auto-spawn de bar/desktop apenas no path DRM." >&2
        echo "Pra nested validar, preciso (a) host wayland session ativa OU (b) patch em lumo-wm.rs" >&2
        echo "pra permitir LUMO_AUTOSTART=1 forcar spawn no path winit." >&2
        exit 2
        ;;
    *)
        echo "ERRO: LUMO_HARNESS_MODE desconhecido: $MODE" >&2
        exit 1
        ;;
esac

cat > "$HARNESS_DIR/env.sh" <<EOF
# Auto-gerado por start-lumo-nested.sh em $(date -Iseconds)
export WAYLAND_DISPLAY=$SOCKET
export XDG_RUNTIME_DIR=$RUNTIME
export YDOTOOL_SOCKET=$RUNTIME/.ydotool_socket
EOF

echo "mode=$MODE"
echo "socket=$SOCKET"
echo "runtime=$RUNTIME"
echo "wm_pid=$WM_PID"
echo "env_file=$HARNESS_DIR/env.sh"
