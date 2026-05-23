//! lumo-style — Design System CSS engine para Lumo OS.
//!
//! Parser CSS subset com:
//! - `:root { --vars }` custom properties
//! - Selectors classe simples `.bar`, `.pill`, `.pill.lumo` (multi-class).
//! - Properties: `width`, `height`, `padding`, `padding-left/right/top/bottom`,
//!   `margin`, `background` (color), `color`, `border-radius`, `font-size`,
//!   `gap`.
//! - Cascade: regras posteriores sobrescrevem; specificity baseada em
//!   numero de classes no selector.
//!
//! Uso:
//! ```ignore
//! let style = lumo_style::load_from_disk()?;
//! let pill_h = style.get_px(".pill", "height").unwrap_or(28.0);
//! ```

use std::collections::HashMap;
use std::path::PathBuf;

pub mod parser;
pub mod model;
pub mod watcher;

pub use model::{Stylesheet, Selector, PropertyValue};

#[derive(Debug, thiserror::Error)]
pub enum StyleError {
    #[error("io: {0}")] Io(#[from] std::io::Error),
    #[error("parse: {0}")] Parse(String),
}

/// Default path: $XDG_CONFIG_HOME/lumo/lumo.css OR $HOME/.config/lumo/lumo.css.
pub fn default_path() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join("lumo").join("lumo.css"))
}

/// Le do disco + parse. Retorna Stylesheet vazio se file ausente.
pub fn load_from_disk() -> Result<Stylesheet, StyleError> {
    let path = match default_path() {
        Some(p) => p,
        None => return Ok(Stylesheet::default()),
    };
    if !path.exists() {
        return Ok(Stylesheet::default());
    }
    let src = std::fs::read_to_string(&path)?;
    parser::parse(&src).map_err(|e| StyleError::Parse(e))
}

/// Le de string (testes + reload manual).
pub fn parse_str(src: &str) -> Result<Stylesheet, StyleError> {
    parser::parse(src).map_err(|e| StyleError::Parse(e))
}
