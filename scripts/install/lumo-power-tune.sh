#!/bin/bash
# Lumo OS power tweaks pra Galaxy KG7BR — PowerTOP-style aplicados systemd.
# Roda ao boot via lumo-power-tune.service.

set -e

# PCIe ASPM max economia (Active State Power Management)
echo powersupersave > /sys/module/pcie_aspm/parameters/policy 2>/dev/null || true

# USB autosuspend default todos devices
for d in /sys/bus/usb/devices/*/power/control; do
  echo auto > "$d" 2>/dev/null || true
done

# PCI runtime PM auto
for d in /sys/bus/pci/devices/*/power/control; do
  echo auto > "$d" 2>/dev/null || true
done

# Audio HDA power save
echo 1 > /sys/module/snd_hda_intel/parameters/power_save 2>/dev/null || true
echo Y > /sys/module/snd_hda_intel/parameters/power_save_controller 2>/dev/null || true

# WiFi power save (iwlwifi)
echo 1 > /sys/module/iwlwifi/parameters/power_save 2>/dev/null || true
iw dev wlan0 set power_save on 2>/dev/null || true

# NVMe APST aggressive (deepest state)
for d in /sys/class/nvme/*/power/control; do
  echo auto > "$d" 2>/dev/null || true
done

# Bluetooth power save
for d in /sys/class/bluetooth/*/power/control; do
  echo auto > "$d" 2>/dev/null || true
done

# Watchdog autosuspend
for d in /sys/devices/platform/watchdog/*/power/control; do
  echo auto > "$d" 2>/dev/null || true
done

# i915 GPU runtime pm
echo 1 > /sys/module/i915/parameters/enable_psr 2>/dev/null || true
echo 1 > /sys/module/i915/parameters/enable_fbc 2>/dev/null || true

# samsung-galaxybook charge limit 80% (idempotente)
for d in /sys/class/power_supply/BAT*/charge_control_end_threshold; do
  echo 80 > "$d" 2>/dev/null || true
done

echo "[lumo-power-tune] applied"
