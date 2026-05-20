#!/usr/bin/env bash
# Lumo OS E2E test runner
# Usage: bash run_all.sh [--report-path /tmp/my-report.md]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TIMESTAMP="$(date +%Y-%m-%d_%H-%M-%S)"
DATE_DAY="$(date +%Y-%m-%d)"
REPORT_PATH="/tmp/e2e-report-${DATE_DAY}.md"
TIMEOUT=30
PYTHON="${HOME}/.local/bin/python3"
# fallback to system python3
command -v "$PYTHON" &>/dev/null || PYTHON="$(command -v python3)"

# parse args
while [[ $# -gt 0 ]]; do
    case "$1" in
        --report-path) REPORT_PATH="$2"; shift 2 ;;
        *) echo "unknown arg: $1"; exit 1 ;;
    esac
done

mkdir -p /tmp/e2e

TESTS=(
    "test_calc.py"
    "test_editor.py"
    "test_files.py"
    "test_monitor.py"
    "test_notes.py"
    "test_settings.py"
    "test_store.py"
)

PASS=0
FAIL=0
SKIP=0
declare -A RESULTS
declare -A TIMES
declare -A NOTES_MAP

run_test() {
    local file="$1"
    local name="${file%.py}"
    local binary="lumo-${name#test_}"
    local bin_path="${HOME}/Projects/lumo-shell/target/release/${binary}"

    # Skip if binary missing
    if [[ ! -f "$bin_path" ]]; then
        RESULTS[$name]="SKIP"
        NOTES_MAP[$name]="binary not found: $bin_path"
        ((SKIP++)) || true
        return
    fi

    local start
    start=$(date +%s%3N)
    local out_file="/tmp/e2e/${name}.out"

    set +e
    timeout "${TIMEOUT}" "$PYTHON" "${SCRIPT_DIR}/${file}" > "$out_file" 2>&1
    local exit_code=$?
    set -e

    local end
    end=$(date +%s%3N)
    local elapsed_ms=$(( end - start ))
    local elapsed_s
    elapsed_s=$(echo "scale=1; ${elapsed_ms} / 1000" | bc 2>/dev/null || echo "${elapsed_ms}ms")
    TIMES[$name]="${elapsed_s}s"

    if [[ $exit_code -eq 0 ]]; then
        RESULTS[$name]="PASS"
        NOTES_MAP[$name]="$(tail -3 "$out_file" | tr '\n' ' ')"
        ((PASS++)) || true
    elif [[ $exit_code -eq 124 ]]; then
        RESULTS[$name]="FAIL"
        NOTES_MAP[$name]="timeout after ${TIMEOUT}s"
        ((FAIL++)) || true
    else
        RESULTS[$name]="FAIL"
        local detail
        detail=$(tail -5 "$out_file" | tr '\n' ' ' | sed 's/  */ /g')
        NOTES_MAP[$name]="exit_code=${exit_code} — ${detail}"
        ((FAIL++)) || true
    fi
}

echo "Running Lumo E2E suite — ${TIMESTAMP}"
for t in "${TESTS[@]}"; do
    name="${t%.py}"
    echo -n "  ${name} ... "
    run_test "$t"
    echo "${RESULTS[$name]} (${TIMES[$name]:-skipped})"
done

# Write report
{
    echo "# E2E Lumo OS — ${DATE_DAY} ${TIMESTAMP}"
    echo ""
    echo "## Summary: ${PASS} pass / ${FAIL} fail / ${SKIP} skip"
    echo ""
    for t in "${TESTS[@]}"; do
        name="${t%.py}"
        result="${RESULTS[$name]:-SKIP}"
        elapsed="${TIMES[$name]:-0s}"
        note="${NOTES_MAP[$name]:-}"
        echo "## ${name}: ${result} (${elapsed})"
        if [[ -n "$note" ]]; then
            echo "  ${note}"
        fi
        echo ""
    done
} > "$REPORT_PATH"

echo ""
echo "Report: $REPORT_PATH"
echo "Summary: ${PASS} pass / ${FAIL} fail / ${SKIP} skip"

# Exit 1 if any failures
[[ $FAIL -eq 0 ]]
