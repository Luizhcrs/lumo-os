#!/bin/bash
# Lumo OS - brightness panel via rofi presets.

set -euo pipefail

PCT=$(brightnessctl -m 2>/dev/null | awk -F, '{print $4}' | tr -d '%')

PANEL="${PCT}% atual
─────────
Dia (80%)
Noite (35%)
Maximo (100%)
─────────
+5%
-5%"

CHOICE=$(echo "$PANEL" | rofi -dmenu -p "Brilho" -theme ~/.config/rofi/lumo-dropdown.rasi)

case "$CHOICE" in
    "Dia (80%)") brightnessctl s 80% ;;
    "Noite (35%)") brightnessctl s 35% ;;
    "Maximo (100%)") brightnessctl s 100% ;;
    "+5%") brightnessctl s 5%+ ;;
    "-5%") brightnessctl s 5%- ;;
esac
