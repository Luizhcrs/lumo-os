//! recent.rs - persiste e carrega apps recentes.

use serde::{Deserialize, Serialize};
use std::fs;

const MAX_RECENT: usize = 8;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RecentApps {
    pub entries: Vec<String>,
}

impl RecentApps {
    pub fn path() -> String {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        format!("{home}/.config/lumo/launcher-recent.toml")
    }
    pub fn load() -> Self {
        match fs::read_to_string(Self::path()) {
            Ok(s) => toml::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }
    pub fn push(&mut self, name: &str) {
        self.entries.retain(|e| e != name);
        self.entries.insert(0, name.to_string());
        self.entries.truncate(MAX_RECENT);
        self.save();
    }
    fn save(&self) {
        let p = Self::path();
        if let Some(parent) = std::path::Path::new(&p).parent() {
            fs::create_dir_all(parent).ok();
        }
        if let Ok(s) = toml::to_string(self) {
            fs::write(&p, s).ok();
        }
    }
}
