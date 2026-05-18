//! bar/appmenu.rs - Integracao DBus appmenu via com.canonical.dbusmenu.
//!
//! Fluxo:
//!   1. Bar recebe ActiveApp { app_id, title, pid } via IPC do compositor.
//!   2. fetch(pid) -> conecta session bus, chama AppMenu.Registrar
//!      GetMenuForWindow(pid) -> (service, object_path).
//!   3. Chama dbusmenu GetLayout(0, 1, []) -> arvore top-level.
//!   4. Retorna Vec<AppMenuItem> com labels e ids.
//!   5. Click num item top-level: fetch_submenu(parent_id).
//!   6. Click em subitem: activate(item_id) -> DBus Event.
//!
//! Fallback: qualquer erro = retorna Vec vazia (bar nao mostra nada). Silencio.
//! Apps GTK4 modernas, Electron, GNOME nao exportam appmenu (GTK4 removeu).
//! Apps com suporte: GTK3 + appmenu-gtk-module, Qt5 + appmenu-qt5.

/// Item de menu top-level ou subitem exportado pelo app via dbusmenu.
#[derive(Debug, Clone)]
pub struct AppMenuItem {
    /// ID DBus do item (pra Event/AboutToShow).
    pub id: i32,
    /// Label visivel (ex: "File", "Edit", "View"). "---" = separador.
    pub label: String,
}

/// Cache do menu atual (None = nenhum app com appmenu ativo).
#[derive(Debug, Default, Clone)]
pub struct AppMenuState {
    /// Service DBus do app com foco (ex: ":1.42").
    pub service: String,
    /// Object path do menu (ex: "/com/app/menus/mainwindow").
    pub object_path: String,
    /// Items top-level ja fetchados.
    pub items: Vec<AppMenuItem>,
    /// app_id do app que originou este cache (pra invalidar na troca).
    pub app_id: String,
}

impl AppMenuState {
    /// Busca menubar via DBus AppMenu.Registrar + dbusmenu.
    /// Em caso de falha retorna estado vazio (silencio).
    pub fn fetch(pid: u32, app_id: &str) -> Self {
        if pid == 0 {
            return Self::default();
        }
        match fetch_inner(pid, app_id) {
            Ok(state) => state,
            Err(e) => {
                eprintln!("[appmenu] C5: fetch falhou pid={} app_id={}: {}", pid, app_id, e);
                Self::default()
            }
        }
    }

    /// Envia DBus Event "clicked" pro item com o dado id.
    pub fn activate(&self, item_id: i32) {
        if self.service.is_empty() || self.object_path.is_empty() {
            return;
        }
        if let Err(e) = activate_inner(&self.service, &self.object_path, item_id) {
            eprintln!("[appmenu] C5: activate item_id={} falhou: {}", item_id, e);
        }
    }

    /// Busca subitens (submenu) de um item top-level via DBus.
    pub fn fetch_submenu(&self, parent_id: i32) -> Vec<AppMenuItem> {
        if self.service.is_empty() {
            return Vec::new();
        }
        match fetch_submenu_inner(&self.service, &self.object_path, parent_id) {
            Ok(items) => items,
            Err(e) => {
                eprintln!("[appmenu] C5: fetch_submenu parent={} falhou: {}", parent_id, e);
                Vec::new()
            }
        }
    }
}

// ============================================================
// Internals: zbus blocking calls.
// ============================================================

fn fetch_inner(pid: u32, app_id: &str) -> Result<AppMenuState, Box<dyn std::error::Error>> {
    use zbus::blocking::Connection;
    use zbus::blocking::Proxy;

    let conn = Connection::session()?;

    // GetMenuForWindow(window_id). appmenu-gtk-module registra via PID como window_id.
    let registrar = Proxy::new(
        &conn,
        "com.canonical.AppMenu.Registrar",
        "/com/canonical/AppMenu/Registrar",
        "com.canonical.AppMenu.Registrar",
    )?;

    let result: zbus::Result<(String, zbus::zvariant::OwnedObjectPath)> =
        registrar.call("GetMenuForWindow", &(pid,));

    let (service, obj_path) = match result {
        Ok((s, o)) => (s, o.to_string()),
        Err(e) => {
            // App nao registrou appmenu. Silencio normal pra GTK4/electron.
            eprintln!("[appmenu] C5: GetMenuForWindow(pid={}) sem registro: {}", pid, e);
            return Ok(AppMenuState::default());
        }
    };

    if service.is_empty() {
        return Ok(AppMenuState::default());
    }

    eprintln!("[appmenu] C5: pid={} -> service={} path={}", pid, service, obj_path);

    let items = {
        let menu_proxy = Proxy::new(
            &conn,
            service.as_str(),
            obj_path.as_str(),
            "com.canonical.dbusmenu",
        )?;
        parse_layout_at(&menu_proxy, 0)?
    };
    eprintln!("[appmenu] C5: {} items top-level para app_id={}", items.len(), app_id);

    Ok(AppMenuState {
        service,
        object_path: obj_path,
        items,
        app_id: app_id.to_string(),
    })
}

