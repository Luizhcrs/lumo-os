# lumo-files menubar appmenu spec

## Requisito Luiz 2026-05-18

lumo-files exporta menu nativo via DBus appmenu protocol → pill aparece automatico ao lado de "Lumo" na bar Lumo.

## Implementacao

### Crate dep
```toml
zbus = { version = "5", default-features = false, features = ["blocking-api"] }
```

### Modulo `apps/lumo-files/src/appmenu.rs`

1. Boot: registra self via `com.canonical.AppMenu.Registrar`:
```rust
let conn = zbus::blocking::Connection::session()?;
let proxy = zbus::blocking::Proxy::new(
    &conn,
    "com.canonical.AppMenu.Registrar",
    "/com/canonical/AppMenu/Registrar",
    "com.canonical.AppMenu.Registrar",
)?;
let pid = std::process::id();
let menu_path = "/com/lumo/lumo_files/menus/main";
proxy.call::<_, _, ()>("RegisterWindow", &(pid, menu_path))?;
```

2. Implementa interface `com.canonical.dbusmenu` em `/com/lumo/lumo_files/menus/main`:
   - `GetLayout(parent_id, recursion_depth, property_names) -> (revision, layout)`
   - `Event(id, event_id, data, timestamp)` — receber clicks de pills
   - `AboutToShow(id) -> bool` — submenu prep

3. Menu structure:
```
File
  Nova janela        Ctrl+N
  Nova pasta         Ctrl+Shift+N
  ---
  Sair                  Ctrl+Q

Edit
  Recortar           Ctrl+X
  Copiar             Ctrl+C
  Colar              Ctrl+V
  ---
  Selecionar tudo    Ctrl+A

View
  Atualizar          F5
  ---
  Mostrar ocultos    Ctrl+H
  Grid               Ctrl+1
  Lista              Ctrl+2

Help
  Sobre lumo-files
  Atalhos teclado   Ctrl+?
```

4. Unregister em Drop / signal handler exit: `proxy.call("UnregisterWindow", &(pid,))`.

5. Forward menubar actions: quando Lumo bar dispara click via DBus Event, lumo-files recebe e executa acao correspondente em iced Message enum.

## Resultado

Pill aparece bar topo: `File ▾ Edit ▾ View ▾ Help ▾` ao lado de "Lumo". Click pill abre dropdown via dbusmenu fetch. Click item dispara acao em lumo-files real.

Compositor + bar ja tem Registrar daemon (C5.1) + fetch + render pills (C5+M3). Lumo-files so precisa exportar.

## Referencias

- Protocolo: https://github.com/AyatanaIndicators/libdbusmenu/blob/master/libdbusmenu-glib/dbus-menu.xml
- Implementacoes em Rust: vala-panel-appmenu, libdbusmenu rust bindings (raros)
- Alternative: implementar zbus interface direto via macros `#[interface]`
