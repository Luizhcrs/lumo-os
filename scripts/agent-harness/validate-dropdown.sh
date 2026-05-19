#!/usr/bin/env bash
# validate-dropdown.sh - E2E: click pill bateria, valida que dropdown abre.
#
# Fluxo:
#   1. Garante harness pronto (start-lumo-nested.sh)
#   2. Screenshot pre.png (estado inicial)
#   3. Click absoluto na pill bateria (top-right da bar)
#   4. Aguarda 500ms (animacao abertura B4)
#   5. Screenshot pos.png
#   6. ImageMagick compare AE -> conta pixels diferentes
#   7. Click fora pra fechar (limpa estado pra proxima run)
#   8. Exit 0 se delta > 5000 (dropdown visivel); senao 1.
#
# Coordenadas pill bateria (state.rs:265):
#   bat_hit_rect = (bat_x_start - 4, pill_y, bat_icon_w + 8, pill_h)
# Layout (tokens.rs:18): BAR_HEIGHT=40. pill_y=margin_top (~6), pill_h~=28.
# Battery icon eh o PRIMEIRO icone do pill direito (ordem bat->wifi->data->hora).
# Em 1920x1080 com margin ~16, pill direito ocupa ~ x=[1700..1900], y=[6..34].
# Click no centro da bateria: ~ (1712, 20).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
HARNESS_DIR="/tmp/lumo-agent"
mkdir -p "$HARNESS_DIR"

echo "[1/7] start-lumo-nested..."
"$SCRIPT_DIR/start-lumo-nested.sh"

# shellcheck disable=SC1091
. "$HARNESS_DIR/env.sh"

PRE="$HARNESS_DIR/pre.png"
POS="$HARNESS_DIR/pos.png"
DIFF="$HARNESS_DIR/diff.png"
rm -f "$PRE" "$POS" "$DIFF"

# Coordenadas pill bateria (sobrescrevivel via env LUMO_PILL_BAT_X/Y).
PILL_X="${LUMO_PILL_BAT_X:-1712}"
PILL_Y="${LUMO_PILL_BAT_Y:-20}"

echo "[2/7] screenshot pre..."
"$SCRIPT_DIR/screenshot.sh" pre.png >/dev/null

echo "[3/7] click pill bateria ($PILL_X,$PILL_Y)..."
"$SCRIPT_DIR/click.sh" "$PILL_X" "$PILL_Y"

echo "[4/7] aguarda 500ms animacao..."
sleep 0.5

echo "[5/7] screenshot pos..."
"$SCRIPT_DIR/screenshot.sh" pos.png >/dev/null

echo "[6/7] compare ImageMagick AE..."
# compare exit 0 = identico, 1 = diferente, 2 = erro. Captura stderr (metric).
DELTA=$(compare -metric AE "$PRE" "$POS" "$DIFF" 2>&1 || true)
# DELTA pode vir como "12345" ou "1.2345e+04" ou "12345 (0.18%)" dependendo da versao.
DELTA_NUM=$(echo "$DELTA" | grep -oE '^[0-9.eE+]+' | head -1 || echo "0")
# Normaliza notacao cientifica pra inteiro via awk.
DELTA_INT=$(awk -v d="$DELTA_NUM" 'BEGIN { printf "%d", d }')

echo "[7/7] cleanup: click fora pra fechar dropdown..."
"$SCRIPT_DIR/click.sh" 960 540 >/dev/null 2>&1 || true

echo ""
echo "=== resultado ==="
echo "pre  : $PRE"
echo "pos  : $POS"
echo "diff : $DIFF"
echo "delta_pixels : $DELTA_INT"

THRESHOLD=5000
if [ "$DELTA_INT" -gt "$THRESHOLD" ]; then
    echo "veredito: PASS (delta $DELTA_INT > $THRESHOLD -> dropdown abriu)"
    exit 0
else
    echo "veredito: FAIL (delta $DELTA_INT <= $THRESHOLD -> dropdown invisivel ou click fora do alvo)"
    exit 1
fi
