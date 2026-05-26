//! history.rs - persiste historico de notificacoes (rotating 100 entries).

use serde::{Deserialize, Serialize};
use std::fs;

const MAX_HISTORY: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub id: u32,
    pub app_name: String,
    pub summary: String,
    pub body: String,
    pub timestamp: u64,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct History {
    pub entries: Vec<HistoryEntry>,
}

impl History {
    fn path() -> String {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        format!("{home}/.cache/lumo-notif/history.json")
    }
    pub fn load() -> Self {
        match fs::read_to_string(Self::path()) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }
    pub fn push(&mut self, entry: HistoryEntry) {
        self.entries.insert(0, entry);
        self.entries.truncate(MAX_HISTORY);
        self.save();
    }
    fn save(&self) {
        let p = Self::path();
        if let Some(parent) = std::path::Path::new(&p).parent() {
            fs::create_dir_all(parent).ok();
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            fs::write(&p, s).ok();
        }
    }
}
