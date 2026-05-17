#!/usr/bin/env bash
# setup-autologin.sh - configura autologin no TTY3 pra usuario rodar Lumo.
#
# A9: roda 1 vez como root. Cria override systemd em
# /etc/systemd/system/getty@tty3.service.d/autologin.conf que substitui
# o login normal por agetty --autologin.
#
# DEPOIS de rodar este script, adicione ao ~/.bash_profile OU ~/.zprofile
# do usuario alvo (ou .profile, dependendo do shell):
#
#   if [[ "$(tty)" = "/dev/tty3" ]] && [[ -z "$WAYLAND_DISPLAY" ]] && [[ -z "$DISPLAY" ]]; then
#       exec ~/Projects/lumo-shell/scripts/lumo-tty.sh
#   fi
#
# Justificativa (memory feedback_design_lapidado): autologin condicional
# por TTY = login normal em outros TTYs (1, 2, 4...) continua exigindo
# senha. Risco isolado a TTY3.
#
# Reversao: sudo rm -rf /etc/systemd/system/getty@tty3.service.d/
#           sudo systemctl daemon-reload
#           sudo systemctl restart getty@tty3.service

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
    echo "ERRO: rodar como root (sudo $0)"
    exit 1
fi

TARGET_USER="${SUDO_USER:-luizhcrds}"
OVERRIDE_DIR=/etc/systemd/system/getty@tty3.service.d
OVERRIDE_FILE="$OVERRIDE_DIR/autologin.conf"

# Sanity: usuario alvo existe?
if ! id "$TARGET_USER" >/dev/null 2>&1; then
    echo "ERRO: usuario '$TARGET_USER' nao existe no sistema."
    exit 1
fi

echo "Configurando autologin TTY3 para usuario: $TARGET_USER"
echo "Override file: $OVERRIDE_FILE"

mkdir -p "$OVERRIDE_DIR"

cat > "$OVERRIDE_FILE" <<EOF
# Gerado por scripts/setup-autologin.sh do Lumo OS.
# Substitui o login interativo do TTY3 por autologin direto pra '$TARGET_USER'.
# Reversao: sudo rm $OVERRIDE_FILE && sudo systemctl daemon-reload
[Service]
ExecStart=
ExecStart=-/sbin/agetty --autologin $TARGET_USER --noclear %I \$TERM
Type=idle
EOF

echo "Wrote $OVERRIDE_FILE"

systemctl daemon-reload
echo "systemd daemon reloaded"

systemctl enable getty@tty3.service >/dev/null 2>&1 || true
echo "getty@tty3.service enabled"

# Restart so se quiser ativar agora.
read -p "Restart getty@tty3.service agora? [y/N] " yn
if [[ "${yn:-N}" == "y" ]]; then
    systemctl restart getty@tty3.service
    echo "getty@tty3.service restarted"
fi

echo ""
echo "============================================================"
echo "Proximo passo: adicione ao ~/.bash_profile OU ~/.zprofile do $TARGET_USER:"
echo ""
echo '   if [[ "$(tty)" = "/dev/tty3" ]] && [[ -z "$WAYLAND_DISPLAY" ]] && [[ -z "$DISPLAY" ]]; then'
echo '       exec ~/Projects/lumo-shell/scripts/lumo-tty.sh'
echo '   fi'
echo ""
echo "Depois disso: Ctrl+Alt+F3 do Hyprland host -> autologin -> Lumo."
echo "Volta com Ctrl+Alt+F1."
echo "============================================================"
