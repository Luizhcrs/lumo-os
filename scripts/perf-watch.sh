#!/usr/bin/env bash
# scripts/perf-watch.sh - tail log lumo-wm e exibe histograma de CPU/RSS (W11.D).
#
# Uso: ./scripts/perf-watch.sh [--pid PID] [--log /path/to/lumo-wm.log]
#
# Sem args: le stdin (pipe de journalctl ou RUST_LOG redirect).
# Plota histograma ASCII de cpu_pct e rss_mb no terminal.

set -euo pipefail

PID=""
LOG_FILE=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --pid) PID="$2"; shift 2 ;;
        --log) LOG_FILE="$2"; shift 2 ;;
        *) echo "uso: $0 [--pid PID] [--log FILE]" >&2; exit 1 ;;
    esac
done

declare -a cpu_samples=()
declare -a rss_samples=()

parse_line() {
    local line="$1"
    if [[ "$line" =~ cpu_pct=([0-9.]+) ]]; then
        cpu_samples+=("${BASH_REMATCH[1]}")
    fi
    if [[ "$line" =~ rss_mb=([0-9.]+) ]]; then
        rss_samples+=("${BASH_REMATCH[1]}")
    fi
}

print_histogram() {
    local label="$1"
    shift
    local values=("$@")
    if [[ ${#values[@]} -eq 0 ]]; then
        echo "  $label: sem dados"
        return
    fi
    echo ""
    echo "  $label (${#values[@]} amostras):"
    # Calcula min/max/avg simples
    local min="${values[0]}" max="${values[0]}" sum=0
    for v in "${values[@]}"; do
        sum=$(echo "$sum + $v" | bc -l 2>/dev/null || echo "$sum")
        # comparacao float via bc
        if (( $(echo "$v < $min" | bc -l 2>/dev/null || echo 0) )); then min="$v"; fi
        if (( $(echo "$v > $max" | bc -l 2>/dev/null || echo 0) )); then max="$v"; fi
    done
    local avg
    avg=$(echo "scale=2; $sum / ${#values[@]}" | bc -l 2>/dev/null || echo "?")
    echo "    min=$min  max=$max  avg=$avg"

    # Histogram de 10 buckets
    local range
    range=$(echo "$max - $min" | bc -l 2>/dev/null || echo "1")
    if (( $(echo "$range <= 0" | bc -l 2>/dev/null || echo 1) )); then range="1"; fi
    local buckets=10
    declare -A counts
    for i in $(seq 0 $((buckets - 1))); do counts[$i]=0; done
    for v in "${values[@]}"; do
        local idx
        idx=$(echo "scale=0; ($v - $min) / $range * ($buckets - 1) / 1" | bc -l 2>/dev/null || echo 0)
        idx=${idx%%.*}
        idx=${idx:-0}
        if [[ $idx -ge $buckets ]]; then idx=$((buckets - 1)); fi
        counts[$idx]=$((${counts[$idx]:-0} + 1))
    done
    local max_count=1
    for i in $(seq 0 $((buckets - 1))); do
        local c=${counts[$i]:-0}
        if [[ $c -gt $max_count ]]; then max_count=$c; fi
    done
    echo "    histograma:"
    for i in $(seq 0 $((buckets - 1))); do
        local bucket_min
        bucket_min=$(echo "scale=1; $min + $i * $range / $buckets" | bc -l 2>/dev/null || echo "?")
        local c=${counts[$i]:-0}
        local bar_len=$(( c * 30 / max_count ))
        local bar
        bar=$(printf '%0.s#' $(seq 1 $bar_len 2>/dev/null) 2>/dev/null || echo "")
        printf "    %8s | %-30s %d\n" "$bucket_min" "$bar" "$c"
    done
}

print_report() {
    clear
    echo "=== lumo-wm perf-watch W11.D =============================="
    echo "  Targets: idle CPU < 1%, RSS < 200MB, frame p95 < 16ms"
    echo "  Amostras coletadas: cpu=${#cpu_samples[@]} rss=${#rss_samples[@]}"
    print_histogram "CPU usage %" "${cpu_samples[@]:-}"
    print_histogram "RSS MB" "${rss_samples[@]:-}"
    echo ""
    echo "  Ultima atualizacao: $(date '+%H:%M:%S')"
    echo "==========================================================="
}

REPORT_INTERVAL=5
last_report=0

process_log() {
    local source="${1:--}"
    while IFS= read -r line; do
        parse_line "$line"
        local now
        now=$(date +%s)
        if (( now - last_report >= REPORT_INTERVAL )); then
            print_report
            last_report=$now
        fi
    done < "$source"
}

if [[ -n "$LOG_FILE" ]]; then
    process_log <(tail -f "$LOG_FILE")
elif [[ -n "$PID" ]]; then
    echo "Monitorando PID=$PID via /proc/$PID..."
    while kill -0 "$PID" 2>/dev/null; do
        local_cpu=$(grep "^cpu " /proc/stat 2>/dev/null | awk '{print $2+$3+$4+$5}' || echo 0)
        proc_stat=$(cat "/proc/$PID/stat" 2>/dev/null || echo "")
        if [[ -n "$proc_stat" ]]; then
            utime=$(echo "$proc_stat" | awk '{print $14}')
            stime=$(echo "$proc_stat" | awk '{print $15}')
            total=$((utime + stime))
            echo "cpu_pct=0 rss_mb=0 proc_total=$total"
        fi
        sleep 60
    done | process_log -
else
    echo "Aguardando log via stdin (CTRL+C para sair)..."
    echo "Dica: RUST_LOG=lumo_wm=info ./lumo-wm 2>&1 | ./scripts/perf-watch.sh"
    process_log -
fi
