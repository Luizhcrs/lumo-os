# agent-harness

Tooling pra agente LLM controlar o Lumo OS sem o Luiz mexendo no laptop.

## O que isso resolve

Validacao visual de UI (dropdowns, animacoes, paint bugs) era 100% manual: Luiz abria a sessao, clicava, descrevia o que via. Esse harness deixa um agente LLM:

1. Tirar screenshot do estado atual
2. Clicar em coordenadas absolutas
3. Tirar screenshot do estado posterior
4. Diferenciar pixel-by-pixel pra decidir se a UI respondeu

## Pre-requisitos

- `grim` (screenshot wlr-screencopy) -- pacman
- `ydotool` + `ydotoold` rodando -- pacman; servico user habilitado
- `imagemagick` (compare) -- pacman
- Usuario em grupo `input` (acesso `/dev/uinput`)
- udev rule em `/etc/udev/rules.d/80-uinput.rules`:

```
KERNEL=="uinput", GROUP="input", MODE="0660", OPTIONS+="static_node=uinput"
```

E `sudo modprobe uinput`. Tudo isso ja configurado no Galaxy Book 4 em 2026-05-19.

## Scripts

### `start-lumo-nested.sh`

Detecta lumo-wm rodando e exporta env (`WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `YDOTOOL_SOCKET`) em `/tmp/lumo-agent/env.sh`.

Modo `live` (default): usa o lumo-wm DRM ativo no tty3.
Modo `nested`: NAO implementado. Bloqueio documentado no proprio script -- lumo-wm.rs faz auto-spawn de bar/desktop apenas em DRM path. Pra nested validar precisa: (a) host wayland session ativa OU (b) patch deixando `LUMO_AUTOSTART=1` forcar spawn no path winit.

### `screenshot.sh [nome.png]`

Captura tela inteira via `grim`. Output em `/tmp/lumo-agent/`. Imprime o path.

### `click.sh <x> <y> [left|right|middle]`

Move o mouse pra `(x,y)` e clica. Coordenadas absolutas (pixels).

Implementacao: `ydotool mousemove --absolute` + `ydotool click`. Eventos uinput entram no kernel -> libinput -> smithay -> lumo-wm igual mouse fisico. Sem patch IPC.

### `validate-dropdown.sh`

E2E completo: screenshot, click na pill bateria, screenshot, diff, veredito.

Threshold default: delta > 5000 pixels = dropdown abriu. Override via env `LUMO_PILL_BAT_X` / `LUMO_PILL_BAT_Y` se as coordenadas mudaram (re-medir em state.rs:265).

## Fluxo tipico do agente

```bash
cd ~/Projects/lumo-shell
./scripts/agent-harness/validate-dropdown.sh
# le os PNGs em /tmp/lumo-agent/{pre,pos,diff}.png pra interpretar visual
```

## Limitacoes conhecidas

- Sem nested: precisa lumo-wm ativo em tty real. CI headless precisa branch nested.
- Coordenadas hardcoded em 1920x1080. Outros modes -> editar.
- ydotool depende de uinput; container Docker sem privileged nao roda.
- Screenshot grim captura output inteiro; pra crops usar `grim -g "$(slurp)"`.
