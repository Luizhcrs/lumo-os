# ISO Build -- Lumo OS Galaxy Book 4

Guia para gerar a ISO instalavel do Lumo OS.

## Pre-requisitos

| Ferramenta | Versao minima | Instalacao          |
|------------|---------------|---------------------|
| archiso    | 78+           | `pacman -S archiso` |
| mkarchiso  | incluido      | via archiso         |
| Rust       | 1.78+         | via rustup          |

Requer sistema Arch Linux ou derivado. Buildar como root.

## Build rapido

```bash
sudo scripts/iso/build-iso.sh
```

ISO gerada em `out/lumo-os-galaxy-book-4-<YYYYMM>-x86_64.iso`.

## Build com diretorio customizado

```bash
sudo scripts/iso/build-iso.sh --work /tmp/meu-work --out /mnt/storage/iso-out
```

## Estrutura do perfil

```
scripts/iso/
  profiledef.sh              # metadados ISO, modos boot, permissoes
  packages.x86_64            # lista de pacotes instalados
  pacman.conf                # repo Arch + repo local lumo
  build-iso.sh               # wrapper mkarchiso
  syslinux/
    syslinux.cfg             # bootloader BIOS/legacy
  airootfs/
    etc/
      systemd/system/
        lumo-firstrun.service  # servico first-run wizard
      udev/rules.d/
        99-lumo-galaxybook.rules  # regras Samsung Galaxy Book 4
    usr/local/bin/           # binarios Lumo prebuilt (copiados pelo pkgbuild)
```

## Pacotes Lumo prebuilt

Os binarios `lumo-*` sao distribuidos via repo local durante o build.
PKGBUILDs ficam em `scripts/iso/pkgbuild/` (criados na fase M5+).

Fluxo:

```
cargo build --release       # compilar binarios
makepkg -si                 # gerar .pkg.tar.zst
repo-add /tmp/lumo-repo/x86_64/lumo.db.tar.gz *.pkg.tar.zst
sudo scripts/iso/build-iso.sh
```

## Kernel

Usa `linux` (mainline). O driver `samsung-galaxybook` esta upstream desde
kernel 6.9, cobrindo:

- controle de performance (normal/silent/performance)
- backlight keyboard
- hotkeys Fn
- bateria / carga rapida

Nenhum patch fora de upstream necessario.

## Boot modes

A ISO suporta:

- BIOS MBR (syslinux)
- BIOS El Torito (syslinux)
- UEFI ia32 (GRUB)
- UEFI x64 (GRUB + systemd-boot)

Galaxy Book 4 usa UEFI x64 na pratica.

## First-run wizard

Apos boot, se `/var/lib/lumo/first-run-done` nao existe,
`lumo-firstrun.service` inicia o wizard antes do `lumo-wm`.

Ver `apps/lumo-firstrun/` para codigo-fonte.
