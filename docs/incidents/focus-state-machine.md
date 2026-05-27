# Focus State Machine — lumo-wm

## Overview

Centralizado em `crates/compositor/lumo-wm/src/focus.rs`.

Antes: `kb.set_focus` espalhado em 3 handlers. Depois: toda policy passa por `FocusManager`.

## Estados

| Estado | Descricao |
|--------|-----------|
| `None` | Nenhuma surface focada (area vazia ou layer-shell) |
| `Toplevel(WlSurface)` | Toplevel xdg com foco de teclado |
| `Dropdown` | Dropdown ativo na bar. Teclado nao redirecionado; `prev` guarda toplevel anterior |

## Diagrama de Transicoes

```
                     click_layer_shell
                    <------------------
None                                    Toplevel(S)
  |  click_toplevel                       |
  +---------------------------------------->
  |
  | new_toplevel (nova janela aberta)
  +---------------------------------------->  Toplevel(S)
                                              |
                                 cycle_next  |---> Toplevel(S') [proximo na lista]
                                 cycle_prev  |---> Toplevel(S') [anterior na lista]
                                             |
                              close_toplevel |---> Toplevel(next) [se outros abertos]
                              close_toplevel |---> None [se era ultimo]
                                             |
                         click_layer_shell   |---> None
                                             |
                          click_dropdown     |---> Dropdown (preserva prev=S)

Dropdown ----close_dropdown----> prev (Toplevel(S) | None)
```

## Matriz Eventos x Estados (15+ entradas)

| # | Evento | De | Para | Acao |
|---|--------|-----|------|------|
| 1 | click_toplevel | None | Toplevel(S) | kb.set_focus(S) |
| 2 | new_toplevel | None | Toplevel(S) | kb.set_focus(S) |
| 3 | click_toplevel | Toplevel(S) | Toplevel(S') | kb.set_focus(S') |
| 4 | click_layer_shell | Toplevel(S) | None | kb.set_focus(None) |
| 5 | close_toplevel (next=Some) | Toplevel(S) | Toplevel(next) | kb.set_focus(next) |
| 6 | close_toplevel (next=None) | Toplevel(S) | None | kb.set_focus(None) |
| 7 | cycle_next | Toplevel(S) | Toplevel(S') | kb.set_focus(S') |
| 8 | cycle_prev | Toplevel(S) | Toplevel(S') | kb.set_focus(S') |
| 9 | cycle_next | None | Toplevel(first) | kb.set_focus(first) |
| 10 | cycle_prev | None | Toplevel(first) | kb.set_focus(first) |
| 11 | click_toplevel | Dropdown | Toplevel(S) | kb.set_focus(S) |
| 12 | click_layer_shell | Dropdown | None | kb.set_focus(None) |
| 13 | cycle (space vazio) | Toplevel(S) | Toplevel(S) | no-op |
| 14 | click_layer_shell | None | None | no-op |
| 15 | close_toplevel (next=None) | None | None | no-op |

## Bindings SUPER+Tab

- `SUPER+Tab` -> `KeyAction::CycleWindow(1)` -> `FocusManager::cycle_next`
- `SUPER+Shift+Tab` -> `KeyAction::CycleWindow(-1)` -> `FocusManager::cycle_prev`

Definido em `input/keyboard.rs::default_bindings()`.

## Arquivo Focus

`crates/compositor/lumo-wm/src/focus.rs`

- `FocusState` enum: `None | Toplevel(WlSurface) | Dropdown`
- `FocusManager` struct: `state: FocusState`, `prev: Option<WlSurface>`
- Metodos: `click_toplevel`, `click_layer_shell`, `close_toplevel`, `new_toplevel`, `cycle`, `cycle_next`, `cycle_prev`
- 15 testes unitarios cobrindo transicoes (T01..T15)
