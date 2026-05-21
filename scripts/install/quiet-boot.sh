#!/bin/bash
# Quiet boot Galaxy: silencia kernel/systemd verbose pre-Lumo.
# Tela fica preta do POST ate Lumo aparecer.
#
# Uso: sudo bash quiet-boot.sh

set -e
if [ "$EUID" -ne 0 ]; then
    echo "Rode como sudo: sudo bash $0"
    exit 1
fi

ENTRY_DIR=/efi/loader/entries
if [ ! -d "$ENTRY_DIR" ]; then
    echo "ERRO: $ENTRY_DIR nao existe. Verifica systemd-boot."
    exit 1
fi

echo "[1/3] Backup entries atuais..."
BAK_DIR=/root/loader-entries-bak-$(date +%Y%m%d-%H%M%S)
mkdir -p "$BAK_DIR"
cp "$ENTRY_DIR"/*.conf "$BAK_DIR/"
echo "  backup em $BAK_DIR"

QUIET_FLAGS="quiet loglevel=0 vt.global_cursor_default=0 udev.log_level=0 rd.systemd.show_status=false rd.udev.log_level=0 systemd.show_status=false splash"

echo "[2/3] Aplicando flags quiet em entries..."
for entry in "$ENTRY_DIR"/*.conf; do
    [ -f "$entry" ] || continue
    name=$(basename "$entry")
    if grep -q "options" "$entry"; then
        # Remove old quiet flags + add new
        sed -i '/^options/ {
            s/ quiet//g
            s/ loglevel=[0-9]*//g
            s/ rd\.systemd\.show_status=[^ ]*//g
            s/ systemd\.show_status=[^ ]*//g
            s/ vt\.global_cursor_default=[0-9]*//g
            s/ udev\.log_level=[0-9]*//g
            s/ rd\.udev\.log_level=[0-9]*//g
            s/ splash//g
        }' "$entry"
        sed -i "/^options/ s|\$| $QUIET_FLAGS|" "$entry"
        echo "  $name patched"
    fi
done

echo "[3/3] Verificar:"
grep "^options" "$ENTRY_DIR"/*.conf | head

echo ""
echo "==========================================="
echo "QUIET BOOT APLICADO"
echo "Backup em $BAK_DIR"
echo "Reboot pra testar."
echo "Tela preta esperada do POST ate Lumo."
echo "Reverter: cp $BAK_DIR/*.conf $ENTRY_DIR/"
echo "==========================================="
