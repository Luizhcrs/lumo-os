# Lumo OS

Sistema operacional Linux proprio em Rust, sob medida para o Samsung Galaxy Book 4 U300.

## O que e

Camada completa de software que substitui Hyprland + GNOME/KDE no Galaxy Book 4. Compositor Wayland proprio, shell completo (bar/dock/launcher/notif), apps nativas, identidade visual unica.

## Por que

Galaxy Book 4 tem hardware comparavel a um MacBook Air, mas a experiencia padrao (Windows generico) nao reflete isso. Lumo eh a camada de software que faz o hardware Samsung competir em UX de polish premium — input com feedback imediato, animacoes com fisica real, pipeline cor correto ate o painel, latencia baixa medida.

## Para quem

- Hoje: Luiz, no Galaxy Book 4 dele
- Alvo medio: Samsung como white-label OEM
- Alvo final: usuario que compra Galaxy Book e quer experiencia premium nativa

## Stack (1 tela)

```
Apps Lumo (term, files, settings, monitor, calc, notes)    [FALTA]
+----------------------------------------------------------
lumo-shell (bar, dock, launcher, notif, lock, OSD)         [~35%]
+----------------------------------------------------------
lumo-wm (compositor smithay Rust)                          [~90%]
+----------------------------------------------------------
lumo-gfx-core (wgpu framework grafico proprio)             [PRONTO]
+----------------------------------------------------------
Wayland / libinput / DRM-KMS                               [host]
+----------------------------------------------------------
Linux kernel + systemd + EndeavourOS                       [PRONTO]
```

## Componentes prontos

```
crates/foundation/lumo-foundation/   tokens + LFColor + LFTokens
crates/graphics/lumo-beam/           wgpu wrapper (memory/buffers)
crates/graphics/lumo-graphics/       quad SDF + shadow + clip
crates/graphics/lumo-text/           cosmic-text shaping + atlas
crates/graphics/lumo-animation/      spring physics + curvas Lumo ease
crates/ui/lumo-kit/                  buttons, widgets, state
crates/ui/lumo-input/                events + pointer state
crates/compositor/lumo-wm/           compositor smithay
crates/compositor/lumo-ipc/          IPC unix socket bar<->wm
shell/                               binarios lumo-bar / lumo-desktop
```

## Diferenciais defendidos

1. Spring physics real parametrizada (massa-mola amortecida em cada widget animado)
2. Pipeline cor sRGB correta ate o painel (5 patches smithay vendored documentados em DEPS.md)
3. 100% Rust no compositor (memory-safe)
4. Hardware-specific desde o inicio (toda decisao tecnica referencia Galaxy Book 4 U300)
5. Wayland-first DRM-native (sem Xorg)

## Hardware-alvo

| Item | Spec |
|---|---|
| CPU | Intel Processor U300 (Raptor Lake-U, 1P+4E, ate 4.4 GHz) |
| GPU | UHD Xe G4, 48 EU, Gen12.1 |
| RAM | 8 GB LPDDR4 |
| Tela | 15.6\" FHD IPS, 6-bit + FRC, eDP-1 |
| DRM | /dev/dri/card1 (renderD128) |
| Kernel | 7.0.7-arch2-1 mainline |

## Filosofia

- Cada pixel justificado, tokens versionados
- Input com feedback imediato (drop input antigo se lag > 100ms)
- Sem neon/glow, sombras pretas neutras
- Sem emoji em codigo/docs/commits
- Polir antes de acumular

## Como rodar (nested no Hyprland host)

```
cd ~/Projects/lumo-shell
cargo build --release --bin lumo-wm --bin lumo-bar --bin lumo-desktop
scripts/lumo-dev.sh
```

## Como rodar (standalone TTY3 direto no DRM)

```
Ctrl+Alt+F3
lumo-tty.sh
```

Recovery garantido: Ctrl+Alt+F1 (volta Hyprland host) ou Ctrl+Alt+Backspace (exit limpo).

## Links

- Repo Galaxy: `ssh luizhcrds@192.168.0.106` -> `cd ~/Projects/lumo-shell`
- GitHub mirror (privado): https://github.com/Luizhcrs/lumo-os
- Docs estrategicos: Obsidian `1 - Projetos/Projeto - Lumo OS/`
- Roadmap fechado: Obsidian `20 - Roadmap Fechado v1.md`
