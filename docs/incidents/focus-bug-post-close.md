# Bug UX foco apos fechar toplevel (precisa L1)

## Reproducao
1. Click icone Mousepad no desktop -> selected (highlight)
2. Enter -> abre mousepad (A40 broadcast OK)
3. Fecha mousepad (X janela)
4. Selection icone permanece (correto, UX Mac-like)
5. **Enter -> nao abre (BUG)**
6. Re-click icone -> Enter funciona

## Root cause hipotese

Apos toplevel destruido (close), `keyboard.current_focus()` ainda retorna `Some(<surface_zumbi>)`. WlSurface foi destroyed mas keyboard internal state nao foi clearado.

A40 check em handlers/input.rs:
```rust
if press && last_sym_for_a40.get() == Keysym::Return {
    let has_focus = self.keyboard.current_focus().is_some();
    if !has_focus {
        self.broadcast_desktop_open_selected();
    }
}
```

`has_focus = true` falsamente -> broadcast NAO dispara -> Enter no desktop morre.

## Fix proposto (L1 FocusManager)

Em `crates/compositor/lumo-wm/src/handlers/xdg_shell.rs` ou onde toplevel_destroyed handler vive: chamar `FocusManager::close_toplevel(...)` quando toplevel destruido. Esse metodo:
1. Verifica se current_focus eh esse toplevel
2. Se sim, set_focus(None) OR next surviving toplevel
3. Limpa estado interno

Pseudo:
```rust
fn toplevel_destroyed(surface: &WlSurface) {
    if self.focus_manager.current() == Some(surface) {
        let next = self.space.elements_for_output(...).last();
        match next {
            Some(w) => self.focus_manager.set_toplevel(w.toplevel().surface(), ...),
            None => self.focus_manager.clear(...),  // set_focus(None)
        }
    }
}
```

## Tests obrigatorios

- test_focus_cleared_after_destroy
- test_focus_moves_to_next_toplevel_when_close_focused
- test_enter_after_close_triggers_broadcast
