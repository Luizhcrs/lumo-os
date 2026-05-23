//! Data model do stylesheet parseado.

use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Stylesheet {
    /// :root vars (key sem `--` prefix).
    pub vars: HashMap<String, String>,
    /// Rules in source order. Selector + properties.
    pub rules: Vec<Rule>,
}

#[derive(Debug, Clone)]
pub struct Rule {
    pub selector: Selector,
    pub props: Vec<(String, PropertyValue)>,
}

/// Multi-class selector. `.pill.lumo` -> classes=["pill","lumo"].
/// Specificity = classes.len() (higher wins).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Selector {
    pub classes: Vec<String>,
}

impl Selector {
    pub fn specificity(&self) -> usize { self.classes.len() }

    /// Element with `set` of classes matches selector if all selector
    /// classes are in set.
    pub fn matches(&self, set: &[&str]) -> bool {
        self.classes.iter().all(|c| set.iter().any(|s| s == c))
    }
}

#[derive(Debug, Clone)]
pub enum PropertyValue {
    /// Pixel quantity (no unit OR `px`/`pt` collapsed to f32).
    Px(f32),
    /// Color #RRGGBB or #RRGGBBAA.
    Color(u32),
    /// Raw string (for inheritance / unknown).
    Str(String),
    /// var(--name) unresolved (resolved later in get()).
    Var(String),
}

impl Stylesheet {
    /// Query property value, with cascade + var resolution.
    /// `classes` = element class set (e.g. &["pill","lumo"]).
    pub fn get(&self, classes: &[&str], prop: &str) -> Option<&PropertyValue> {
        let mut best: Option<(usize, &PropertyValue)> = None;
        for r in &self.rules {
            if !r.selector.matches(classes) { continue; }
            for (k, v) in &r.props {
                if k == prop {
                    let spec = r.selector.specificity();
                    if best.map(|(s, _)| spec >= s).unwrap_or(true) {
                        best = Some((spec, v));
                    }
                }
            }
        }
        best.map(|(_, v)| v)
    }

    /// Resolve var lookup chain.
    pub fn resolve_var(&self, name: &str) -> Option<&str> {
        self.vars.get(name).map(|s| s.as_str())
    }

    /// Convenience: get f32 px, resolving vars.
    pub fn get_px(&self, classes: &[&str], prop: &str) -> Option<f32> {
        let v = self.get(classes, prop)?;
        self.resolve_px(v)
    }

    /// Convenience: get color u32 RGBA, resolving vars.
    pub fn get_color(&self, classes: &[&str], prop: &str) -> Option<u32> {
        let v = self.get(classes, prop)?;
        self.resolve_color(v)
    }

    fn resolve_px(&self, v: &PropertyValue) -> Option<f32> {
        match v {
            PropertyValue::Px(p) => Some(*p),
            PropertyValue::Var(name) => {
                let raw = self.resolve_var(name)?;
                parse_px_literal(raw)
            }
            PropertyValue::Str(s) => parse_px_literal(s),
            _ => None,
        }
    }

    fn resolve_color(&self, v: &PropertyValue) -> Option<u32> {
        match v {
            PropertyValue::Color(c) => Some(*c),
            PropertyValue::Var(name) => {
                let raw = self.resolve_var(name)?;
                parse_color_literal(raw)
            }
            PropertyValue::Str(s) => parse_color_literal(s),
            _ => None,
        }
    }
}

pub fn parse_px_literal(s: &str) -> Option<f32> {
    let s = s.trim();
    let s = s.strip_suffix("px").or_else(|| s.strip_suffix("pt")).unwrap_or(s);
    s.trim().parse::<f32>().ok()
}

pub fn parse_color_literal(s: &str) -> Option<u32> {
    let s = s.trim();
    let s = s.strip_prefix('#')?;
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(match s.len() {
        6 => (v << 8) | 0xFF,  // RGB → RGBA
        8 => v,                  // RGBA
        _ => return None,
    })
}
