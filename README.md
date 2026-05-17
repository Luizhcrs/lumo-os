# luiz-shell

GPUI gallery — 18 widgets Apple-fluid em Rust, GPU-acelerado via wgpu.

Lab pra calibrar tokens emerald/Geist antes do shell completo sobre Hyprland.

## Hardware-alvo

Samsung Galaxy Book 4 U300 — Intel UHD Raptor Lake-P, Vulkan/Mesa, Wayland/Hyprland.

## Demos

1. Spring button — press + spring release com overshoot
2. Glide toggle — fill horizontal (signature propria nao iOS)
3. Stagger reveal — items aparecem em sequencia 70ms
4. Hover lift — card sobe + accent border
5. Toast stack — slide-in lateral, max 5
6. Modal overlay — backdrop + card com fade
7. Bottom sheet — slide bottom-up
8. Page transition — push/pop stack
9. Segmented control — pill animado entre opcoes
10. Skeleton shimmer — opacity pulse durante load
11. Bounce list — overscroll feel
12. Pinch zoom — trackpad pinch real (zwp_pointer_gesture_pinch_v1)
13. Carousel snap — pill indicator
14. Swipe to delete — touchpad horizontal wheel
15. Context menu — dropdown clique
16. Press and hold — async timer 800ms
17. Tilt card — hover lift toggle
18. Stretch banner — height scale

## Build + run

```
cargo build
./target/debug/luiz-shell
```

## Tokens

