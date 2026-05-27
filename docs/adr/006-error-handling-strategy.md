# ADR-006 — Error handling strategy

- **Status**: accepted
- **Data**: 2026-05-27
- **Refs**: `docs/error-audit.md`, `docs/error-codes.md`, `crates/foundation/lumo-error/`

## Context

Estado pre-ADR:
- 331 panic-sites em prod (lumo-ipc 19, lumo-files/ops 19, apps appmenu pattern 8-10 cada).
- 227 silent ignores (`let _ =`, `.ok();`).
- Estrategia atual = "crash → respawn via systemd/lumo-tty.sh".
- Crash do `lumo-wm` mata sessao. Crash de client (bar/desktop/app) gera respawn silencioso, sem feedback ao user.
- Erros recuperaveis frequentemente engolidos.

Modelo de referencia: macOS (WindowServer + ReportCrash + Console + beachball). Compositor protegido paranoicamente, apps isoladas, crash visivel ao user com codigo.

## Decision

Quatro camadas:

### 1. Taxonomia compartilhada

Novo crate `lumo-error` provides:
- `LumoError { domain, severity, code, msg, cause, recovery }`.
- Enums `Domain`, `Severity { Fatal, Degraded, Recoverable, UserError }`, `RecoveryHint`.
- Macro `lumo_err!(domain, sev, "CODE-001", "fmt {}", arg)`.
- `CrashReport` serializavel em JSON.
- `install_panic_hook(binary, domain)` que escreve dump em `~/.local/state/lumo/crashes/`.

### 2. Politica por camada

| Camada | Crash policy |
|---|---|
| lumo-wm core | Preservar sessao. Wrap render + handlers em catch_unwind. Erros virem `LumoError`, propagados pro state, nunca panic |
| Backend DRM | Retry counter 3x; depois `Degraded` (vsync off); device-lost → tentar reabrir |
| IPC | Client dropou → unsub. Broadcast continua. Frame corrupt → skip linha |
| Shell clients | Respawn + crash counter persisted. 3 restarts/min → modal "lumo-bar instavel" |
| Apps Iced | Crash → toast + crash dump + reopen offer |
| Bridge | Erro → HTTP status + JSON `{code, msg, recovery}` |

### 3. UX

- **Crash banner**: layer-shell overlay `lumo-toast` mostra "lumo-X reiniciado · CODE" 4s.
- **Degraded indicator**: pill na bar quando feature off ("Vsync off · WM-RENDER-002").
- **Critical modal**: tela cheia quando fatal compositor irrecuperavel, 5s pra save state.
- **App freeze**: ping/pong 500ms, 2s sem pong = beachball cursor + "Nao responde" no titulo.

### 4. Diagnostico

- `~/.local/state/lumo/crashes/` JSON dump por crash.
- `lumoctl diag` coleta ultimos erros + restarts + GPU + IPC, gera tarball.
- `lumoctl crash <id>` mostra dump symbolicado.
- `lumoctl logs --subsystem wm --since 1h` viewer estilo `Console.app`.
- Telemetria `errors_total{domain, code, severity}`.

## Consequences

**Positivas**:
- Erros consistentemente classificados + codificados.
- User sabe quando algo crashou (toast em vez de tela preta silenciosa).
- Diagnostico self-service via `lumoctl diag`.
- Migracao gradual: sites podem migrar um por um sem big-bang.

**Negativas**:
- Toda chamada de erro aloca `String` (Cow nao ajuda quando msg dinamica). Aceitavel — erros nao sao hot-path.
- Novo crate aumenta dependency graph (todos crates que reportam erro importam lumo-error).
- Compromisso append-only de codigos exige disciplina em PR review.

## Migracao

Sprints definidos em `docs/error-audit.md`. Crate disponivel a partir deste commit. Refactor por sprint, nao big-bang.
