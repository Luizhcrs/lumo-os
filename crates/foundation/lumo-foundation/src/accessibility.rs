//! W8.C: AccessibilityTokens -- reduced_motion, high_contrast, font_scale.
//!
//! Carregado de ~/.config/lumo/accessibility.toml.
//!
//! Formato:
//!   [accessibility]
//!   reduced_motion = false
//!   high_contrast = false
//!   font_scale = 1.0
//!
//! reduced_motion=true: animacoes puladas (duracao=0), fades skip.
//! high_contrast=true:  bg=#000000, fg=#FFFFFF, accent=#FFFF00, bordas 2px.
//! font_scale: multiplica todos os tamanhos de fonte (range 0.8..=1.4).

use std::path::PathBuf;

/// Tokens de acessibilidade do Lumo OS.
#[derive(Debug, Clone, PartialEq)]
pub struct A11yTokens {
    pub reduced_motion: bool,
    pub high_contrast: bool,
    pub font_scale: f32,
}

impl Default for A11yTokens {
    fn default() -> Self {
        Self {
            reduced_motion: false,
            high_contrast: false,
            font_scale: 1.0,
        }
    }
}

impl A11yTokens {
    /// Path padrao: ~/.config/lumo/accessibility.toml
    pub fn config_path() -> Option<PathBuf> {
        let home = std::env::var("HOME").ok()?;
        let mut p = PathBuf::from(home);
        p.push(".config/lumo/accessibility.toml");
        Some(p)
    }

    /// Le do disco. Fallback para defaults se arquivo nao existe ou invalido.
    pub fn load_from_disk() -> Self {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return Self::default(),
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => return Self::default(),
        };
        Self::parse_toml(&text).unwrap_or_default()
    }

    /// Salva no disco.
    pub fn save_to_disk(&self) -> Result<(), std::io::Error> {
        let path = Self::config_path().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "HOME nao definido")
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, self.to_toml())
    }

    /// Serializa para TOML.
    pub fn to_toml(&self) -> String {
        format!(
            "[accessibility]\nreduced_motion = {}\nhigh_contrast = {}\nfont_scale = {:.2}\n",
            self.reduced_motion, self.high_contrast, self.font_scale
        )
    }

    /// Parse TOML sem dependencia extra (subset para nosso formato).
    #[allow(clippy::result_unit_err)]
    pub fn parse_toml(text: &str) -> Result<Self, ()> {
        let mut tokens = Self::default();
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('#') || line.starts_with('[') || line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.splitn(2, '=').collect();
            if parts.len() != 2 {
                continue;
            }
            let key = parts[0].trim();
            let val = parts[1].trim().trim_matches('"');
            match key {
                "reduced_motion" => tokens.reduced_motion = val == "true",
                "high_contrast"  => tokens.high_contrast  = val == "true",
                "font_scale" => {
                    if let Ok(f) = val.parse::<f32>() {
                        tokens.font_scale = f.clamp(0.8, 1.4);
                    }
                }
                _ => {}
            }
        }
        Ok(tokens)
    }

    /// Multiplica tamanho de fonte pelo font_scale.
    pub fn scale_font(&self, base_pt: f32) -> f32 {
        base_pt * self.font_scale
    }

    /// Cor de fundo linear quando high_contrast=true. #000000.
    pub fn hc_bg_linear(&self) -> [f32; 4] {
        [0.0, 0.0, 0.0, 1.0]
    }

    /// Cor de primeiro plano linear quando high_contrast=true. #FFFFFF.
    pub fn hc_fg_linear(&self) -> [f32; 4] {
        [1.0, 1.0, 1.0, 1.0]
    }

    /// Accent linear quando high_contrast=true. #FFFF00 yellow.
    pub fn hc_accent_linear(&self) -> [f32; 4] {
        let to_lin = |c: f32| -> f32 {
            if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
        };
        [to_lin(1.0), to_lin(1.0), to_lin(0.0), 1.0]
    }

    /// Largura de borda: 2px high_contrast, 1px normal.
    pub fn border_width(&self) -> u32 {
        if self.high_contrast { 2 } else { 1 }
    }

    /// Largura de focus outline: 3px high_contrast, 1px normal.
    pub fn focus_outline_width(&self) -> u32 {
        if self.high_contrast { 3 } else { 1 }
    }
}

