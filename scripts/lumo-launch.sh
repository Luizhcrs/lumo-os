#!/usr/bin/env bash
# W37.9: wrapper de spawn pra qualquer app GUI.
# Injeta LD_PRELOAD + env vars que suprimem CSD de GTK3/4/libadwaita,
# Firefox e Chrome. Resultado: SSD do Lumo unica identidade visual em
# qualquer app.
#
# Uso: lumo-launch.sh <comando> [args...]
# Ex:  lumo-launch.sh mousepad ~/Desktop/foo.txt

set -e

# GTK3/4/libadwaita: LD_PRELOAD gtk-nocsd se instalado.
# Tenta /usr/lib (AUR pkg) e /usr/local/lib (makepkg manual).
for NOCSD_LIB in /usr/lib/libgtk3-nocsd.so.0 /usr/local/lib/libgtk3-nocsd.so.0; do
    if [[ -f "$NOCSD_LIB" ]]; then
        export LD_PRELOAD="${LD_PRELOAD:+$LD_PRELOAD:}$NOCSD_LIB"
        break
    fi
done

# GTK env vars (fallback se nocsd nao instalado).
export GTK_CSD=0

# W37.10: appmenu-gtk-module exporta menu GTK3 via dbusmenu.
# Bar capta + renderiza como pills (File/Edit/Search/etc).
# UBUNTU_MENUPROXY=1 ativa o gateway (sem isso o modulo nao publica).
export GTK_MODULES="${GTK_MODULES:+$GTK_MODULES:}appmenu-gtk-module"
export UBUNTU_MENUPROXY=1

# Unity em XFCE/Qt para reusar dbusmenu plataforma.
export QT_QPA_PLATFORMTHEME=appmenu-qt5

# Firefox: pede titulo via WM, nao CSD.
export MOZ_GTK_TITLEBAR_DECORATION=client
export MOZ_ENABLE_WAYLAND=1

# Garantia Wayland.
export XDG_SESSION_TYPE=wayland
export GDK_BACKEND=wayland

exec "$@"
