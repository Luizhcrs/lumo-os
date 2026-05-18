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
- **DRM property "Broadcast RGB"** (A16): eDP-1 default = Automatic (0). Kernel i915 pode resolver pra Limited 16:235 dependendo EDID = banding/dither visivel. Forcar = Full (1) via `drm_device.set_property(connector, prop_handle, 1)` antes de `initialize_output`. Enum: Automatic=0, Full=1, Limited 16:235=2.
- **DRM modifiers Argb8888 Galaxy** (A16): swapchain recebe lista `[Invalid, I915_y_tiled_gen12_rc_ccs, ?, I915_y_tiled, I915_x_tiled, Linear]`. Mesa escolhe automatico melhor pra scanout — driver prefere Y-tiled CCS quando disponivel. Logar via `drm_output.with_compositor(|c| c.modifiers())`.
- **GlesRenderer blend func default** (A16): `glBlendFunc(GL_ONE=1, GL_ONE_MINUS_SRC_ALPHA=771)` = premultiplied, IGUAL Hyprland. Logar via `renderer.with_context(|gl| gl.GetIntegerv(BLEND_SRC_RGB))`. Nao precisa override.

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


## Pipeline cor sRGB final (FIXADO 2026-05-17 A19.4) — NAO MEXER

**Causa raiz historica**: smithay 0.7 GlesRenderer NAO faz CM. Textura SHM importada como GL_RGBA8 (LINEAR-treated) + shader output LINEAR direto = painel exibe LINEAR onde espera sRGB = banding, 2 cores em pills, dither visivel.

**Patches obrigatorios em `vendor/smithay/`**:

1. **`gles/format.rs`**: `Fourcc::Abgr8888 => (SRGB8_ALPHA8, RGBA, UNSIGNED_BYTE)`. Texturas sRGB-tagged, sampler converte sRGB->linear automatico.

2. **`gles/mod.rs`** linhas 808, 966, 1572, 1614: branches `SRGB8_ALPHA8` em paralelo a `RGBA8`. import_shm_buffer + import_memory + create_buffer + create_renderbuffer.

3. **`gles/shaders/implicit/mod.rs`** linha 51: `variant_for_format` aceita `SRGB8_ALPHA8` no match.

4. **`gles/shaders/implicit/texture.frag` + `solid.frag`** — output linear->sRGB **DEMULTIPLIED** (premul correct):
   ```glsl
   vec3 srgb_rgb;
   if (color.a > 0.0001) {
       vec3 lin = color.rgb / color.a;
       srgb_rgb = pow(lin, vec3(1.0/2.2)) * color.a;
   } else {
       srgb_rgb = vec3(0.0);
   }
   gl_FragColor = vec4(srgb_rgb, color.a);
   ```
   **NUNCA usar `pow(color.rgb, 1/2.2)` direto** — premul + gamma simples = bordas AA com cor diferente do centro (visual "2 azuis").

5. **Cargo.toml workspace**: `[patch.crates-io] smithay = { path = "vendor/smithay" }`.

**Wallpaper loading** (A19.4): chamar `LumoWallpaper::try_load(&mut renderer)` em AMBOS backends (winit.rs + drm.rs) apos GlesRenderer init. Esquecer drm.rs = wallpaper nao aparece em TTY3 real.

**Regra de ouro**: cor verdadeira (Hyprland-equivalent) exige todos 5 patches acima. Se voltar 2 cores ou dither: **NAO chutar**, verificar se algum patch foi revertido inadvertidamente.


## Bar layer-shell width (FIXADO 2026-05-17 A19.18) — NAO MEXER

**Causa raiz**: `LumoBar::new` inicializava `width: 1280`. Configure callback do smithay-client-toolkit 0.20 (`LayerSurfaceConfigure`) que deveria trocar pra 1920 **nem sempre dispara** no primeiro commit (smithay/compositor timing). Resultado: bar pintava em 1280 = pill direita aparecia "no meio" do output 1920.

**Fix**: init default `width: 1920` (output Galaxy nativo). Configure handler ainda atualiza se for chamado, mas nao depende dele pra primeiro render.

**Regra**: se output do compositor for diferente de 1920x1080, atualizar default OU implementar deteccao via OutputState antes do primeiro redraw. Hardcode + fallback robusto.


## Bar layer-shell click + dropdown (FIXADO 2026-05-17 A20.x) — NAO MEXER

**4 patches obrigatorios** pra bar receber click + abrir dropdown:

1. **`compositor/state.rs::surface_under`** (A20.2): incluir layer-shell surfaces. Padrao Smithay so busca `space.element_under` (toplevels). Sem isso, click em bar = compositor nao acha surface = nao roteia event. Z-order: Overlay > Top > Window > Bottom > Background.

2. **`bar/main.rs` loop** (A20.9): `dispatch_pending` sozinho NAO le events do socket. Precisa `prepare_read + poll + read` antes. Pattern:
   ```rust
   conn.flush().ok();
   if let Some(guard) = queue.prepare_read() {
       nix::poll::poll(&mut [PollFd::new(fd, POLLIN)], 50ms);
       guard.read();
   }
   queue.dispatch_pending(&mut state).ok();
   ```
   Sem isso, `seat capabilities` events nunca chegam, `new_capability` callback nunca dispara, pointer nunca eh adquirido.

3. **`bar/main.rs::new_capability`** (A20.x): usar `get_pointer_with_theme` (retorna ThemedPointer). `get_pointer` plain NAO dispatcha events automatico pro PointerHandler. ThemedPointer eh obrigatorio pro callback funcionar.

4. **Surface altura fixa** (A20.11): NUNCA chamar `layer.set_size()` dinamico apos init. Re-size durante runtime = flicker open/close cycle. Render dropdown em surface ja grande (BAR_HEIGHT + DROPDOWN_H sempre), com area transparente quando fechado. `exclusive_zone` permanece BAR_HEIGHT (toplevels nao afetam).
