# Lumo OS — Arquitetura

## Workspace Crates (13 membros)

| Crate | Path | Responsabilidade |
|-------|------|-----------------|
| `lumo-foundation` | `crates/foundation/lumo-foundation/` | Tokens de design (LFColor, LFTokens), constantes compartilhadas |
| `lumo-beam` | `crates/graphics/lumo-beam/` | Wrapper wgpu: buffers, memory management, device/queue lifecycle |
| `lumo-graphics` | `crates/graphics/lumo-graphics/` | Quad SDF, sombras, clipping, pipeline de render 2D |
| `lumo-text` | `crates/graphics/lumo-text/` | Shaping via cosmic-text, atlas de glifos, cache de layout |
| `lumo-animation` | `crates/graphics/lumo-animation/` | Spring physics massa-mola, presets LASpring, curvas Lumo ease |
| `lumo-input` | `crates/ui/lumo-input/` | Eventos normalizados, estado de pointer, gestos basicos |
| `lumo-kit` | `crates/ui/lumo-kit/` | Widgets (botoes, pills, toggles, dropdowns), state management |
| `lumo-wm` | `crates/compositor/lumo-wm/` | Compositor Wayland smithay 0.7, DRM-KMS backend, input handler |
| `lumo-ipc` | `crates/compositor/lumo-ipc/` | Protocolo IPC Unix socket, tipos de mensagem wm <-> bar |
| `lumo-gfx-core` | `crates/lumo-gfx-core/` | Abstracoes graficas compartilhadas wm + shell |
| `lumo-sensors` | `crates/system/lumo-sensors/` | Leitura sysfs: bateria, brilho, temperatura, platform profile |
| `lumoctl` | `crates/cli/lumoctl/` | CLI de controle runtime (brightness, theme, workspace) |
| `shell` | `shell/` | Binarios `lumo-bar` e `lumo-desktop` (layer-shell clients) |

## Fluxo de Dados

```
[libinput]
    |
    v
lumo-wm (compositor)
    |--- protocolo Wayland xdg_shell --> apps clientes
    |--- wlr-layer-shell              --> lumo-bar, lumo-desktop
    |--- IPC unix socket              --> lumo-bar (workspace, focus, appmenu)
    |
    v
[DRM-KMS / eDP-1]
```

Binarios do shell sao clientes Wayland normais mais canal IPC proprio:
- `lumo-bar` abre socket `$XDG_RUNTIME_DIR/lumo-wm.sock` para receber eventos `Workspaces`, `ActiveApp`, `AppMenu`.
- `lumo-desktop` recebe wallpaper e contexto de desktop via mesmo protocolo.

## IPC — Mensagens Principais

| Mensagem | Direcao | Payload |
|----------|---------|---------|
| `Workspaces` | wm -> bar | `active: u8, total: u8` |
| `ActiveApp` | wm -> bar | `title: String, class: String` |
| `AppMenu` | wm -> bar | `Vec<MenuItem>` (C5.1 AppMenu Registrar) |
| `SetWorkspace` | bar -> wm | `n: u8` |
| `SetTheme` | bar -> wm | `Light \| Dark` |

## Pipeline Cor sRGB

Cinco patches obrigatorios em `vendor/smithay/` — detalhes completos em `DEPS.md#pipeline-cor-srgb`.

Resumo: smithay 0.7 GlesRenderer e "naive" (sem CM). Patches adicionam:
1. `gles/format.rs`: texturas SHM importadas como `SRGB8_ALPHA8` (sampler converte sRGB->linear automatico).
2. `gles/mod.rs`: branches `SRGB8_ALPHA8` em 4 sites de import/create.
3. `gles/shaders/implicit/mod.rs`: aceita `SRGB8_ALPHA8` no match de formato.
4. Shaders `texture.frag` + `solid.frag`: output linear->sRGB demultiplied correto.
5. `Cargo.toml workspace`: `[patch.crates-io] smithay = { path = "vendor/smithay" }`.

**Regra de ouro**: nunca reverter patches — resultado = banding e "2 cores" em pills.

## Filewatcher e Hot Reload (L6)

`lumo-wm` observa `~/.config/lumo/theme.toml` via `inotify`. Mudancas disparam `ThemeReloaded` event no calloop, que propaga tokens atualizados para bar e desktop via IPC sem restart.

## AppMenu Registrar (C5.1)

Daemon DBus `com.canonical.AppMenu.Registrar` implantado no compositor. Apps GTK3+ carregados com `GTK_MODULES=appmenu-gtk-module` exportam seus menus via DBus. Compositor coleta e envia `AppMenu` IPC para `lumo-bar`.

## Backends Compositor

| Backend | Ativado por | Uso |
|---------|-------------|-----|
| Winit (nested) | default build | Desenvolvimento dentro de Hyprland host |
| DRM-KMS | feature `drm-backend` | Producao TTY3, acesso direto ao hardware |

## Wayland Protocols Suportados

xdg_shell, wlr-layer-shell, wl_shm, linux-dmabuf-v1, xdg-decoration, presentation-time, relative-pointer-v1, pointer-constraints-v1, pointer-gestures-v1, primary-selection, xdg-activation, fractional-scale, cursor-shape, wp-viewporter, wp-single-pixel-buffer, wp-presentation.

**Opt-in via env (ver ADRs)**:
- `xdg-toplevel-icon-v1` — `LUMO_ENABLE_TOPLEVEL_ICON=1` (ADR-003).
- `wp-color-manager-v1` — `LUMO_ENABLE_COLOR_MGMT=1` (ADR-002).

## ADRs

Decisoes arquiteturais em `docs/adr/`. Index e criterio em `docs/adr/README.md`.

## Bridge HTTP (apps/lumo-bridge)

Controle remoto via HTTP em `0.0.0.0:7778`. Auth Bearer token + rate limit per-peer (token bucket, 100rps/burst 50 default). Tuning via env `LUMO_BRIDGE_RPS` / `LUMO_BRIDGE_BURST`.
