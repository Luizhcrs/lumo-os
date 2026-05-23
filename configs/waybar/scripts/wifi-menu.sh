#!/bin/bash
# Lumo OS - wifi menu via rofi.
# Mac-style: lista redes + click pra conectar.

set -euo pipefail

# Lista redes via nmcli.
NETWORKS=$(nmcli -t -f IN-USE,SSID,SIGNAL,SECURITY dev wifi list 2>/dev/null | \
    awk -F: 'NR<=20 && $2 != "" {
        active = ($1 == "*") ? "● " : "  "
        sec = ($4 == "" || $4 == "--") ? "" : " "
        printf "%s%s (%s%%)%s\n", active, $2, $3, sec
    }')

if [[ -z "$NETWORKS" ]]; then
    rofi -e "Nenhuma rede disponivel" -theme ~/.config/rofi/lumo.rasi
    exit 0
fi

# Toggle option
MENU="Toggle WiFi
─────────
$NETWORKS"

CHOICE=$(echo "$MENU" | rofi -dmenu -p "WiFi" -theme ~/.config/rofi/lumo.rasi)

if [[ -z "$CHOICE" ]]; then
    exit 0
fi

if [[ "$CHOICE" == "Toggle WiFi" ]]; then
    STATE=$(nmcli radio wifi)
    if [[ "$STATE" == "ativado" || "$STATE" == "enabled" ]]; then
        nmcli radio wifi off
    else
        nmcli radio wifi on
    fi
    exit 0
fi

# Extract SSID (remove leading marker + signal info)
SSID=$(echo "$CHOICE" | sed -e 's/^[●  ]*//' -e 's/ ([0-9]*%).*$//' -e 's/ $//')

if [[ -z "$SSID" ]]; then
    exit 0
fi

# Try connect.
RESULT=$(nmcli dev wifi connect "$SSID" 2>&1)
if echo "$RESULT" | grep -q "successfully"; then
    notify-send "WiFi" "Conectado a $SSID"
elif echo "$RESULT" | grep -qi "secrets\|password\|secret"; then
    # Precisa senha.
    PASS=$(rofi -dmenu -password -p "Senha $SSID" -theme ~/.config/rofi/lumo.rasi)
    if [[ -n "$PASS" ]]; then
        nmcli dev wifi connect "$SSID" password "$PASS" \
            && notify-send "WiFi" "Conectado a $SSID" \
            || notify-send "WiFi" "Falhou: $SSID" -u critical
    fi
else
    notify-send "WiFi" "Erro: $RESULT" -u critical
fi
