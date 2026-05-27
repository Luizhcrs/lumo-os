# ADR-003 — xdg-toplevel-icon-v1 OFF por default

- **Status**: accepted
- **Data**: 2026-05-27 (W37.18)

## Context

`xdg-toplevel-icon-v1` permite client setar icon de janela via buffer Wayland.

Bug smithay 0.7.0: `XdgToplevelIconManager` registra `register_buffer_destruction_hook` ao chamar `set_icon` mas **nao desregistra** quando `icon.destroy()`. Buffer.destroy() subsequente (spec-compliant) dispara hook que tenta acessar icon ja livre → protocol error injetado de volta pro client → "Broken pipe".

Reproducao 100% no Chromium 130+ via DRM headless. Root cause documentado em `docs/incidents/2026-05-27-W37-chromium-broken-pipe.md`.

Memory: `bug_smithay_xdg_toplevel_icon_leak.md`.

## Decision

`XdgToplevelIconManager` **nao instanciado por default**.

Opt-in via env:
```bash
LUMO_ENABLE_TOPLEVEL_ICON=1 lumo-wm
```

Field `state.xdg_toplevel_icon_manager: Option<...>` default `None`. Helper `should_enable_toplevel_icon_manager()` lazy-init.

## Consequences

**Positivas**:
- Chromium nao crash mais (validado).
- Apps sem icon manager continuam funcionando (degrad. cosmetica = icon padrao do compositor).

**Negativas**:
- Apps que tentam set_icon: requisicao silently no-op no client side (binding nao encontra global).
- Algumas DEs mostrariam icon especifico do app na taskbar.

**Reverter quando**:
- Smithay PR fix do leak (desregistrar hook em destroy).
- Lumo implementa toplevel icon manager propio com lifetime correto.

## Upstream

Report smithay tracker. Repro minimal: `chromium --headless about:blank` em DRM session com global registrado.
