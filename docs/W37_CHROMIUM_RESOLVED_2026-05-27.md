# W37 — Chromium Wayland Resolvido (2026-05-27)

## Resumo

Chromium falhava ao spawnar no Lumo Wayland com **"Fatal Wayland communication
error: Broken pipe"** apos handshake completo xdg-shell. Causa raiz: bug em
smithay 0.7.0 `XdgToplevelIconManager` que dispara `wl_display.error` falso
positivo. Cliente recebe error → fecha conexao.

Apos 8 fixes incrementais (W37.11 → W37.18), Chromium roda normal no Lumo
com about:blank, tabs, URL bar e close button funcionais. Validado via
screenshot e bridge `/screenshot`.

## Sintomas

```
[ERROR] '--ozone-platform=wayland' is not compatible with Vulkan.
        Consider switching to '--ozone-platform=x11' or disabling Vulkan.
[ERROR] Fatal Wayland communication error: Broken pipe.
```

Cliente Chromium morria em ~357ms apos `systemctl --user start chr-test`.

Apps Wayland-native (foot, lumo-files Iced) NAO eram afetadas.

## Investigacao

### Etapa 1: WAYLAND_DEBUG=client (cliente)

Mostrou Chromium completando todo handshake xdg-shell ate o ultimo
`ack_configure`. Broken pipe acontecia depois. Sem `wl_display.error` no
client log → server fechava socket sem enviar protocol error visivel?

### Etapa 2: WAYLAND_DEBUG=server (compositor)

`WAYLAND_DEBUG=server ./target/release/lumo-wm 2>&1 | tee /tmp/lumo-wm-tty.log`

Capturou a linha smoking gun (server-side):

```
[1585201.408][rs] -> wl_display@1.error(xdg_toplevel_icon_v1@35[4], 3,
    Some("The provided buffer has been destroyed before the toplevel icon"))
```

Server estava emitindo protocol error pre Chromium destruir buffer apos
icon destroy.

### Etapa 3: Codigo smithay

`~/.cargo/registry/.../smithay-0.7.0/src/wayland/xdg_toplevel_icon.rs:360`

```rust
data.register_buffer_destruction_hook(buffer.clone(), shm, {
    let icon = icon.clone();
    move || {
        icon.post_error(
            xdg_toplevel_icon_v1::Error::NoBuffer,
            "The provided buffer has been destroyed before the toplevel icon",
        )
    }
});
```

Comentario:

```rust
// Let's listen for buffer destruction event to catch no_buffer protocol error
// This hook has to be unregistered once the icon is destroyed
```

**Bug**: hook nunca e desregistrado em `destroyed()`. Buffer destroy
posterior dispara callback que emite error com icon ja morto.

### Etapa 4: Spec wp_toplevel_icon_v1

Sequencia Chromium e VALIDA segundo spec:

1. `icon.add_buffer(buffer)`
2. `icon_manager.set_icon(toplevel, icon)`
3. `icon.destroy()` — spec permite
4. `buffer.destroy()` — spec permite apos icon destroy

Comentario na spec: "The buffers attached to this icon may safely be
destroyed after the icon is destroyed."

## Stack de Fixes W37.11-18

Cada commit foi parte do diagnostico ate isolar bug verdadeiro.

| Sub | Fix | Origem |
|---|---|---|
| W37.11 | Protocols modernos (viewporter, spbuf, presentation) | agent report wayland gaps |
| W37.12 | `ready(N>=1)` spec compliance | spec read |
| W37.13 | `target_primaries` mandatory adicionado | spec read |
| W37.14 | env toggle `LUMO_ENABLE_COLOR_MGMT` | workaround |
| W37.15 | color_manager OFF default | safety |
| W37.16 | send_configure storm prevention | hipotese |
| W37.17 | systemd-user service automacao | infra teste |
| **W37.18** | **xdg_toplevel_icon OFF (ROOT FIX)** | WAYLAND_DEBUG=server |

