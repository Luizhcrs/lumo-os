#!/usr/bin/env bash
# demo.sh -- Script de demo ensaiavel Lumo OS (15min)
# Uso: bash scripts/demo.sh
# Requer: sessao Lumo OS ativa (TTY3 DRM ou nested Winit)

set -euo pipefail

LUMO_SOCK="${XDG_RUNTIME_DIR:-/run/user/1000}/lumo-wm.sock"
LUMOCTL="lumoctl"

log() {
    echo "[demo] $*"
}

cue() {
    echo ""
    echo "=== CUE: $* ==="
    echo ""
}

wait_presenter() {
    local msg="$1"
    echo ""
    echo "--- PAUSA: $msg"
    echo "    Pressione ENTER para continuar..."
    read -r
}

# ============================================================
# FASE 0 -- Pre-voo (nao apresentar ao publico)
# ============================================================

log "Verificando ambiente..."

if [[ ! -S "$LUMO_SOCK" ]]; then
    echo "ERRO: lumo-wm socket nao encontrado em $LUMO_SOCK"
    echo "      Certifique-se de que lumo-wm esta rodando."
    exit 1
fi

if ! command -v "$LUMOCTL" &>/dev/null; then
    echo "ERRO: lumoctl nao encontrado no PATH"
    exit 1
fi

log "Ambiente OK. lumo-wm socket encontrado."

$LUMOCTL set-theme dark 2>/dev/null || log "set-theme dark: falhou (ignorando)"

sleep 1

# ============================================================
# FASE 1 -- Introducao (0:00 - 2:00)
# CUE: mostrar desktop limpo, bar no topo
# ============================================================

cue "FASE 1: Desktop limpo -- mostrar bar e workspace pills"
wait_presenter "confirmar que desktop esta limpo e bar visivel"

$LUMOCTL set-workspace 2 2>/dev/null || log "set-workspace: usar click na bar"
sleep 2
$LUMOCTL set-workspace 3 2>/dev/null || true
sleep 2
$LUMOCTL set-workspace 1 2>/dev/null || true
sleep 1

# ============================================================
# FASE 2 -- Integracao Hardware (2:00 - 5:00)
# CUE: abrir dropdown de bateria
# ============================================================

cue "FASE 2: Integracao hardware -- dropdown bateria"
wait_presenter "clicar no icone de bateria na bar para abrir dropdown"

echo ""
echo "--- Valores sysfs ao vivo ---"
cat /sys/class/power_supply/BAT1/capacity 2>/dev/null && echo "% bateria" || echo "bateria: N/A"
cat /sys/class/power_supply/BAT1/charge_full 2>/dev/null | awk '{printf "saude: %.1f%%\n", $1/3530000*100}' || true
cat /sys/class/power_supply/BAT1/cycle_count 2>/dev/null && echo "ciclos" || echo "ciclos: N/A"
cat /sys/firmware/acpi/platform_profile 2>/dev/null && echo "(platform profile atual)" || echo "platform profile: N/A"
echo "----------------------------"
echo ""

cue "Trocar platform profile: balanced -> performance"
wait_presenter "clicar no dropdown para trocar platform profile"

if [[ -w /sys/firmware/acpi/platform_profile ]]; then
    echo "performance" > /sys/firmware/acpi/platform_profile
    log "Platform profile: performance"
    sleep 2
    echo "balanced" > /sys/firmware/acpi/platform_profile
    log "Platform profile: voltou para balanced"
else
    log "platform_profile: sem permissao de escrita (mostrar via UI)"
fi

sleep 2

# ============================================================
# FASE 3 -- Janelas e SSD (5:00 - 8:00)
# CUE: abrir apps
# ATENCAO: nao clicar no botao close da janela (P0 pendente)
# ============================================================

cue "FASE 3: Janelas com server-side decorations -- abrir lumo-files"
wait_presenter "pronto para abrir lumo-files"

