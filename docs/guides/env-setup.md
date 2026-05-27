# Lumo OS — Environment Setup

## Hardware-Alvo

Samsung Galaxy Book 4 U300 (NP750XGJ-*). Especificacoes de sensores e hardware em `docs/sensors_galaxy_book4.md`.

| Item | Spec |
|------|------|
| CPU | Intel U300 Raptor Lake-U, 1P+4E, ate 4.4 GHz |
| GPU | Intel UHD Xe-LP Gen12.1, 48 EU |
| RAM | 8 GB LPDDR4 |
| Display | 15.6" FHD IPS 60Hz eDP-1, 6-bit + FRC |
| DRM device | `/dev/dri/card1` |
| Kernel alvo | Linux 7.x mainline (Arch) |

Outros hardwares podem funcionar, mas nenhum teste e realizado fora do Galaxy Book 4.

## Sistema Operacional

EndeavourOS (base Arch Linux). Kernel `linux` mainline recomendado.

## Pacotes Obrigatorios

```
# Compositor e graficos
sudo pacman -S wayland wayland-protocols libxkbcommon mesa vulkan-intel
sudo pacman -S seatd libseat libinput

# Fontes
sudo pacman -S noto-fonts noto-fonts-emoji ttf-geist

# Bar e shell
sudo pacman -S appmenu-gtk-module foot mousepad

# Build toolchain
sudo pacman -S rust cargo base-devel pkg-config cmake
sudo pacman -S libxkbcommon libxkbcommon-x11

# Sensores
sudo pacman -S lm_sensors iio-sensor-proxy

# Utils
sudo pacman -S lsof fuser
```

## Grupos de Usuario

```
sudo usermod -aG seat,input,video,render $USER
```

Relogar apos adicionar grupos.

## Servicos do Sistema

```
sudo systemctl enable --now seatd.socket
sudo systemctl enable --now systemd-logind
```

## Scripts de Install (udev + polkit + tmpfiles)

```
sudo cp scripts/install/*.rules /etc/udev/rules.d/
sudo cp scripts/install/*.tmpfiles.conf /etc/tmpfiles.d/
sudo cp scripts/install/*.pkla /etc/polkit-1/rules.d/
sudo udevadm control --reload
sudo systemd-tmpfiles --create
sudo systemctl restart polkit
```

Arquivos em `scripts/install/`:
- `49-lumo-nm.rules` — NetworkManager sem senha para usuario lumo
- `49-lumo-sensors.rules` — acesso sysfs sensores sem sudo
- `90-lumo-backlight.rules` — acesso `/sys/class/backlight/` sem sudo
- `99-lumo-leds.rules` — acesso LEDs
- `lumo-leds.tmpfiles.conf` — permissoes tmpfiles para LEDs
- `lumo-prewarm.service` — cria IPC socket antes do compositor subir

## Fontes Geist

```
scripts/install-fonts.sh
```

Instala Geist e Geist Mono em `~/.local/share/fonts/`.

## Autologin TTY3

```
sudo scripts/setup-autologin.sh
```

Configura autologin do usuario no TTY3 via systemd-getty. Lumo sobe automaticamente no TTY3 ao ligar o sistema.

Para fazer manualmente: criar override em `/etc/systemd/system/getty@tty3.service.d/autologin.conf`.

## Build

```
cd ~/Projects/lumo-shell
cargo build --release --workspace
```

Build com backend DRM (producao):

```
cargo build --release --features lumo-wm/drm-backend --bin lumo-wm --bin lumo-bar
```

## Execucao

### Modo desenvolvimento (nested no Hyprland)

```
cargo build --release --bin lumo-wm --bin lumo-bar --bin lumo-desktop
scripts/lumo-dev.sh
```

Abre janela nested dentro do compositor existente. Sem acesso DRM real.

### Modo producao (TTY3 direto no DRM)

Mudar para TTY3 fisico (Ctrl+Alt+F3 no console), entao:

```
scripts/lumo-tty.sh
```

**NAO funciona via SSH** (requer TTY real com `/dev/tty*`).

Saidas de emergencia:
- `Ctrl+Alt+Backspace` — exit limpo do lumo-wm
- `Ctrl+Alt+F1` — volta TTY1
- SSH de outra maquina: `sudo pkill -9 lumo-wm`

## Variaveis de Ambiente Relevantes

| Variavel | Default | Descricao |
|----------|---------|-----------|
| `LUMO_WM_BACKEND` | `winit` | `drm` para producao TTY |
| `LUMO_THEME` | `light` | `light` ou `dark` |
| `RUST_LOG` | `lumo_wm=info` | Nivel de log |
| `GTK_MODULES` | — | Adicionar `appmenu-gtk-module` para global menu GTK |
| `LIBSEAT_BACKEND` | auto | `logind` se seatd falhar |

## Verificacao de Instalacao

```
# Confirma grupos
groups

# Confirma seatd
systemctl status seatd.socket

# Confirma acesso backlight
ls -la /sys/class/backlight/intel_backlight/brightness

# Confirma DRM
ls -la /dev/dri/card1

# Build limpo
cargo build --release --workspace 2>&1 | tail -5
```
