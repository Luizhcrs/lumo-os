# lumo-decor

libdecor plugin nativo Lumo OS. Apps que usam libdecor (GTK4/libadwaita/Firefox/mpv/Blender/SDL3) automaticamente pegam titlebar Lumo unificada.

## Build

```bash
cargo build --release -p lumo-decor
# Output: target/release/liblumo_decor.so
```

## Install

```bash
sudo install -Dm755 target/release/liblumo_decor.so /usr/lib/libdecor/plugins-1/libdecor-lumo.so
```

## Ativar

Plugin loader libdecor prioriza por `XDG_CURRENT_DESKTOP`. Lumo seta `XDG_CURRENT_DESKTOP=lumo` na sessao. Sem override extra.

Alternativa explicita (testing):
```bash
LIBDECOR_PLUGIN_DIR=/usr/lib/libdecor/plugins-1 firefox
```

## Status

| Versao | Funcionalidade |
|---|---|
| M1 (current) | Load OK, reserva 32px top border, sem render real |
| M2 | wl_shm pool + render bg titlebar + 3 botoes |
| M3 | Font rendering pra title text |
| M4 | Click handling + window state (max/min/close) |
| M5 | Light/dark theme via lumo-foundation sync |

## Arquitetura

- `Cargo.toml`: cdylib + build deps (cc, pkg-config)
- `build.rs`: invoca cc::Build pra compilar c-src/
- `src/lib.rs`: helpers puros + constants sync com C (testes unit)
- `c-src/lumo-decor.c`: implementacao plugin libdecor (interface + symbol export)
- `c-src/draw.h` + `draw.c`: pixel rendering ARGB8888

## Conflicting symbols

Plugin nao define conflicts (vazio array). libdecor loader pode carregar junto com fallback.

## ABI

API version 1 (libdecor 0.2.x). Quando libdecor bumpa pra 2, atualizar `api_version` + retest.