- accent: emerald-600 (#059669)
- bg: deep ink (#0a0a0c)
- font: Geist + Geist Mono
- curvas: SwiftUI .smooth (cubic-bezier(.32,.72,0,1))

## Keyboard

- Left/Right — anterior/proximo demo
- Esc — fecha modal/sheet/context menu

## A7 - Triade completa: cursor real + bar lapidada + foot tema

Tres bugs visuais resolvidos numa unica passada (anti-padrao tapar-buraco):

### Bug 1 - cursor azul translucido -> seta cinza Adwaita real

Causa raiz: A6.5 carregava `xcursor::Image.pixels_argb` (bytes ordem `[A,R,G,B]`)
para um `MemoryRenderBuffer` com `Fourcc::Argb8888`. Em little-endian esse
formato espera bytes `[B,G,R,A]`. Resultado: byte A virava B (azul!), byte B
virava A (alpha quebrado).

Fix:
- usar `pixels_rgba` (bytes `[R,G,B,A]`, vem direto do file format Xcursor)
- pre-multiplicar canal RGB pelo alpha (Smithay assume premultiplied)
- montar buffer com `Fourcc::Abgr8888` (LE memoria = `[R,G,B,A]`) - bate exato

### Bug 2 - bar so "quadrado verde" -> bar completa lapidada

Causa raiz: gpui-platform exigia superficie GPU via linux-dmabuf, e nosso
compositor expoe apenas wl_shm + GLES interno. lumo-bar panicava em "No GPU
adapter found". Sem bar, sobrava no compositor o `brand_dot_element` 8x8
emerald do overlay - exatamente o "so quadrado verde" reportado.

Fix:
- bar reescrita em smithay-client-toolkit + tiny-skia (software rendering
  via SHM). Roda em qualquer compositor wayland sem GPU.
- layout final: brand dot esquerda; workspaces 1-5 (pill emerald no ativo);
  wifi/bateria/relogio HH:MM/power button direita. Tudo via primitivas
  vetoriais (sem font carregado, digitos 7-segmento em vector). Bateria le
  `/sys/class/power_supply/BAT[01]/capacity`. Wifi le `/sys/class/net/wl*/operstate`.
  Relogio via `chrono::Local`.
- brand_dot_element removido do overlay do compositor (era responsabilidade
  da bar, nao do WM).

### Bug 3 - foot sem tema Lumo -> tema aplicado

Causa raiz: spawn `Command::new("foot")` herdava env do lumo-wm, mas quando
lumo-wm subia de contexto sem `HOME`/`XDG_CONFIG_HOME` populados (caso
typico de servico systemd), foot caia em config default branco/preto.

Fix: spawn explicito propaga `HOME`, `XDG_CONFIG_HOME`, `LC_CTYPE=C.UTF-8`,
e passa `-c $HOME/.config/foot/foot.ini` direto pro foot achar o tema
ink_deep + emerald + JetBrainsMono.

### Validacao

Build limpo `cargo build --release -p lumo-wm -p lumo-shell`. Compositor
nested rodando, lumo-bar render completo + foot tema validados por screenshot
do Hyprland host (`/tmp/lumo-clean.png`, copiado pra Windows como
`lumo-test-A7.png`). Cursor pixel sample na area (645-675, 350-380) confirma
zero pixels azuis - todos grayscale (#FFFFFF, #E2E2E3, #1C1C1D etc.).

### Pendente (proximo ciclo)

- Implementar `wlr_screencopy_v1` no compositor pra `grim` funcionar diretamente
  (hoje screenshot e via host Hyprland)
- Implementar `linux_dmabuf_v1` + suporte SHM/dmabuf import pra clientes GPU
  (vivaldi, chromium, GPUI)
- Workspaces da bar ainda sao estatico 1-5; falta wire-up via IPC compositor
- Bar nao consome eventos de input ainda (sem hover/click feedback)
- foot dentro do nested precisa wire-up de zwp_text_input + decoration manager

## A8 - DRM backend scaffolding + IPC workspaces + moldura desktop

Tres frentes paralelas pra antecipar Fase 3 (saida do Hyprland host).

### Frente 1 - Backend DRM/KMS (Etapa 1: bring-up)

`crates/lumo-wm/src/backend/drm.rs` gated por feature `drm-backend`. Selecao
via env `LUMO_WM_BACKEND=drm|winit` (default `winit`, sempre seguro).

Etapa 1 cobre: `LibSeatSession::new` (logind/seatd), `udev::primary_gpu`,
`DrmNode::from_path`, log de connectors. Sai cleanly se nao tem TTY (smoke
test friendly). Etapa 2 (page-flip loop + GbmDevice + DrmCompositor +
GlesRenderer + libinput) fica na proxima iteracao A9. Justificativa:
portar 1500 linhas de anvil em uma tacada vira festival de bug que so
aparece em TTY fisico (sem repro via SSH); Etapa 1 isola bring-up.

Build: `cargo build --release --features lumo-wm/drm-backend --bin lumo-wm`.

Run: `./scripts/lumo-tty.sh` DENTRO de TTY (Ctrl+Alt+F3). Nao funciona via
SSH.

### Frente 2 - Workspaces IPC

Socket unix em `$XDG_RUNTIME_DIR/lumo-wm.sock`. JSON line-delimited.

Crate compartilhada `crates/lumo-ipc/` com tipos serde
(`LumoEvent::Workspaces`, `LumoCommand::Switch`).

Server no lumo-wm: `crate::ipc` integrado em calloop (mesmo event loop que
Wayland). SUPER+1..5 -> `state.set_workspace(N)` -> broadcast pros clients.
Snapshot inicial enviado no connect. Sem thread extra, sem async runtime.

Client no lumo-bar: conecta no startup (best-effort), drain non-blocking
8ms. Click numa pill aplica local imediato + envia `Switch{to:N}` (memory
feedback_input_feedback_imediato). Anti-burst 100ms.

Test manual (Hyprland host):
```
./scripts/lumo-test.sh
echo '{type:switch,to:3}' | socat - UNIX-CONNECT:$XDG_RUNTIME_DIR/lumo-wm.sock
```

### Frente 3 - Moldura desktop + sombras pretas

Compositor desenha corner mask (quads pretos solidos 10x10 nos 4 cantos do
output) + sombra preta neutra rgba(0,0,0,0.4) offset (0,+8) bleed 4px atras
de cada toplevel.

Memory feedback_zero_neon_glow: zero glow colorido. Memory
feedback_design_lapidado: cantos via quad em vez de shader custom = ~200
linhas a menos + zero risco de regressao no path principal.

Limitacao conhecida: smithay `render_output` coloca custom elements POR CIMA
dos space elements; sombras ficam visualmente atras de janelas com leve
sobreposicao nas bordas. Fix futuro: separar `space.render_elements()` e
intercalar.

### Como sair se o lumo-wm DRM travou

Em ordem de preferencia:

1. **Ctrl+Alt+Backspace** dentro do lumo-wm -> `Action::Quit` -> exit clean.
2. **Ctrl+Alt+F1** -> volta pro TTY1 (host Hyprland normalmente).
3. **Ctrl+Alt+F2** -> volta pro TTY2 (display manager / login).
4. SSH de outra maquina:
   ```
   sudo pkill -9 lumo-wm
   sudo systemctl restart display-manager
   ```
5. Hard reset: power button longo (ultimo recurso).

### Validacao A8

- `cargo build --release` (winit default) zero warning, finished em 31s.
- `cargo build --release --features lumo-wm/drm-backend --bin lumo-wm` zero
  warning, finished em 47s primeira vez (download deps drm/gbm/libseat/input/udev).
- IPC smoke test: socket criado em `$XDG_RUNTIME_DIR/lumo-wm.sock`,
  `socat`/`nc -U` recebe broadcast inicial e aceita `{type:switch,to:N}`.
- DRM smoke test fora de TTY: `LUMO_WM_BACKEND=drm ./target/release/lumo-wm`
  falha em `LibSeatSession::new` com mensagem-guia clara (esperado).
- Screenshot moldura + sombras: `lumo-test-A8-winit.png` no Windows.

### Pendente A9

- DRM Etapa 2: GbmDevice + DrmCompositor + page-flip loop completo
- libinput backend (teclado/mouse direto via udev)
- VT switch real (`session.change_vt(n)`) - hoje so log
- Sombras render order: usar `space.render_elements()` separadamente
- Watchdog 5s DRM render stalled -> exit code 2

## A9 etapa 2B - DRM render path real + Hyprland-aware

Lumo agora renderiza de verdade no DRM/KMS. Pipeline completo: GbmDevice ->
EGLDisplay+EGLContext -> GlesRenderer -> DrmOutputManager -> DrmOutput.
Frame timer 60Hz dispara render; page-flip event marca submitted.

### Como testar Lumo TTY3 (com Hyprland parado)

Lumo precisa ser DRM master. Hyprland host nao pode rodar simultaneo.

Workflow dev:

1. Hyprland tty1 normal.
2. `Ctrl+Alt+F3` -> autologin (se ativo) -> rode `./scripts/lumo-tty.sh`.
3. Script detecta Hyprland em execucao, avisa em 3s, mata via `hyprctl
   dispatch exit` (limpo) ou SIGTERM/SIGKILL (fallback).
4. Espera Hyprland sair, build idempotente, sobe lumo-wm DRM master.
5. Tela esperada: fundo `ink_deep` (#0a0a0c) escuro + cursor cinza Adwaita
   + 4 quads pretos nos cantos (simulando borda arredondada). Toplevels
   reais (terminais, apps) virao na Etapa 2C quando o display dispatch
   estiver wireado no event loop DRM.
6. Sair: `Ctrl+Alt+Backspace` (lumo-wm exit clean).
7. Reabrir Hyprland: script da hint final; geralmente `Ctrl+Alt+F1` +
   login fresh OU rodar `nohup Hyprland > /tmp/hypr.log 2>&1 &`.

### Re-ativar autologin TTY3 (opcional)

Foi movido pra `/tmp/autologin.conf.bak` em diagnostico anterior. Pra
re-aplicar quando quiser:

```
sudo cp /tmp/autologin.conf.bak /etc/systemd/system/getty@tty3.service.d/autologin.conf
sudo systemctl daemon-reload
sudo systemctl restart getty@tty3.service
```

### Mudancas tecnicas etapa 2B

- `crates/compositor/lumo-wm/src/backend/render_common.rs` (novo, ~150 LoC):
  cursor (xcursor + solid fallback), corner mask, shadows -- consumidos
  por winit.rs e drm.rs. Memory feedback_design_lapidado: zero duplicacao,
  visual consistente entre Lumo nested e Lumo TTY.
- `drm.rs`: 280 -> ~470 LoC. Pipeline real ativo. `DrmOutputManager` +
  `DrmOutput` encapsulam alocador GBM + framebuffer exporter + swapchain.
- `winit.rs`: refatorado pra usar `render_common`. Visual identico ao 2A.
- Frame timer 60Hz (16ms) + page-flip event source -> input lag deve
  ficar < 16ms (memory feedback_input_feedback_imediato: libinput dispatch
  entre frames).
- VRR skipped (Galaxy U300 painel 60Hz fixo).
- Cursor HW plane skipped (low priority; software fallback via xcursor
  MemoryRenderBuffer ja funciona).
- Toplevels reais (xdg-shell render) ficam pra Etapa 2C: requer estender
  o `LumoCustomElement` (render_elements! macro) com variante `Space` que
  wrappa `WaylandSurfaceRenderElement`, e ligar display dispatch dentro
  do event loop DRM (hoje o main.rs so dispatcha em winit path).

### Pendente A9 etapa 2C

- LumoCustomElement::Space -> render de toplevels reais
- Display dispatch dentro do event loop DRM (clients Wayland precisam ser
  servidos durante session DRM)
- linux-dmabuf-v1 pra clients GPU
- Hot-plug real (atualmente so loga)
- Cursor HW plane (overlay scan-out direct, low priority)

## A19 - Wallpaper + Dev nested workflow

### Wallpaper (textura de fundo)

Backend (winit ou drm) carrega imagem do disco no startup, sobe como
textura GL via `renderer.import_memory` e desenha como element no
fundo do stack a cada frame.

Path resolvido em ordem:

1. Env `LUMO_WALLPAPER` se setada
2. `$HOME/.config/lumo-wallpaper.jpg` (default)

Falha de load (arquivo ausente, decode bug, GL upload erro) = warn no
log + cai pra clear color (comportamento A18). Compositor nunca trava
por wallpaper.

Decode via crate `image` 0.25 (jpeg + png minimo, sem default features).
Upload em `Fourcc::Abgr8888` (mesmo formato do cursor xcursor desde A7).

Strategy de scale: stretch pro tamanho do output. Wallpaper padrao
1999x1124 vs display 1920x1080 sao ambos ~16:9 — distorcao invisivel.

### Dev nested workflow

Duas formas de subir lumo-wm pra iterar:

- `./scripts/lumo-dev.sh` — **debug live nested**. Roda lumo-wm em winit
  dentro do Hyprland host (qualquer terminal). Janela ~1280x720 aparece;
  spawn de apps via `WAYLAND_DISPLAY=$SOCKET foot &`. Cor pipeline pode
  diferir do DRM real (HDR/dither do painel ficam fora), mas layout
  e render basico iteram em segundos. Ctrl+C mata tudo.

- `./scripts/lumo-tty.sh` — **polish final TTY3**. Roda lumo-wm em DRM
  direto no TTY3 (mata Hyprland host primeiro). Color management e dither
  reais. Usar quando precisar validar pipeline visual completo.

Workflow recomendado: iterar visual no nested, validar final no TTY.
