#!/usr/bin/env bash
# scripts/perf-baseline.sh - M2 perf baseline measurement (W6.D)
#
# Captura 5min de sessao Lumo WM, extrai p50/p95/p99 do log tracing,
# mede RSS e exibe resultado.
#
# Uso:
#   ./scripts/perf-baseline.sh
#
# Criterios de aceite (roadmap M2):
#   frame_time_p95_ms < 16    (< 16.7ms = 60fps threshold)
#   frame_time_p50_ms < 16    (media dentro de budget)
#   RAM RSS lumo-wm < 500MB

set -euo pipefail

LOG=/tmp/lumo-perf-$(date +%Y%m%d-%H%M%S).log
WM_BIN="$(dirname "$0")/../target/release/lumo-wm"

if [ ! -x "$WM_BIN" ]; then
    echo "ERRO: $WM_BIN nao encontrado. Rode cargo build --release --workspace primeiro."
    exit 1
fi

echo "=== Lumo M2 Perf Baseline ==="
echo "Capturando 5min de sessao. Log: $LOG"
echo "Pressione Ctrl+C para encerrar antes dos 5min."
echo ""

timeout 300 "$WM_BIN" 2>&1 | tee "$LOG" || true

echo ""
echo "=== p50 / p95 / p99 frame_time (us) ==="
grep "frame_time_p" "$LOG" | tail -10 || echo "(nenhuma amostra -- sessao muito curta)"

echo ""
echo "=== p50 / p95 / p99 input_latency (us) ==="
grep "input_latency_p" "$LOG" | tail -10 || echo "(nenhuma amostra)"

echo ""
echo "=== RAM RSS lumo-wm ==="
if pgrep lumo-wm > /dev/null 2>&1; then
    pmap -x "$(pgrep lumo-wm | head -1)" | tail -1
else
    echo "(lumo-wm nao esta rodando; inicie uma sessao e rode pmap -x \$(pgrep lumo-wm) | tail -1)"
fi

echo ""
echo "=== Criterios M2 ==="
# Extrai p95_ms do ultimo log line
P95_MS=$(grep "frame_time_p95_ms=" "$LOG" 2>/dev/null | tail -1 | grep -oP 'frame_time_p95_ms=\K[0-9]+' || echo "")
if [ -n "$P95_MS" ]; then
    if [ "$P95_MS" -lt 16 ]; then
        echo "[ok] frame_time p95 = ${P95_MS}ms (< 16ms)"
    else
        echo "[fail] frame_time p95 = ${P95_MS}ms (>= 16ms)"
    fi
else
    echo "[?] frame_time p95 = N/A (amostras insuficientes)"
fi

echo ""
echo "Log completo em: $LOG"