lumo-files &
LUMO_FILES_PID=$!
log "lumo-files PID: $LUMO_FILES_PID"
sleep 3

cue "Navegar por alguns diretorios. NAO clicar no botao close (bug P0 pendente -- usar Alt+F4)."
wait_presenter "terminada navegacao em lumo-files"

lumo-monitor &
LUMO_MONITOR_PID=$!
sleep 3

cue "Mostrar duas janelas lado a lado. Drag na titlebar para reposicionar."
wait_presenter "reposicionamento demonstrado"

# ============================================================
# FASE 4 -- Hot Reload de Tema (8:00 - 10:00)
# CUE: trocar tema
# ============================================================

cue "FASE 4: Hot reload de tema -- dark -> light -> dark"
wait_presenter "pronto para trocar tema"

$LUMOCTL set-theme light 2>/dev/null || log "set-theme: usar toggle na bar"
sleep 3

cue "Mostrar todas as janelas e bar em light mode"
sleep 4

$LUMOCTL set-theme dark 2>/dev/null || true
sleep 2

# ============================================================
# FASE 5 -- Spring Animations (10:00 - 12:00)
# CUE: abrir e fechar apps para mostrar animacoes
# ============================================================

cue "FASE 5: Animacoes spring -- abrir lumo-calc (entrada bouncy)"
wait_presenter "pronto para demonstrar animacoes"

lumo-calc &
LUMO_CALC_PID=$!
sleep 3

cue "Mostrar entrada bouncy da janela. Abrir dropdown de brilho (animacao smooth)."
wait_presenter "animacoes demonstradas"

# ============================================================
# FASE 6 -- Launcher (12:00 - 14:00)
# CUE: abrir launcher fuzzy search
# Nota: launcher e M1 -- pular se nao disponivel
# ============================================================

cue "FASE 6: Launcher fuzzy search (M1 -- pular se nao disponivel)"

if command -v lumo-launcher &>/dev/null; then
    wait_presenter "pronto para abrir launcher"
    lumo-launcher &
    LUMO_LAUNCHER_PID=$!
    sleep 2
    cue "Digitar nome de app no fuzzy search"
    wait_presenter "launcher demonstrado"
    kill "${LUMO_LAUNCHER_PID:-0}" 2>/dev/null || true
else
    log "lumo-launcher nao disponivel (M1 feature) -- pulando fase 6"
    sleep 1
fi

# ============================================================
# FASE 7 -- Encerramento (14:00 - 15:00)
# CUE: desktop com multiplas janelas abertas para Q&A
# ============================================================

cue "FASE 7: Encerramento -- desktop para Q&A"
wait_presenter "pronto para encerrar apresentacao"

echo ""
echo "Apps abertos durante demo:"
echo "  lumo-files   PID: ${LUMO_FILES_PID:-N/A}"
echo "  lumo-monitor PID: ${LUMO_MONITOR_PID:-N/A}"
echo "  lumo-calc    PID: ${LUMO_CALC_PID:-N/A}"
echo ""
echo "Para cleanup manual: kill <pids> 2>/dev/null"
echo ""

# ============================================================
# CLEANUP -- executar apos Q&A terminar
# ============================================================

wait_presenter "Q&A terminado -- ENTER para cleanup"

log "Cleanup: fechando apps demo..."
kill "${LUMO_FILES_PID:-0}" 2>/dev/null && log "lumo-files fechado" || true
kill "${LUMO_MONITOR_PID:-0}" 2>/dev/null && log "lumo-monitor fechado" || true
kill "${LUMO_CALC_PID:-0}" 2>/dev/null && log "lumo-calc fechado" || true

if [[ -w /sys/firmware/acpi/platform_profile ]]; then
    echo "balanced" > /sys/firmware/acpi/platform_profile
    log "Platform profile: restaurado para balanced"
fi

$LUMOCTL set-theme dark 2>/dev/null || true

log "Cleanup concluido."
