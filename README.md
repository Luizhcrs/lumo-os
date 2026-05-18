# Lumo OS

Sistema operacional Linux em Rust para o Samsung Galaxy Book 4 U300.

Camada completa que substitui Hyprland + GNOME/KDE: compositor Wayland proprio, shell (bar/dock/launcher/notif), apps nativas, identidade visual unica.

## Documentacao

| Doc | Conteudo |
|-----|----------|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | 13 crates, fluxo de dados, IPC, pipeline cor sRGB |
| [docs/UX_GUIDELINES.md](docs/UX_GUIDELINES.md) | Filosofia, tokens, animacoes, componentes |
| [docs/ENV_SETUP.md](docs/ENV_SETUP.md) | Instalacao completa passo-a-passo |
| [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) | Padrao de codigo, commits, code review |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Milestones M0-M4 ate Samsung pitch |
| [docs/sensors_galaxy_book4.md](docs/sensors_galaxy_book4.md) | Auditoria de sensores do hardware-alvo |
| [docs/focus_state_machine.md](docs/focus_state_machine.md) | Maquina de estados de foco do compositor |
| [docs/safety_invariants.md](docs/safety_invariants.md) | Invariantes de seguranca do sistema |
| [docs/reviews/](docs/reviews/) | Code reviews datados |
| [DEPS.md](DEPS.md) | Dependencias fixadas, patches smithay, gotchas |

## Stack

```
Apps Lumo (term, files, settings)          [M3]
+---------------------------------------------
lumo-shell (bar, dock, launcher, notif)    [~35%]
+---------------------------------------------
lumo-wm (compositor smithay Rust)          [~90%]
+---------------------------------------------
lumo-gfx-core (wgpu + pipeline grafica)   [pronto]
+---------------------------------------------
Wayland / libinput / DRM-KMS              [host]
+---------------------------------------------
Linux kernel + systemd + EndeavourOS      [pronto]
```

## Como Rodar

Desenvolvimento (nested no Hyprland):

```
cargo build --release --bin lumo-wm --bin lumo-bar --bin lumo-desktop
scripts/lumo-dev.sh
```

Producao (TTY3, DRM direto) — requer TTY fisico:

```
scripts/lumo-tty.sh
```

Setup inicial: ver [docs/ENV_SETUP.md](docs/ENV_SETUP.md).

## Diferenciais

1. Spring physics real em cada widget (massa-mola amortecida parametrizada)
2. Pipeline cor sRGB correta ate o painel (5 patches smithay vendored — ver DEPS.md)
3. 100% Rust no compositor (memory-safe)
4. Hardware-specific desde o inicio (Galaxy Book 4 U300)
5. Wayland-first DRM-native, sem Xorg

## Repositorio

SSH no hardware-alvo: `luizhcrds@192.168.0.106` -> `cd ~/Projects/lumo-shell`
