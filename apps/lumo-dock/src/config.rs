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
    // W38: so apps que EXISTEM de fato (binarios Lumo em target/release ou no
    // PATH do sistema). Antes referenciava galculator/firefox/lumo-calendar que
    // nao existem no Galaxy -> click sem efeito. `process` = nome real em
    // /proc/<pid>/comm pra acender o dot de "app aberto".
    vec![
        SlotConfig {
            label: "Files".into(),
            exec: "lumo-files".into(),
            process: "lumo-files".into(),
            icon: "home".into(),
        },
        SlotConfig {
            label: "Calculator".into(),
            exec: "lumo-calc".into(),
            process: "lumo-calc".into(),
            icon: "calc".into(),
        },
        SlotConfig {
            label: "Settings".into(),
            exec: "lumo-settings".into(),
            process: "lumo-settings".into(),
            icon: "settings".into(),
        },
        SlotConfig {
            label: "Browser".into(),
            exec: "chromium".into(),
            process: "chromium".into(),
            icon: "browser".into(),
        },
        SlotConfig {
            label: "Terminal".into(),
            exec: "lumo-term".into(),
            process: "alacritty".into(),
            icon: "term".into(),
        },
        SlotConfig {
            label: "Notes".into(),
            exec: "lumo-notes".into(),
            process: "lumo-notes".into(),
            icon: "calendar".into(),
        },
    ]
}

impl Default for DockConfig {
    fn default() -> Self {
        Self {
            slots: default_slots(),
            autohide: false,
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_has_six_slots() {
        let c = DockConfig::default();
        assert_eq!(c.slots.len(), 6);
    }

    #[test]
    fn default_autohide_off() {
        let c = DockConfig::default();
        assert!(!c.autohide);
    }

    #[test]
    fn default_slots_have_known_apps() {
        let c = DockConfig::default();
        let labels: Vec<&str> = c.slots.iter().map(|s| s.label.as_str()).collect();
        assert!(labels.contains(&"Files"));
        assert!(labels.contains(&"Calculator"));
        assert!(labels.contains(&"Settings"));
        assert!(labels.contains(&"Browser"));
        assert!(labels.contains(&"Terminal"));
        assert!(labels.contains(&"Notes"));
    }

    #[test]
    fn default_slots_have_non_empty_exec() {
        let c = DockConfig::default();
        for slot in &c.slots {
            assert!(!slot.exec.is_empty(), "slot {} sem exec", slot.label);
        }
    }

    #[test]
    fn parse_toml_with_autohide() {
        let toml_src = r#"
            autohide = true
            [[slots]]
            label = "MyApp"
            exec = "myapp"
            process = "myapp"
            icon = "app"
        "#;
        let c: DockConfig = toml::from_str(toml_src).expect("parse");
        assert!(c.autohide);
        assert_eq!(c.slots.len(), 1);
        assert_eq!(c.slots[0].label, "MyApp");
    }

    #[test]
    fn parse_toml_empty_uses_default_slots() {
        let c: DockConfig = toml::from_str("").expect("parse empty");
        assert_eq!(c.slots.len(), 6);
        assert!(!c.autohide);
    }

    #[test]
    fn slot_config_defaults_process_and_icon_to_empty() {
        let toml_src = r#"
            [[slots]]
            label = "X"
            exec = "x-cmd"
        "#;
        let c: DockConfig = toml::from_str(toml_src).expect("parse");
        assert_eq!(c.slots[0].process, "");
        assert_eq!(c.slots[0].icon, "");
    }
}
