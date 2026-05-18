#!/bin/bash
set -euo pipefail
REPO="$(cd "$(dirname "$0")/../.." && pwd)"

echo "[1/5] udev rules"
sudo cp "$REPO"/scripts/install/*.rules /etc/udev/rules.d/

echo "[2/5] tmpfiles"
sudo cp "$REPO"/scripts/install/*.tmpfiles.conf /etc/tmpfiles.d/

echo "[3/5] polkit"
sudo cp "$REPO"/scripts/install/*.rules.pkla /etc/polkit-1/localauthority/50-local.d/ 2>/dev/null || true
sudo cp "$REPO"/scripts/install/*.rules.polkit /etc/polkit-1/rules.d/ 2>/dev/null || true

echo "[4/5] reload"
sudo udevadm control --reload
sudo systemd-tmpfiles --create
sudo systemctl restart polkit

echo "[5/5] user config"
mkdir -p ~/.config/lumo
cp "$REPO"/scripts/install/lumo-env.conf ~/.config/lumo/env.conf

echo "Done. Run lumo-tty.sh from TTY3."
