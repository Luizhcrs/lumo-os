#!/bin/bash
# scripts/safety-check.sh — smoke test de memory leaks via valgrind.
#
# Roda uma sessao de 30s do lumo-wm sob valgrind e reporta leaks definitivos.
# Requer: valgrind instalado, build release disponivel.
# Uso: ver scripts/install/README.md secao "Safety smoke test".

set -euo pipefail

BINARY="${1:-./target/release/lumo-wm}"
LOG=/tmp/lumo-valgrind.log

if ! command -v valgrind &>/dev/null; then
    echo "erro: valgrind nao encontrado. Instale com: sudo pacman -S valgrind"
    exit 1
fi

if [ ! -x "$BINARY" ]; then
    echo "erro: binario nao encontrado em $BINARY"
    echo "Execute: cargo build --release -p lumo-wm"
    exit 1
fi

echo "=== Lumo OS — safety smoke test ==="
echo "Binario: $BINARY"
echo "Log:     $LOG"
echo "Duracao: 30s"
echo ""

valgrind \
    --leak-check=full \
    --show-leak-kinds=definite \
    --track-origins=yes \
    --error-exitcode=1 \
    timeout 30 "$BINARY" 2>&1 | tee "$LOG" || true

echo ""
echo "=== definitely lost ==="
grep "definitely lost" "$LOG" || echo "(nenhum leak definitivo encontrado)"

echo ""
echo "=== resumo ==="
grep "LEAK SUMMARY" -A 6 "$LOG" || echo "(sem sumario de leak)"

echo ""
echo "Log completo em: $LOG"
