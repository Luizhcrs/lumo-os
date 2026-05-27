# ADR-001 — Multibinary como app spawn canonical

- **Status**: accepted
- **Data**: 2026-05-27
- **Supersedes**: nenhum (consolida 3 modelos coexistentes)

## Context

Lumo tem 3 modelos coexistentes pra spawn de apps Iced:

1. **Standalone**: cada app binario separado (`lumo-calc`, `lumo-notes`...). RAM ~25MB/app, isolacao crash, sem IPC handshake.
2. **Multibinary** (`lumo-apps`): 1 binario com symlinks por nome (`argv[0]` dispatcha). RAM ~30MB total, processo por app, sem daemon.
3. **Daemon** (`lumo-appsd` + `lumo-appctl`): 1 processo single, multi-window Iced via IPC. RAM ~150MB total, mas crash mata todos os apps, multi-window Iced 0.13 e complexo.

Manter os 3 = ambiguidade. Launcher precisa saber qual chamar. Install script duplica. Onboarding confuso.

## Decision

**Multibinary (`lumo-apps`) e o modelo canonical.**

- Build produz binario `lumo-apps`.
- Install cria symlinks `/usr/local/bin/lumo-{calc,notes,about,settings,...}` apontando pra `lumo-apps`.
- Dispatch via `std::env::args().next()` basename match.

**`lumo-appsd` + `lumo-appctl` marcados deprecated**, movidos pra `archive/` em release seguinte.

**Standalone permanece pra dev iteration** (cargo run -p lumo-X), mas nao e o path de install.

## Consequences

**Positivas**:
- Menor RAM idle (1 binario shared).
- Crash isolation preservada (processo por app via fork+exec).
- Sem IPC handshake na startup = launcher abre mais rapido.
- Install script trivial (symlinks).
- Sem complexidade de multi-window Iced.

**Negativas**:
- Binario unico cresce (todos apps compilados juntos).
- Cold start primeiro spawn carrega libs uma vez por processo (sem ganho de daemon hot path).

**Migracao**:
- `apps/lumo-appsd/` → `archive/lumo-appsd/`
- `apps/lumo-appctl/` → `archive/lumo-appctl/`
- Update `scripts/install.sh` pra criar symlinks.
- Update `apps/lumo-launcher` pra spawn `lumo-X` direto.
