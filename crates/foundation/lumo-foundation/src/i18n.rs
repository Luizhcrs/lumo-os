//! i18n.rs - Localizacao Lumo OS (W11.A).
//!
//! Carrega locale de `~/.config/lumo/locale.toml` ou env `LANG`.
//! Fallback: EN-US embutido. Segundo fallback: chave literal.
//!
//! Uso:
//!   use lumo_foundation::i18n::{t, I18n};
//!   I18n::init();                     // chama uma vez no startup
//!   let s = t!("battery.title");      // retorna "Bateria" ou "Battery"
//!
//! Hot reload: chamar I18n::reload() a qualquer momento.

use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

// ---------------------------------------------------------------------------
// Locale embutidos (compile-time)
// ---------------------------------------------------------------------------

const LOCALE_PT_BR: &str = include_str!("../locales/pt-BR.toml");
const LOCALE_EN_US: &str = include_str!("../locales/en-US.toml");

// ---------------------------------------------------------------------------
// Flat map: "section.key" -> valor string
// ---------------------------------------------------------------------------

fn parse_toml_flat(src: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut section = String::new();
    for raw in src.lines() {
        let line = raw.trim();
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].to_string();
            continue;
        }
        if let Some(eq) = line.find('=') {
            let key = line[..eq].trim().to_string();
            let val_raw = line[eq + 1..].trim();
            let val = val_raw.trim_matches('"').to_string();
            let full_key = if section.is_empty() {
                key
            } else {
                format!("{}.{}", section, key)
            };
            map.insert(full_key, val);
        }
    }
    map
}

// ---------------------------------------------------------------------------
// Locale detection
// ---------------------------------------------------------------------------

/// Detecta locale via LANG env ou config file.
pub fn detect_locale() -> String {
    if let Ok(home) = std::env::var("HOME") {
        let path = format!("{home}/.config/lumo/locale.toml");
        if let Ok(src) = std::fs::read_to_string(&path) {
            for line in src.lines() {
                let l = line.trim();
                if l.starts_with("locale") {
                    if let Some(eq) = l.find('=') {
                        let val = l[eq + 1..].trim().trim_matches('"').to_string();
                        if !val.is_empty() {
                            return val;
                        }
                    }
                }
            }
        }
    }
    if let Ok(lang) = std::env::var("LANG") {
        let lang = lang.split('.').next().unwrap_or("").replace('_', "-");
        if !lang.is_empty() {
            return lang;
        }
    }
    "en-US".to_string()
}

// ---------------------------------------------------------------------------
// I18n singleton
// ---------------------------------------------------------------------------

static I18N: OnceLock<RwLock<I18nState>> = OnceLock::new();

struct I18nState {
    locale: String,
    strings: HashMap<String, String>,
    fallback: HashMap<String, String>,
}

impl I18nState {
    fn build(locale: &str) -> Self {
        let strings = if locale.starts_with("pt") {
            parse_toml_flat(LOCALE_PT_BR)
        } else {
            parse_toml_flat(LOCALE_EN_US)
        };
        let fallback = parse_toml_flat(LOCALE_EN_US);
        Self {
            locale: locale.to_string(),
            strings,
            fallback,
        }
    }

    fn get(&self, key: &str) -> String {
        if let Some(v) = self.strings.get(key) {
            return v.clone();
        }
        if let Some(v) = self.fallback.get(key) {
            return v.clone();
        }
        key.to_string()
    }
}

/// Ponto de entrada do sistema de localizacao.
pub struct I18n;

impl I18n {
    /// Inicializa singleton com locale detectado. Idempotente.
    pub fn init() {
        let locale = detect_locale();
        I18N.get_or_init(|| RwLock::new(I18nState::build(&locale)));
    }

    /// Inicializa com locale explicito (util em testes e dev tools).
    pub fn init_with(locale: &str) {
        I18N.get_or_init(|| RwLock::new(I18nState::build(locale)));
    }

    /// Recarrega strings com locale atual detectado.
    pub fn reload() {
        if let Some(lock) = I18N.get() {
            if let Ok(mut state) = lock.write() {
                let locale = detect_locale();
                *state = I18nState::build(&locale);
            }
        }
    }

    /// Retorna string traduzida para `key`. Fallback: EN-US -> chave literal.
    pub fn get(key: &str) -> String {
        if let Some(lock) = I18N.get() {
            if let Ok(state) = lock.read() {
                return state.get(key);
            }
        }
        let fallback = parse_toml_flat(LOCALE_EN_US);
        fallback
            .get(key)
            .cloned()
            .unwrap_or_else(|| key.to_string())
    }

    /// Retorna locale ativo.
    pub fn locale() -> String {
        if let Some(lock) = I18N.get() {
            if let Ok(state) = lock.read() {
                return state.locale.clone();
            }
        }
        "en-US".to_string()
    }
}

// ---------------------------------------------------------------------------
// Macro t!
// ---------------------------------------------------------------------------

/// Retorna string localizada para a chave.
#[macro_export]
macro_rules! t {
    ($key:expr) => {
        $crate::i18n::I18n::get($key)
    };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn s(locale: &str) -> I18nState {
        I18nState::build(locale)
    }

    #[test]
    fn pt_br_battery_title() {
        assert_eq!(s("pt-BR").get("battery.title"), "Bateria");
    }

    #[test]
    fn en_us_battery_title() {
        assert_eq!(s("en-US").get("battery.title"), "Battery");
    }

    #[test]
    fn pt_br_wifi_connect() {
        assert_eq!(s("pt-BR").get("wifi.connect"), "Conectar");
    }

    #[test]
    fn unknown_key_returns_key_literal() {
        assert_eq!(s("en-US").get("nonexistent.key"), "nonexistent.key");
    }

    #[test]
    fn app_name_both_locales() {
        assert_eq!(s("pt-BR").get("app.name"), "Lumo");
        assert_eq!(s("en-US").get("app.name"), "Lumo");
    }

    #[test]
    fn parse_toml_flat_basic() {
        let src = "[section]\nkey = \"value\"\n";
        let m = parse_toml_flat(src);
        assert_eq!(m.get("section.key").map(|s| s.as_str()), Some("value"));
    }

    #[test]
    fn parse_toml_flat_ignores_comments() {
        let src = "# comment\n[s]\nk = \"v\"\n";
        let m = parse_toml_flat(src);
        assert_eq!(m.len(), 1);
    }

    #[test]
    fn detect_locale_non_empty() {
        let l = detect_locale();
        assert!(!l.is_empty());
    }

    #[test]
    fn menu_items_pt_br() {
        let st = s("pt-BR");
        assert_eq!(st.get("menu.shutdown"), "Desligar...");
        assert_eq!(st.get("menu.suspend"), "Suspender");
    }

    #[test]
    fn notif_clear_all_en() {
        assert_eq!(s("en-US").get("notif.clear_all"), "Clear all");
    }
}
