# ADR-005 — Shell em 3 processos (bar / desktop / dock)

- **Status**: accepted
- **Data**: 2026-05-XX

## Context

Alternativas avaliadas:

1. **Monolito**: 1 processo `lumo-shell` desenha bar + desktop + dock.
2. **3 processos**: separa bar, desktop, dock como clients Wayland independentes.
3. **Compositor interno**: render no proprio lumo-wm.

Monolito: crash mata toda UI. Compositor interno: acopla render com input/output management, dificulta restart shell sem dropar sessao.

## Decision

**3 processos**, cada um cliente Wayland do lumo-wm:

- `lumo-shell` (bar) — wlr-layer-shell `Top`, exclusive zone 28px.
- `lumo-shell` (desktop, mesmo binario, modo `--desktop`) — wlr-layer-shell `Background`.
- `lumo-dock` — wlr-layer-shell `Top` (bottom anchor), autohide.

Compartilham `lumo-foundation` (tokens), `lumo-style` (CSS), `lumo-ipc` (events).

IPC: cada client conecta socket Unix `$XDG_RUNTIME_DIR/lumo.sock`, le LumoEvent broadcast.

## Consequences

**Positivas**:
- Crash bar nao mata desktop nem dock.
- Hot-restart de cada componente sem dropar sessao.
- Permite swap de impl (ex: dock alternativo) sem rebuild de tudo.

**Negativas**:
- 3 processos = 3 conexoes Wayland + 3 sockets IPC.
- IPC broadcast: cada evento serializa 3x.
- Coordenacao visual (ex: hover bar revela dropdown que sobrepoe desktop) precisa Z-order via layer-shell layer + IPC.

**Aceito porque** Lumo prioriza isolation e hot-reload sobre RAM. Cada processo idle ~15MB.
