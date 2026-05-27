#!/usr/bin/env bash
# Install lumo-csd.conf em ~/.config/environment.d/ pra apply em toda
# sessao user. Idempotente.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC="$SCRIPT_DIR/lumo-csd.conf"
DST_DIR="$HOME/.config/environment.d"
DST="$DST_DIR/lumo-csd.conf"

if [[ ! -f "$SRC" ]]; then
    echo "ERRO: $SRC nao encontrado" >&2
    exit 1
fi

mkdir -p "$DST_DIR"
cp "$SRC" "$DST"
chmod 0644 "$DST"
echo "[lumo-csd] instalado em $DST"

# Verifica gtk3-nocsd lib
if [[ -f /usr/lib/libgtk3-nocsd.so.0 ]] || [[ -f /usr/local/lib/libgtk3-nocsd.so.0 ]]; then
    echo "[lumo-csd] gtk3-nocsd encontrado ✓"
else
    echo "[lumo-csd] AVISO: gtk3-nocsd nao instalado. Instalar via AUR: yay -S gtk3-nocsd"
fi

# Reload systemd user env
if command -v systemctl &>/dev/null; then
    systemctl --user daemon-reload 2>/dev/null || true
    echo "[lumo-csd] systemctl --user daemon-reload OK"
fi

echo ""
echo "Apos isso, logout + login OU restart sessao Lumo pra aplicar."
echo "Verificar via: systemctl --user show-environment | grep -E 'LD_PRELOAD|GTK_CSD|GTK_MODULES'"
