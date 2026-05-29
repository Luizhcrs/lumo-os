#!/usr/bin/env bash
# install-system-files.sh — instala arquivos de sistema do Lumo nos dirs CORRETOS.
#
# Motivo (bug boot 2026-05-28): os .rules de polkit e udev compartilham o
# prefixo "49-", entao um `cp 49-*.rules /etc/udev/rules.d/` por glob jogava
# os arquivos polkit (polkit.addRule, comentarios //) dentro do dir do udev.
# udev parseia e choca: "Invalid key/value pair" / "Invalid key 'subject.user'"
# em ~50 linhas no boot. Este script mapeia cada arquivo ao destino certo
# pra nunca mais depender de glob ambiguo.
#
# Uso: sudo ./install-system-files.sh

set -euo pipefail

if [[ $EUID -ne 0 ]]; then
    echo "Rode com sudo." >&2
    exit 1
fi

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
USER_NAME="${SUDO_USER:-luizhcrds}"
USER_HOME="$(getent passwd "$USER_NAME" | cut -d: -f6)"

install_file() {
    local src="$1" dst="$2" mode="$3"
    install -D -m "$mode" "$HERE/$src" "$dst"
    echo "  $src -> $dst"
}

echo "[polkit] regras JS (polkit.addRule) -> /etc/polkit-1/rules.d/"
install_file 49-lumo-nm.rules      /etc/polkit-1/rules.d/49-lumo-nm.rules      0644
install_file 49-lumo-power.rules   /etc/polkit-1/rules.d/49-lumo-power.rules   0644
install_file 49-lumo-sensors.rules /etc/polkit-1/rules.d/49-lumo-sensors.rules 0644

echo "[udev] regras de device -> /etc/udev/rules.d/"
# Brightness + LEDs: GROUP=/MODE= nativo (sem callout externo — funciona no
# early boot/initramfs onde /usr nao esta montado).
install_file 90-lumo-backlight.rules /etc/udev/rules.d/90-lumo-backlight.rules 0644
install_file 99-lumo-leds.rules      /etc/udev/rules.d/99-lumo-leds.rules      0644

echo "[grupo power] necessario pra escrita do charge_control_end_threshold"
if ! getent group power >/dev/null; then
    groupadd -r power
    echo "  grupo 'power' criado"
fi
gpasswd -a "$USER_NAME" power >/dev/null || true

echo "[battery perms] systemd oneshot pos-boot (sysfs attr nao aceita"
echo "  GROUP/MODE nativo do udev, e RUN+=/bin/chgrp falha no initramfs)"
install_file lumo-battery-perms.service /etc/systemd/system/lumo-battery-perms.service 0644
# Limpa abordagens antigas que davam erro de boot.
rm -f /etc/udev/rules.d/91-lumo-battery.rules /etc/tmpfiles.d/lumo-bat.conf

echo "[tmpfiles] -> /etc/tmpfiles.d/"
install_file lumo-leds.tmpfiles.conf /etc/tmpfiles.d/lumo-leds.conf 0644

echo "[gtk decoration] apps CSD (GTK) com min/max/close (nao so o X)"
# Layout dos botoes da headerbar GTK. Sem GNOME settings-daemon o GTK usa
# 'appmenu:close' (so X). Forcar via gsettings (dconf) + settings.ini.
USER_HOME_REAL="$(getent passwd "$USER_NAME" | cut -d: -f6)"
install -D -m 0644 "$HERE/gtk-settings.ini" "$USER_HOME_REAL/.config/gtk-3.0/settings.ini"
install -D -m 0644 "$HERE/gtk-settings.ini" "$USER_HOME_REAL/.config/gtk-4.0/settings.ini"
chown -R "$USER_NAME:$USER_NAME" "$USER_HOME_REAL/.config/gtk-3.0" "$USER_HOME_REAL/.config/gtk-4.0" 2>/dev/null || true
# gsettings precisa rodar como o user (dconf por-user).
sudo -u "$USER_NAME" DBUS_SESSION_BUS_ADDRESS="unix:path=/run/user/$(id -u "$USER_NAME")/bus" \
    gsettings set org.gnome.desktop.wm.preferences button-layout ':minimize,maximize,close' 2>/dev/null || true

echo "[reload]"
udevadm control --reload-rules
systemctl daemon-reload
systemctl enable --now lumo-battery-perms.service 2>/dev/null || true

echo "OK. Polkit recarrega sozinho (polkitd observa o dir)."
