#!/usr/bin/env bash
# scripts/install/lumo-gtk-theme/install.sh
# Instala o tema GTK Lumo dark em ~/.themes/Lumo/

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
TARGET="$HOME/.themes/Lumo"

echo "Instalando tema Lumo em $TARGET ..."

install -d "$TARGET/gtk-3.0"
install -d "$TARGET/gtk-4.0"
install -m 644 "$SCRIPT_DIR/gtk-3.0/gtk.css" "$TARGET/gtk-3.0/gtk.css"
install -m 644 "$SCRIPT_DIR/gtk-4.0/gtk.css" "$TARGET/gtk-4.0/gtk.css"

echo "Tema instalado."
echo "Para ativar, adicione ao seu ~/.profile ou lumo-env.conf:"
echo "  GTK_THEME=Lumo:dark"
echo "  ADW_DEBUG_COLOR_SCHEME=force-dark"
