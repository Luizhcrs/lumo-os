# Lumo OS

Um sistema operacional sob medida para o Samsung Galaxy Book 4 U300.

Da camada do kernel ate o ultimo pixel, cada componente foi feito pra esse hardware. Sem GNOME, sem KDE, sem Hyprland. Compositor proprio em Rust, shell proprio, apps proprios, identidade visual unica. O resultado e um laptop que responde diferente — input <16ms, animacoes com fisica real, sRGB correto ate o painel 6-bit FRC, autonomia gerida cell-by-cell.

> Lumo OS — o sistema que faz o Galaxy Book parecer outro produto.

## Por que existe

Hardware Samsung topo de linha rodando Windows generico ou Ubuntu padrao desperdicia o que o Galaxy Book 4 tem de melhor. Sensores ignorados, charge curve sem politica, painel 6-bit sem dither, touchpad com latencia X11. Lumo resolve isso na origem.

## Estado atual

| Camada | Status |
|---|---|
| Kernel + drivers samsung-galaxybook | pronto |
| Compositor lumo-wm (smithay Rust) | ~90% |
| Shell (bar/dock/launcher/notif/osd) | ~50% |
| Apps Lumo (files/calc/editor/notes/monitor/settings/store) | em curso |
| Installer ISO + first-run wizard | beta |
| Samsung pitch deck | pendente |

## Diferenciais tecnicos

1. Spring physics closed-form em cada widget (massa-mola amortecida, sem fixed timestep)
2. Pipeline sRGB correto ate o painel — 5 patches smithay vendored
3. 100% Rust no compositor — memory-safe sem GC
4. Hardware-specific desde a primeira linha (Galaxy Book 4 U300)
5. Wayland-first DRM-native, zero Xorg
6. Charge limit + cell balancing automatico via samsung-galaxybook driver

## Documentacao

| Doc | Conteudo |
|-----|----------|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | 13 crates, fluxo de dados, IPC, pipeline cor sRGB |
| [docs/UX_GUIDELINES.md](docs/UX_GUIDELINES.md) | Filosofia, tokens, animacoes, componentes |
| [docs/ENV_SETUP.md](docs/ENV_SETUP.md) | Instalacao completa passo-a-passo |
| [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) | Padrao de codigo, commits, code review |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Milestones M0-M4 ate Samsung pitch |
| [docs/ISO_BUILD.md](docs/ISO_BUILD.md) | Build da distro lumo-os-galaxy-book-4 |
| [docs/sensors_galaxy_book4.md](docs/sensors_galaxy_book4.md) | Auditoria de sensores do hardware-alvo |
| [DEPS.md](DEPS.md) | Dependencias fixadas, patches smithay, gotchas |

## Stack

```
Apps Lumo (files, calc, editor, notes, monitor, settings, store)
+-----------------------------------------------------------------
lumo-shell (bar, dock, launcher, notif, osd, lock)
+-----------------------------------------------------------------
lumo-wm (compositor smithay Rust)
+-----------------------------------------------------------------
lumo-gfx-core + lumo-animation + lumo-foundation (tokens)
+-----------------------------------------------------------------
Wayland 1.23 / libinput / DRM-KMS / PipeWire
+-----------------------------------------------------------------
Linux kernel mainline + systemd + samsung-galaxybook
```

## Como rodar

Desenvolvimento (nested no Hyprland host):

```
cargo build --release --bin lumo-wm --bin lumo-bar --bin lumo-desktop
scripts/lumo-dev.sh
```

Producao (TTY3, DRM direto, requer TTY fisico):

```
scripts/lumo-tty.sh
```

Setup inicial: [docs/ENV_SETUP.md](docs/ENV_SETUP.md). Build da ISO live: [docs/ISO_BUILD.md](docs/ISO_BUILD.md).

## Hardware-alvo

Samsung Galaxy Book 4 U300:
- Intel Core Ultra 5 / 7 Meteor Lake Xe-LPG
- Painel 15.6" 1920x1080 6-bit + FRC
- Touchpad Elan 5-slot
- Sensor lid + battery samsung-galaxybook driver
- WiFi 6E + Bluetooth 5.3

## Repositorio

SSH no hardware-alvo: `luizhcrds@192.168.0.106` → `cd ~/Projects/lumo-shell`.

## Licenca

Proprietary. Todos os direitos reservados a Luiz Henrique Cavalcanti Ramos da Silva.
