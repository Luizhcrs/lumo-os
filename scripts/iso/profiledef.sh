#!/usr/bin/env bash
# profiledef.sh -- Archiso profile for Lumo OS Galaxy Book 4 installer.
#
# Build with: scripts/iso/build-iso.sh
# Requires: archiso package (mkarchiso), run as root.

iso_name="lumo-os-galaxy-book-4"
iso_label="LUMO_OS_$(date --date="@${SOURCE_DATE_EPOCH:-$(date +%s)}" +%Y%m)"
iso_publisher="Lumo OS Project"
iso_application="Lumo OS — Lumo Book 4 Edition"
iso_version="0.1.0"
iso_url=""
install_dir="lumo"
buildmodes=('iso')
bootmodes=(
    'bios.syslinux.mbr'
    'bios.syslinux.eltorito'
    'uefi-ia32.grub.esp'
    'uefi-x64.grub.esp'
    'uefi-x64.systemd-boot.esp'
)
arch="x86_64"
pacman_conf="scripts/iso/pacman.conf"
airootfs_image_type="squashfs"
airootfs_image_tool_options=('-comp' 'zstd' '-Xcompression-level' '15')
bootstrap_tarball_compression=('zstd' '-c' '-T0' '--auto-threads=logical' '--long' '-19')

# Files to be placed in the root of the ISO filesystem.
file_permissions=(
    ["/etc/shadow"]="0:0:400"
    ["/root"]="0:0:750"
    ["/usr/local/bin/lumo-firstrun"]="0:0:755"
    ["/usr/local/bin/lumo-wm"]="0:0:755"
    ["/usr/local/bin/lumo-bar"]="0:0:755"
    ["/usr/local/bin/lumo-dock"]="0:0:755"
    ["/usr/local/bin/lumo-launcher"]="0:0:755"
    ["/usr/local/bin/lumo-store"]="0:0:755"
)
