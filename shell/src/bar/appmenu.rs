//! bar/appmenu.rs - Integracao DBus appmenu via com.canonical.dbusmenu.
//!
//! C5 WIP: internals pendentes de correcao da API zbus 5 (feature blocking-api).
//! Interface publica estavel, stubs retornam default silencioso.

/// Item de menu top-level exportado pelo app.
#[derive(Debug, Clone)]
pub struct AppMenuItem {
    pub id: i32,
    pub label: String,
}

/// Cache do menu atual.
#[derive(Debug, Default, Clone)]
pub struct AppMenuState {
    pub service: String,
    pub object_path: String,
    pub items: Vec<AppMenuItem>,
    pub app_id: String,
}

impl AppMenuState {
    pub fn fetch(_pid: u32, _app_id: &str) -> Self {
        Self::default()
    }
    pub fn activate(&self, _item_id: i32) {}
    pub fn fetch_submenu(&self, _parent_id: i32) -> Vec<AppMenuItem> {
        Vec::new()
    }
}
