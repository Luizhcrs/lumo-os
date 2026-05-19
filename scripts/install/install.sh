#!/bin/bash
set -euo pipefail
REPO="$(cd "$(dirname "$0")/../.." && pwd)"

echo "[1/6] udev rules"
sudo cp "$REPO"/scripts/install/*.rules /etc/udev/rules.d/

echo "[2/6] tmpfiles"
sudo cp "$REPO"/scripts/install/*.tmpfiles.conf /etc/tmpfiles.d/

echo "[3/6] polkit"
sudo cp "$REPO"/scripts/install/*.rules.pkla /etc/polkit-1/localauthority/50-local.d/ 2>/dev/null || true
sudo cp "$REPO"/scripts/install/*.rules.polkit /etc/polkit-1/rules.d/ 2>/dev/null || true

echo "[4/6] reload system units"
sudo udevadm control --reload
sudo systemd-tmpfiles --create
sudo systemctl restart polkit

echo "[5/6] user config"
mkdir -p ~/.config/lumo
cp "$REPO"/scripts/install/lumo-env.conf ~/.config/lumo/env.conf

echo "[6/6] systemd user units"
mkdir -p ~/.config/systemd/user
for svc in lumo-bar lumo-desktop lumo-osd lumo-power lumo-prewarm; do
    if [ -f "$REPO/scripts/install/${svc}.service" ]; then
        cp "$REPO/scripts/install/${svc}.service" ~/.config/systemd/user/
        echo "  installed ${svc}.service"
    fi
done
systemctl --user daemon-reload
for svc in lumo-bar lumo-desktop lumo-osd lumo-power lumo-prewarm; do
    if [ -f ~/.config/systemd/user/${svc}.service ]; then
        systemctl --user enable "${svc}.service" 2>&1 && echo "  enabled ${svc}" || true
    fi
done

echo "Done. Run lumo-tty.sh from TTY3."
