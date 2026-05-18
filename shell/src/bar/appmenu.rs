//! bar/appmenu.rs - STUB (C5 menubar pendente nova sessao com API zbus 5 correta).
//!
//! Interface publica estavel: AppMenuItem + AppMenuState retornam vazio.

#[derive(Debug, Clone, Default)]
pub struct AppMenuItem {
    pub id: i32,
    pub label: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default)]
pub struct AppMenuState {
    pub items: Vec<AppMenuItem>,
    pub service: String,
    pub object_path: String,
}

impl AppMenuState {
    pub fn fetch(_pid: u32) -> Self {
        Self::default()
    }

    pub fn fetch_submenu(&self, _id: i32) -> Vec<AppMenuItem> {
        Vec::new()
    }

    pub fn activate(&self, _id: i32) {}
}
