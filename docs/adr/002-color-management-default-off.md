# ADR-002 — wp-color-manager-v1 OFF por default

- **Status**: accepted
- **Data**: 2026-05-27 (W37.15)

## Context

`wp-color-manager-v1` (staging) e o protocolo Wayland pra HDR/wide-gamut signaling.

Chromium 130+ negocia color management na startup. Lumo-wm exporta o global; smithay 0.7.0 stage implementation tem 2 violacoes spec:

1. `ImageDescriptionInfo.ready(id=0)` — spec: "Zero is reserved as an invalid id number". Chromium fecha conexao.
2. `target_primaries` mandatory pra parametric, smithay nao chama. Chromium falha parse.

Fixes locais (W37.12 ready_id ≥ 1, W37.13 target_primaries) reduziram falhas mas ainda intermitente em headless. Chromium se da bem **sem** o global (negocia sRGB fallback via primary buffer attach).

## Decision

`wp-color-manager-v1` **nao registrado por default**.

Opt-in via env var:
```bash
LUMO_ENABLE_COLOR_MGMT=1 lumo-wm
```

Helper `should_enable_color_manager()` em `state.rs` + `is_env_truthy()` strict parsing.

## Consequences

**Positivas**:
- Chromium funciona out-of-box (validado 2026-05-27).
- Sem regressao em apps que nao usam color mgmt (~99% dos casos).
- Lumo continua com sRGB pipeline correto (ver ADR-004).

**Negativas**:
- HDR signaling indisponivel ate flag ser ligada.
- Wide-gamut apps (DaVinci, Krita HDR) precisam env var.

**Reverter quando**:
- Smithay merge fix upstream (target_primaries + id ≥ 1).
- Lumo migra pra impl propria de color management.
