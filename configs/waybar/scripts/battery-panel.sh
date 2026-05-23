#!/bin/bash
# Lumo OS - battery panel popup via rofi.

set -euo pipefail

BAT=/sys/class/power_supply/BAT1
[[ -d $BAT ]] || BAT=/sys/class/power_supply/BAT0

PCT=$(cat $BAT/capacity 2>/dev/null || echo "?")
STATUS=$(cat $BAT/status 2>/dev/null || echo "?")
HEALTH=$(awk -v full=$(cat $BAT/charge_full 2>/dev/null || echo 1) \
             -v design=$(cat $BAT/charge_full_design 2>/dev/null || echo 1) \
             'BEGIN {printf "%.0f%%", (full/design)*100}')
CYCLES=$(cat $BAT/cycle_count 2>/dev/null || echo "?")
LIMIT=$(cat $BAT/charge_control_end_threshold 2>/dev/null || echo "100")

PROFILE=$(powerprofilesctl get 2>/dev/null || echo "?")

PANEL="${PCT}% — ${STATUS}
─────────
Saude: ${HEALTH}
Ciclos: ${CYCLES}
Limite carga: ${LIMIT}%
Perfil: ${PROFILE}
─────────
Cuidar bateria (80%)
Modo: balanced
Modo: performance
Modo: power-saver"

CHOICE=$(echo "$PANEL" | rofi -dmenu -p "Bateria" -theme ~/.config/rofi/lumo.rasi -width 30)

case "$CHOICE" in
    "Cuidar bateria (80%)")
        if [[ "$LIMIT" == "80" ]]; then
            echo 100 | sudo tee $BAT/charge_control_end_threshold
            notify-send "Bateria" "Limite 100%"
        else
            echo 80 | sudo tee $BAT/charge_control_end_threshold
            notify-send "Bateria" "Limite 80%"
        fi
        ;;
    "Modo: balanced") powerprofilesctl set balanced ;;
    "Modo: performance") powerprofilesctl set performance ;;
    "Modo: power-saver") powerprofilesctl set power-saver ;;
esac
