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
    pub fn specificity(&self) -> usize {
        self.classes.len()
    }

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
            if !r.selector.matches(classes) {
                continue;
            }
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
    let s = s
        .strip_suffix("px")
        .or_else(|| s.strip_suffix("pt"))
        .unwrap_or(s);
    s.trim().parse::<f32>().ok()
}

pub fn parse_color_literal(s: &str) -> Option<u32> {
    let s = s.trim();
    let s = s.strip_prefix('#')?;
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(match s.len() {
        6 => (v << 8) | 0xFF, // RGB → RGBA
        8 => v,               // RGBA
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(classes: &[&str]) -> Selector {
        Selector {
            classes: classes.iter().map(|s| s.to_string()).collect(),
        }
    }

    // --- Selector ---

    #[test]
    fn specificity_equals_class_count() {
        assert_eq!(sel(&[]).specificity(), 0);
        assert_eq!(sel(&["pill"]).specificity(), 1);
        assert_eq!(sel(&["pill", "lumo", "dark"]).specificity(), 3);
    }

    #[test]
    fn matches_when_all_selector_classes_in_set() {
        assert!(sel(&["pill"]).matches(&["pill", "lumo"]));
        assert!(sel(&["pill", "lumo"]).matches(&["lumo", "pill", "extra"]));
    }

    #[test]
    fn matches_false_when_class_missing() {
        assert!(!sel(&["pill", "missing"]).matches(&["pill"]));
        assert!(!sel(&["nope"]).matches(&["pill", "lumo"]));
    }

    #[test]
    fn empty_selector_matches_everything() {
        // Selector :root style sem classes deve matchar qualquer set.
        assert!(sel(&[]).matches(&[]));
        assert!(sel(&[]).matches(&["pill"]));
    }

    // --- parse_px_literal ---

    #[test]
    fn parse_px_with_px_suffix() {
        assert_eq!(parse_px_literal("14px"), Some(14.0));
    }

    #[test]
    fn parse_px_with_pt_suffix() {
        assert_eq!(parse_px_literal("13pt"), Some(13.0));
    }

    #[test]
    fn parse_px_no_suffix() {
        assert_eq!(parse_px_literal("8"), Some(8.0));
    }

    #[test]
    fn parse_px_trim_whitespace() {
        assert_eq!(parse_px_literal("  16px  "), Some(16.0));
    }

    #[test]
    fn parse_px_fractional() {
        assert_eq!(parse_px_literal("1.5px"), Some(1.5));
    }

    #[test]
    fn parse_px_invalid_returns_none() {
        assert_eq!(parse_px_literal("abc"), None);
        assert_eq!(parse_px_literal("12emm"), None);
    }

    // --- parse_color_literal ---

    #[test]
    fn parse_color_rrggbb_adds_full_alpha() {
        assert_eq!(parse_color_literal("#FF0000"), Some(0xFF0000_FF));
        assert_eq!(parse_color_literal("#00ff00"), Some(0x00FF00_FF));
    }

    #[test]
    fn parse_color_rrggbbaa_preserves_alpha() {
        assert_eq!(parse_color_literal("#11223344"), Some(0x11223344));
        assert_eq!(parse_color_literal("#FF000080"), Some(0xFF000080));
    }

    #[test]
    fn parse_color_invalid_length_none() {
        assert_eq!(parse_color_literal("#F00"), None); // 3-digit nao suportado
        assert_eq!(parse_color_literal("#FF000"), None);
        assert_eq!(parse_color_literal("#FF000000F"), None);
    }

    #[test]
    fn parse_color_missing_hash_none() {
        assert_eq!(parse_color_literal("FF0000"), None);
    }

    #[test]
    fn parse_color_invalid_hex_none() {
        assert_eq!(parse_color_literal("#XX0000"), None);
    }

    // --- Stylesheet.get + cascade ---

    fn make_sheet() -> Stylesheet {
        Stylesheet {
            vars: {
                let mut m = HashMap::new();
                m.insert("accent".into(), "#00AAFF".into());
                m.insert("pad".into(), "8px".into());
                m
            },
            rules: vec![
                Rule {
                    selector: sel(&["pill"]),
                    props: vec![
                        ("bg".into(), PropertyValue::Color(0xAA0000_FF)),
                        ("pad-x".into(), PropertyValue::Px(10.0)),
                    ],
                },
                Rule {
                    selector: sel(&["pill", "lumo"]),
                    props: vec![("bg".into(), PropertyValue::Color(0x00AA00_FF))],
                },
            ],
        }
    }

    #[test]
    fn get_returns_higher_specificity() {
        let s = make_sheet();
        let v = s.get(&["pill", "lumo"], "bg");
        match v {
            Some(PropertyValue::Color(c)) => assert_eq!(*c, 0x00AA00_FF),
            _ => panic!("expected color"),
        }
    }

    #[test]
    fn get_falls_back_to_lower_specificity() {
        let s = make_sheet();
        let v = s.get(&["pill"], "bg");
        match v {
            Some(PropertyValue::Color(c)) => assert_eq!(*c, 0xAA0000_FF),
            _ => panic!("expected color"),
        }
    }

    #[test]
    fn get_returns_none_for_unmatched_prop() {
        let s = make_sheet();
        assert!(s.get(&["pill"], "border").is_none());
    }

    #[test]
    fn get_px_resolves_direct() {
        let s = make_sheet();
        assert_eq!(s.get_px(&["pill"], "pad-x"), Some(10.0));
    }

    #[test]
    fn resolve_var_returns_value() {
        let s = make_sheet();
        assert_eq!(s.resolve_var("accent"), Some("#00AAFF"));
        assert_eq!(s.resolve_var("missing"), None);
    }
}
