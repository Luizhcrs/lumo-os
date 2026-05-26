//! locale.rs -- grava configuracao de idioma em ~/.config/lumo/locale.toml

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, Deserialize)]
pub struct LocaleConfig {
    pub locale: String,
}

impl LocaleConfig {
    pub fn new(code: &str) -> Self {
        LocaleConfig {
            locale: code.to_string(),
        }
    }

    /// Persiste em `~/.config/lumo/locale.toml`.
    pub fn write(&self) -> std::io::Result<()> {
        self.write_to(&default_config_path())
    }

    /// Persiste no path explicitamente informado.
    pub fn write_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content =
            toml::to_string(self).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        std::fs::write(path, content)
    }

    /// Le config em `~/.config/lumo/locale.toml`, se houver.
    pub fn read() -> Option<Self> {
        Self::read_from(&default_config_path())
    }

    /// Le config no path explicitamente informado.
    pub fn read_from(path: &Path) -> Option<Self> {
        let raw = std::fs::read_to_string(path).ok()?;
        toml::from_str(&raw).ok()
    }
}

fn default_config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
    PathBuf::from(home).join(".config/lumo/locale.toml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn write_and_read_roundtrip() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("locale.toml");
        let cfg = LocaleConfig::new("pt_BR");
        cfg.write_to(&path).unwrap();
        let loaded = LocaleConfig::read_from(&path).unwrap();
        assert_eq!(loaded.locale, "pt_BR");
    }

    #[test]
    fn read_missing_returns_none() {
        let path = PathBuf::from("/tmp/lumo-nonexistent-xyz/locale.toml");
        let result = LocaleConfig::read_from(&path);
        assert!(result.is_none());
    }
}