/// Watcher de accessibility.toml via notify. Callback chamado ao mudar.
pub fn watch_accessibility<F: Fn(A11yTokens) + Send + 'static>(callback: F) {
    use notify::{RecommendedWatcher, RecursiveMode, Watcher};
    use std::sync::mpsc;

    let path = match A11yTokens::config_path() {
        Some(p) => p,
        None => return,
    };

    let (tx, rx) = mpsc::channel();

    let mut watcher = match RecommendedWatcher::new(
        move |res: notify::Result<notify::Event>| {
            if let Ok(ev) = res {
                if matches!(ev.kind, notify::EventKind::Modify(_) | notify::EventKind::Create(_)) {
                    let _ = tx.send(());
                }
            }
        },
        notify::Config::default(),
    ) {
        Ok(w) => w,
        Err(_) => return,
    };

    let watch_dir = path.parent().unwrap_or(&path).to_owned();
    if watcher.watch(&watch_dir, RecursiveMode::NonRecursive).is_err() {
        return;
    }

    std::thread::spawn(move || {
        let _watcher = watcher;
        while let Ok(()) = rx.recv() {
            callback(A11yTokens::load_from_disk());
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a11y_defaults() {
        let t = A11yTokens::default();
        assert!(!t.reduced_motion);
        assert!(!t.high_contrast);
        assert!((t.font_scale - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn a11y_parse_reduced_motion() {
        let t = A11yTokens::parse_toml("[accessibility]\nreduced_motion = true\n").unwrap();
        assert!(t.reduced_motion);
        assert!(!t.high_contrast);
    }

    #[test]
    fn a11y_parse_high_contrast() {
        let t = A11yTokens::parse_toml("[accessibility]\nhigh_contrast = true\n").unwrap();
        assert!(t.high_contrast);
    }

    #[test]
    fn a11y_font_scale_clamp_high() {
        let t = A11yTokens::parse_toml("[accessibility]\nfont_scale = 2.0\n").unwrap();
        assert!((t.font_scale - 1.4).abs() < 0.01);
    }

    #[test]
    fn a11y_font_scale_apply() {
        let mut t = A11yTokens::default();
        t.font_scale = 1.2;
        let scaled = t.scale_font(11.0);
        assert!((scaled - 13.2).abs() < 0.01);
    }

    #[test]
    fn a11y_border_width_normal_vs_hc() {
        let mut t = A11yTokens::default();
        assert_eq!(t.border_width(), 1);
        t.high_contrast = true;
        assert_eq!(t.border_width(), 2);
    }

    #[test]
    fn a11y_focus_outline_hc() {
        let t = A11yTokens { high_contrast: true, ..Default::default() };
        assert_eq!(t.focus_outline_width(), 3);
    }

    #[test]
    fn a11y_hc_accent_is_yellow() {
        let t = A11yTokens { high_contrast: true, ..Default::default() };
        let a = t.hc_accent_linear();
        assert!((a[0] - 1.0).abs() < 0.001);
        assert!((a[1] - 1.0).abs() < 0.001);
        assert!(a[2] < 0.001);
    }

    #[test]
    fn a11y_toml_roundtrip() {
        let t = A11yTokens { reduced_motion: true, high_contrast: false, font_scale: 1.2 };
        let toml = t.to_toml();
        let t2 = A11yTokens::parse_toml(&toml).unwrap();
        assert_eq!(t.reduced_motion, t2.reduced_motion);
        assert_eq!(t.high_contrast, t2.high_contrast);
        assert!((t.font_scale - t2.font_scale).abs() < 0.01);
    }
}
