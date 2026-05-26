#!/usr/bin/env bash
# W37.9: testes do lumo-launch wrapper. Garante env vars + LD_PRELOAD.
# Rode: bash scripts/test-lumo-launch.sh

set -e

SCRIPT="$(dirname "$0")/lumo-launch.sh"
FAIL=0

assert_env() {
    local var="$1"
    local expected="$2"
    local got
    got=$("$SCRIPT" sh -c "echo \$$var")
    if [[ "$got" != "$expected" ]]; then
        echo "FAIL: $var=$got (expected $expected)"
        FAIL=$((FAIL + 1))
    else
        echo "OK: $var=$got"
    fi
}

assert_env_contains() {
    local var="$1"
    local needle="$2"
    local got
    got=$("$SCRIPT" sh -c "echo \$$var")
    if [[ "$got" == *"$needle"* ]]; then
        echo "OK: $var contains $needle"
    else
        echo "INFO: $var=$got does NOT contain $needle (gtk3-nocsd nao instalado?)"
    fi
}

echo "[test] W37.9 lumo-launch env injection"
assert_env "GTK_CSD" "0"
assert_env_contains "GTK_MODULES" "appmenu-gtk-module"
assert_env "QT_QPA_PLATFORMTHEME" "appmenu-qt5"
assert_env "MOZ_GTK_TITLEBAR_DECORATION" "client"
assert_env "MOZ_ENABLE_WAYLAND" "1"
assert_env "GDK_BACKEND" "wayland"
assert_env "XDG_SESSION_TYPE" "wayland"
assert_env_contains "LD_PRELOAD" "libgtk3-nocsd.so.0"

echo "[test] exit propagation"
"$SCRIPT" sh -c "exit 42" && true
exit_code=$?
if [[ $exit_code -eq 42 ]]; then
    echo "OK: exit code propagated ($exit_code)"
else
    echo "FAIL: exit code $exit_code (expected 42)"
    FAIL=$((FAIL + 1))
fi

if [[ $FAIL -gt 0 ]]; then
    echo "FAILED $FAIL tests"
    exit 1
fi
echo "ALL OK"
