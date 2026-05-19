//! config.rs — carrega ~/.config/lumo/dock.toml.

use serde::Deserialize;
use std::fs;

#[derive(Debug, Clone, Deserialize)]
pub struct SlotConfig {
    pub label: String,
    pub exec: String,
    /// Nome do processo pra detectar running (basename).
    #[serde(default)]
    pub process: String,
    /// Ícone: nome (para lookup interno) ou "text:<letra>".
    #[serde(default)]
    pub icon: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DockConfig {
    #[serde(default = "default_slots")]
    pub slots: Vec<SlotConfig>,
    #[serde(default)]
    pub autohide: bool,
}

fn default_slots() -> Vec<SlotConfig> {
    vec![
        SlotConfig { label: "Home".into(),          exec: "lumo-files".into(),   process: "lumo-files".into(),   icon: "home".into() },
        SlotConfig { label: "Calculator".into(),     exec: "galculator".into(),   process: "galculator".into(),   icon: "calc".into() },
        SlotConfig { label: "Settings".into(),       exec: "lumo-settings".into(),process: "lumo-settings".into(),icon: "settings".into() },
        SlotConfig { label: "Browser".into(),        exec: "firefox".into(),      process: "firefox".into(),      icon: "browser".into() },
        SlotConfig { label: "Terminal".into(),       exec: "lumo-term".into(),    process: "alacritty".into(),    icon: "term".into() },
        SlotConfig { label: "Calendar".into(),       exec: "lumo-calendar".into(),process: "lumo-calendar".into(),icon: "calendar".into() },
    ]
}

impl Default for DockConfig {
    fn default() -> Self {
        Self { slots: default_slots(), autohide: false }
    }
}

impl DockConfig {
    pub fn load() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
        let path = format!("{home}/.config/lumo/dock.toml");
        match fs::read_to_string(&path) {
            Ok(s) => toml::from_str(&s).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }
}