fn parse_layout_at(
    proxy: &zbus::blocking::Proxy,
    parent_id: i32,
) -> Result<Vec<AppMenuItem>, Box<dyn std::error::Error>> {
    use zbus::zvariant::Value;

    // GetLayout(parent_id, recursion_depth=1, property_names=[])
    // Retorna (revision: u32, layout: (i32, a{sv}, av))
    let reply = proxy.call_method("GetLayout", &(parent_id, 1i32, Vec::<&str>::new()))?;
    let body = reply.body();

    let (_rev, layout): (u32, (i32, std::collections::HashMap<String, Value>, Vec<Value>)) =
        body.deserialize()?;

    let (_root_id, _root_props, children) = layout;
    let mut items = Vec::new();
    for child in children {
        // Cada child e uma struct (i32, a{sv}, av).
        if let Value::Structure(s) = child {
            let fields = s.into_fields();
            if fields.len() < 2 {
                continue;
            }
            let id = match &fields[0] {
                Value::I32(v) => *v,
                _ => continue,
            };
            let (label, is_sep) = if let Value::Dict(dict) = &fields[1] {
                let dict_clone = dict.try_clone().ok();
                let (label, kind) = if let Some(dc) = dict_clone {
                    let map: std::collections::HashMap<String, Value> =
                        dc.try_into().unwrap_or_default();
                    let lbl = map.get("label")
                        .and_then(|v| {
                            if let Value::Str(s) = v { Some(s.as_str().to_string()) } else { None }
                        })
                        .unwrap_or_default();
                    let knd = map.get("type")
                        .and_then(|v| {
                            if let Value::Str(s) = v { Some(s.as_str().to_string()) } else { None }
                        })
                        .unwrap_or_default();
                    (lbl, knd)
                } else {
                    (String::new(), String::new())
                };
                (label, kind == "separator")
            } else {
                (String::new(), false)
            };
            if is_sep {
                items.push(AppMenuItem { id, label: "---".to_string() });
                continue;
            }
            // Remove mnemonics GTK (_File -> File, Fi_le -> File).
            let label = label.replace('_', "");
            if label.is_empty() {
                continue;
            }
            items.push(AppMenuItem { id, label });
        }
    }
    Ok(items)
}

fn activate_inner(
    service: &str,
    object_path: &str,
    item_id: i32,
) -> Result<(), Box<dyn std::error::Error>> {
    use zbus::blocking::Connection;
    use zbus::blocking::Proxy;

    let conn = Connection::session()?;
    let proxy = Proxy::new(&conn, service, object_path, "com.canonical.dbusmenu")?;
    // Event(id, eventId, data, timestamp)
    proxy.call_noreply(
        "Event",
        &(item_id, "clicked", zbus::zvariant::Value::U32(0), 0u32),
    )?;
    Ok(())
}

fn fetch_submenu_inner(
    service: &str,
    object_path: &str,
    parent_id: i32,
) -> Result<Vec<AppMenuItem>, Box<dyn std::error::Error>> {
    use zbus::blocking::Connection;
    use zbus::blocking::Proxy;

    let conn = Connection::session()?;
    let proxy = Proxy::new(&conn, service, object_path, "com.canonical.dbusmenu")?;

    // AboutToShow pra garantir que o submenu esta populado antes de GetLayout.
    let _: zbus::Result<(bool,)> = proxy.call("AboutToShow", &(parent_id,));

    parse_layout_at(&proxy, parent_id)
}
