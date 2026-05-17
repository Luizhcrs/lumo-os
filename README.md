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
