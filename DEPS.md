# DEPS — versoes fixadas + docs

**Regra**: agente CONSULTA este arquivo ANTES de mexer com dep externa. Versao fixa = doc certo, evita conflito API.

## Crates Rust principais

| Crate | Versao fixa | Docs canonicos | Notas |
|-------|-------------|----------------|-------|
| smithay | `0.7.0` | https://docs.rs/smithay/0.7.0/smithay/ | Wayland framework, anvil reference em `examples/anvil/src/udev.rs` |
| smithay-client-toolkit | `0.20` | https://docs.rs/smithay-client-toolkit/0.20/ | bar/dock client side |
| wgpu | `23` | https://docs.rs/wgpu/23/ | nosso LumoBeam |
| winit | `0.30` | https://docs.rs/winit/0.30/ | janela backend nested |
| cosmic-text | `0.12` | https://docs.rs/cosmic-text/0.12/ | text shaping |
| tiny-skia | `0.11` | https://docs.rs/tiny-skia/0.11/ | rasterizer SHM bar. **Pixmap.data() retorna RGBA premultiplied** |
| fontdb | `0.18` | https://docs.rs/fontdb/0.18/ | font discovery |
| etagere | `0.2` | https://docs.rs/etagere/0.2/ | atlas allocator |
| xcursor | `0.3` | https://docs.rs/xcursor/0.3/ | cursor theme reader. **Bytes RGBA non-premul, swap pra Abgr8888 wayland** |
| calloop | `0.14` | https://docs.rs/calloop/0.14/ | event loop |
| libseat | (via smithay backend) | https://git.sr.ht/~kennylevinsen/seatd | libseat env `LIBSEAT_BACKEND=logind` se seatd falhar |
| input (libinput rust) | `0.9.1` | https://docs.rs/input/0.9.1/ | libinput backend |
| chrono | `0.4` | https://docs.rs/chrono/0.4/ | clock + data PT-BR |
| anyhow | `1` | https://docs.rs/anyhow/1/ | error |
| tracing | `0.1` | https://docs.rs/tracing/0.1/ | logs |
| serde | `1` | https://docs.rs/serde/1/ | IPC |
| serde_json | `1` | https://docs.rs/serde_json/1/ | IPC |

## Sistema (Galaxy Book 4 U300, Arch Linux)

| Componente | Versao | Notas |
|------------|--------|-------|
| Kernel | linux 7.0.7-arch2-1 | DRM/KMS estavel |
| Mesa | 26.0.6 | Vulkan + GLES, dmabuf modifiers Intel auto |
| i915 driver | mainline | FRC dithering 6-bit panel auto. `i915.enable_dithering=0` desliga |
| libinput | 1.31.2-1 | touchpad accel adaptive |
| seatd | 0.9.3-1 | NAO usar `seatd-launch` (precisa cap socket bind) |
| Hyprland host | tty1 referencia visual | source: https://github.com/hyprwm/Hyprland |
| EndeavourOS base | Arch derivative | |

## Hardware

| Item | Spec |
|------|------|
| CPU | Intel Raptor Lake-P U300 1P+4E HT 6 threads |
| GPU | Intel UHD Xe-LP Gen12.1 48 EUs |
| RAM | 8GB LPDDR4 4267 |
| Display | Innolux 1920x1080 60Hz eDP-1 **TN 6-bit + FRC** (dithering hardware temporal) |
| DRM device | `/dev/dri/card1` (NAO card0, esse nao existe) |
| Render node | `/dev/dri/renderD128` |
| TTY livre Lumo | tty3 (tty1=Hyprland host) |

## Wayland protocols suportados lumo-wm hoje

- xdg_shell (toplevels)
- wlr-layer-shell (bar/dock)
- wl_shm (clients software)
- linux-dmabuf-v1 (clients GPU)
- xdg-decoration
- presentation-time
- relative-pointer-v1
- pointer-constraints-v1
- pointer-gestures-v1
- primary-selection
- xdg-activation
- fractional-scale
- cursor-shape
- xdg-toplevel-icon

## Descobertas tecnicas pra preservar

- **GL_FRAMEBUFFER_SRGB**: Hyprland NAO usa. Faz CM manual via shader uniforms. Smithay 0.7 GlesRenderer = "naive" blend sRGB.
- **Tiny-skia Pixmap**: armazena **RGBA premultiplied** internamente. `data()` retorna premul. So precisa swap RGBA->BGRA pra wl_shm Argb8888. NAO premultiplicar de novo (era bug A15.1).
- **Cosmic-text glyph color**: usar **alpha mask only** (grayscale AA) pra evitar rainbow subpixel artifact em painel sem LCD-RGB stripe.
- **EGL_KHR_gl_colorspace default**: LINEAR. sRGB precisa attribute explicito na surface creation. Smithay 0.7 nao expoe.
- **DRM master**: smithay 0.7 silencia `acquire_master_lock` falha. Kernels novos concedem modeset mesmo sem master tag. Nao abortar.
- **PointerMotion vs PointerMotionAbsolute**: mouse/touchpad emitem **PointerMotion (delta)**. PointerMotionAbsolute eh raro (touchscreen). Codigo A11.9.
- **libinput acceleration**: config via `DeviceAdded` event, NAO no init estatico. AccelProfile::Adaptive default suave.
- **seatd vs logind**: libseat tenta seatd primeiro, fallback logind. Em logind, `Active=yes` na session pra DRM master.

## Referencias externas chave

- Hyprland renderer: https://github.com/hyprwm/Hyprland/tree/main/src/render
- Hyprland OpenGL: https://github.com/hyprwm/Hyprland/blob/main/src/render/OpenGL.cpp
- Hyprland aquamarine backend: https://github.com/hyprwm/aquamarine
- Smithay anvil (reference compositor): https://github.com/Smithay/smithay/tree/master/anvil
- EGL_KHR_gl_colorspace spec: https://registry.khronos.org/EGL/extensions/KHR/EGL_KHR_gl_colorspace.txt
- Skia color management: https://skia.org/docs/user/color/
- Intel i915 kernel docs: https://docs.kernel.org/gpu/i915.html
- Wayland protocols: https://wayland.app/protocols/

## Workflow obrigatorio agente

ANTES de mexer com dep externa:
1. Le este DEPS.md
2. Confirma versao fixada
3. Consulta docs.rs com versao exata na URL (ex: `https://docs.rs/smithay/0.7.0/...`)
4. Se versao desatualizada, registra aqui antes de bump

NAO chutar API. NAO usar `cargo search` ou `crates.io` latest sem confirmar compat.
