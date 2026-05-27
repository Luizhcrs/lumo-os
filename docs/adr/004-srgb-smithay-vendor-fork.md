# ADR-004 — Vendor smithay com sRGB patches

- **Status**: accepted
- **Data**: 2026-05-XX (pre-W37)

## Context

Smithay 0.7.0 `GlesRenderer`:

- Cria texturas internas com `GL_RGBA8` (linear), mas amostragem assume sRGB.
- Resultado: cores washed-out em buffers SHM RGB linear.
- Wallpaper, SSD titlebar, dropdowns ficavam pastel.

Upstream issue aberta, sem fix em release.

## Decision

Vendor smithay em `vendor/smithay/` com patches sRGB aplicados:

- `GL_SRGB8_ALPHA8` em textures internas.
- Fragment shader corrigido pra sample sRGB.
- Patch documentado em `vendor/smithay/PATCHES.md`.

`Cargo.toml` aponta pra path local:
```toml
smithay = { path = "vendor/smithay" }
```

## Consequences

**Positivas**:
- Cores corretas. Wallpaper, dropdowns, titlebar com gamma certo.
- Permite outros patches local-only (ex: W37.18 toplevel icon gating).

**Negativas**:
- Update upstream nao automatico. Precisa rebase manual de patches.
- CI precisa do path vendored.

**Migracao saida**:
- Quando smithay 0.8+ merge sRGB fix: remove vendor, volta crate version.
- Mantem `PATCHES.md` como historico.
