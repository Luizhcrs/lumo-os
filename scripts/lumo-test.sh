#!/usr/bin/env bash
# lumo-test.sh — inicia compositor + 2 clientes em um comando
set -euo pipefail

cd ~/Projects/lumo-shell

echo "[1/4] Build lumo-wm + lumo-bar..."
cargo build --release -p lumo-wm --bin lumo-wm 2>&1 | tail -3
cargo build --release -p lumo-shell --bin lumo-bar 2>&1 | tail -3

LOG=/tmp/lumo-wm.log
> "$LOG"

echo "[2/4] Inicia lumo-wm em background..."
# A13: theme default LIGHT (override com LUMO_THEME=dark ./scripts/lumo-test.sh).
export LUMO_THEME="${LUMO_THEME:-dark}"
RUST_LOG=lumo_wm=info,smithay=warn ./target/release/lumo-wm >"$LOG" 2>&1 &
LUMO_PID=$!
trap "kill $LUMO_PID 2>/dev/null; pkill -P $LUMO_PID 2>/dev/null; echo Fechado." EXIT

# Espera socket aparecer no log
echo "[3/4] Aguardando socket Wayland..."
for i in {1..30}; do
    SOCKET=$(grep -oP 'WAYLAND_DISPLAY=\Kwayland-\d+' "$LOG" 2>/dev/null | head -1 || true)
    if [ -n "$SOCKET" ]; then break; fi
    sleep 0.2
done

if [ -z "$SOCKET" ]; then
    echo "ERRO: socket nao apareceu em 6s. Log:"
    cat "$LOG"
    exit 1
fi

echo "[4/4] Compositor pronto em $SOCKET. Lancando clientes..."
sleep 0.5

# Lança lumo-bar (layer-shell top)
WAYLAND_DISPLAY=$SOCKET ./target/release/lumo-bar >/tmp/lumo-bar.log 2>&1 &
BAR_PID=$!

sleep 0.3

# Lança foot (cliente normal)
WAYLAND_DISPLAY=$SOCKET foot >/tmp/foot.log 2>&1 &
FOOT_PID=$!

echo ""
echo "=== LUMO OS rodando ==="
echo "lumo-wm  PID $LUMO_PID  socket $SOCKET"
echo "lumo-bar PID $BAR_PID"
echo "foot     PID $FOOT_PID"
echo ""
echo "Logs:"
echo "  lumo-wm  : tail -f $LOG"
echo "  lumo-bar : tail -f /tmp/lumo-bar.log"
echo "  foot     : tail -f /tmp/foot.log"
echo ""
echo "Ctrl+C aqui pra fechar tudo."

# Aguarda lumo-wm terminar (Ctrl+C propaga)
wait $LUMO_PID
