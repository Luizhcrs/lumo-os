# Error Audit — Lumo OS

Snapshot 2026-05-27.

## Numeros agregados

- **331 panic-sites** em codigo prod (excl. tests, vendor).
- **227 silent-ignore** (`let _ =`, `.ok();`, `let _: =`).
- 22 panic-sites no `lumo-wm` (compositor) — qualquer um deles mata sessao.

## Hotspots prod (top 15)

| Sites | Path | Risco |
|------:|------|-------|
| 19 | `crates/compositor/lumo-ipc/src/lib.rs` | IPC framing — alto |
| 19 | `apps/lumo-files/src/ops.rs` | FS ops — medio |
| 10 | `apps/lumo-notes/src/appmenu.rs` | App pattern — baixo |
| 10 | `apps/lumo-files/src/appmenu.rs` | App pattern — baixo |
| 10 | `apps/lumo-editor/src/appmenu.rs` | App pattern — baixo |
|  9 | `apps/lumo-lock/src/main.rs` | Critical (lock screen) |
|  8 | `shell/src/osd.rs` | Shell — medio |
|  8 | `shell/src/bar/main_loop.rs` | Bar event loop — alto |
|  8 | `apps/lumo-bridge/src/main.rs` | HTTP daemon — medio |
|  7 | `shell/src/desktop/main_loop.rs` | Desktop loop — alto |
|  7 | `apps/lumo-launcher/src/main.rs` | App — medio |
|  7 | `apps/lumo-dock/src/main.rs` | Dock — medio |
|  3 | `crates/compositor/lumo-wm/src/perf.rs` | Telemetry — baixo |
|  3 | `crates/compositor/lumo-wm/src/handlers/screencopy.rs` | Screencopy — medio |
|  3 | `crates/compositor/lumo-wm/src/backend/wallpaper.rs` | Wallpaper — baixo (slice bounds asserted) |

## Categorias de panic encontradas

1. **Mutex.lock().unwrap()** — Rust convention. Raro envenenar. Aceitavel.
2. **Config parse .unwrap()** em paths user (`vrr.rs`, `idle.rs`) — **MAU**: config user pode estar corrompida, deve fallback gracioso pra default.
3. **Bounds check + unwrap** (`bounding_box`, `wallpaper.rs:97`) — assertion de invariante. Aceitavel se comentado.
4. **Init-time setup** (`state.rs:319` keyboard add) — fatal cedo, mensagem clara. Aceitavel.
5. **Hot path em handlers** (`seat.rs:125`, `lid.rs:35`, `screencopy.rs:182`) — **MAU**: mata sessao quando dado divergente chega.
6. **App pattern repetido em `appmenu.rs`** dos apps Iced — provavel `app_id.unwrap()` ou path lookup. Padroniza pra usar lumo-error.

## Politica recomendada por categoria

| Categoria | Acao |
|---|---|
| Mutex.lock().unwrap() | Manter (convenção). Comentar `// poison: impossivel em codigo nosso` |
| Config user .unwrap() | **Refatorar** pra `unwrap_or_default()` + log warning |
| Bounds assert | Manter + comentar invariante (`// SAFETY: ...`) |
| Init-time | Aceitar fatal mas usar `LumoError::Fatal` + crash dump |
| Hot path handler | **Refatorar** pra Result, propagar pro state, ignorar event |
| Apps appmenu | **Refatorar** pra helper compartilhado em lumo-error |

## Silent ignores

227 sites `let _ =`. Auditoria 2026-05-27:

### Categorias

| Categoria | Politica | Sites |
|---|---|---|
| Hardware feature unsupported (libinput config_*_set) | Manter mas log debug | ~30 |
| spawn fire-and-forget (Command::spawn) | Manter — proc desacoplado | ~40 |
| IPC tx.send (receiver pode dropar) | Manter — broadcast best-effort | ~50 |
| socket write/flush em handler | Manter — proximo tick re-envia | ~30 |
| Init secondary (set_app_id, etc.) | Manter — falha estetica | ~30 |
| **Erro suspeito** (config write, sensor read) | **Refatorar pra log warn** | ~50 |

### Refatoracoes nesta sessao

- `crates/compositor/lumo-wm/src/input/touchpad.rs`: config_accel_set_profile/speed/tap_set_enabled — `let _ =` → `if let Err(e) = ... { tracing::debug!(?e, ...) }`. Restantes tap_set_button_map/drag mantidos pois sub-features (gating ja em `if self.tap_enabled`).

### Sprint futuro

Restantes ~50 sites suspeitos. Trabalho individual por arquivo. Nao escala bulk-sed.

## Plano de migracao gradual

1. **lumo-error crate criado** (este commit). Crates podem importar.
2. **Sprint 1**: lumo-wm hot-path (seat, lid, screencopy, vrr, idle). ~10 sites.
3. **Sprint 2**: lumo-ipc framing. ~19 sites.
4. **Sprint 3**: shell bar/desktop main_loop. ~15 sites.
5. **Sprint 4**: apps `appmenu.rs` pattern + lumo-lock.
6. **Sprint 5**: silent-ignores audit dirigido.

Cada sprint: identificar sites → adicionar codigo erro em `docs/error-codes.md` → refactor → adicionar testes regressao.

## Comandos de verificacao

```bash
# Contagem total
grep -rn "\.unwrap()\|\.expect(\|panic!" --include="*.rs" \
  | grep -v "test\|/tests/\|vendor/" | wc -l

# Hotspots por arquivo
grep -rn "\.unwrap()\|\.expect(\|panic!" --include="*.rs" \
  | grep -v "test\|/tests/\|vendor/" \
  | awk -F: '{print $1}' | sort | uniq -c | sort -rn

# Silent ignores
grep -rn "let _ =\|\.ok();$" --include="*.rs" \
  | grep -v "test\|/tests/\|vendor/" | wc -l
```