## Fix Final W37.18

`crates/compositor/lumo-wm/src/state.rs`:

```rust
let xdg_toplevel_icon_manager: Option<XdgToplevelIconManager> =
    if std::env::var("LUMO_ENABLE_TOPLEVEL_ICON").is_ok() {
        Some(XdgToplevelIconManager::new::<Self>(&display_handle))
    } else {
        None
    };
```

Global `xdg_toplevel_icon_manager_v1` NAO registrado por default.
Chromium pula icon support gracefully. Opt-in via env quando smithay
upstream corrigir.

## Validacao

- chr-test.service spawna Chromium normalmente (ativo, nao morre)
- about:blank carrega com URL bar + tabs + close button
- WAYLAND_DEBUG mostra bind/unbind limpo, sem display.error
- Apps wayland-native intactas (foot, lumo-files, lumo-bar)
- Sem regressao em 185+ tests existentes do lumo-wm

## Apps testadas

| App | Status | Notas |
|---|---|---|
| Chromium 148 | ✓ FUNCIONA | about:blank, tabs, URL bar |
| foot | ✓ FUNCIONA | Wayland-native terminal |
| lumo-files | ✓ FUNCIONA | Iced 0.13 |
| lumo-bar | ✓ FUNCIONA | layer-shell native |
| Mousepad | parcial CSD | GTK3 ignora ServerSide (separado, W37.8 cobriu) |
| Kate | a testar | Qt5 |
| Firefox | a testar | provavel OK |

## Infra de Teste

`scripts/chr-test.service` — systemd-user unit pra spawn determinista
sem dependencia de SSH (SSH dropava bg spawns).

```bash
cp scripts/chr-test.service ~/.config/systemd/user/
systemctl --user daemon-reload
systemctl --user start chr-test
tail -f /tmp/chr_dbg.log
```

## Upstream TODO

Reportar issue em https://github.com/Smithay/smithay:

> `XdgToplevelIconManager::request` registra `register_buffer_destruction_hook`
> mas nao desregistra em `destroyed()`. Cliente cumprindo spec
> (`icon.destroy()` antes de `buffer.destroy()`) recebe protocol error
> falso positivo. Chromium 148 quebra. Patch: armazenar hook ID e
> remover em `destroyed` callback OR usar `is_alive` check no closure.

## Sources

- [Wayland color-management-v1 spec](https://wayland.app/protocols/color-management-v1)
- [Wayland xdg-toplevel-icon-v1 spec](https://wayland.app/protocols/xdg-toplevel-icon-v1)
- [Hyprland Chromium CM crash discussion](https://github.com/hyprwm/Hyprland/discussions/11843)
- [Brave CM monitor switch crash](https://github.com/brave/brave-browser/issues/49921)
- [omarchy CM workaround](https://github.com/basecamp/omarchy/issues/4610)
- [Sway Chromium broken pipe](https://issues.chromium.org/issues/40817882)
- [smithay 0.7.0 xdg_toplevel_icon.rs:360](https://github.com/Smithay/smithay/blob/master/src/wayland/xdg_toplevel_icon.rs)

## Commits

```
6f29990 fix(wm): W37.18 RESOLVE Chromium broken pipe - xdg_toplevel_icon OFF
cbcfe23 chore(test): W37.17 systemd-user unit pra automatizar test Chromium
b85910a fix(wm): W37.16 evita send_configure storm em commit handler
bf9b3b1 fix(wm): W37.15 wp_color_manager_v1 OFF por default (workaround Chromium)
acb0b32 feat(wm): W37.14 env LUMO_DISABLE_COLOR_MGMT pra workaround Chromium
2ad682a fix(wm): W37.12+13 color_management protocol compliance
ac9ef5b feat(wm): W37.11 protocols Wayland modernos - viewporter + spbuf + presentation
```
