//! status_pills.rs — helpers UX2/UX3 pra renderizar pills de degraded/freeze.
//!
//! `compute_degraded_text` consolida set de codigos em texto curto pra pill.
//! `freeze_title_suffix` retorna " (Nao responde)" se pid do app em foco
//! esta em freeze set.
//!
//! Funcoes puras pra testar sem hardware Wayland.

use std::collections::BTreeMap;

/// Texto curto pra pill warning quando ha features degradadas.
/// None = sem degraded, nao mostrar pill.
/// 1 feature = label.
/// 2+ features = "N issues" pra economizar espaco.
pub fn compute_degraded_text(degraded: &BTreeMap<String, String>) -> Option<String> {
    match degraded.len() {
        0 => None,
        1 => degraded.values().next().cloned(),
        n => Some(format!("{} issues", n)),
    }
}

/// Tooltip multilinha com todos codigos + labels.
pub fn degraded_tooltip(degraded: &BTreeMap<String, String>) -> String {
    let mut lines: Vec<String> = degraded.iter().map(|(c, l)| format!("{}: {}", c, l)).collect();
    lines.sort();
    lines.join("\n")
}

/// Sufixo a appendar no titulo do app em foco se esta em freeze.
pub fn freeze_title_suffix(frozen: &BTreeMap<u32, String>, focus_pid: u32) -> &'static str {
    if frozen.contains_key(&focus_pid) {
        " (Nao responde)"
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compute_degraded_none_when_empty() {
        let m = BTreeMap::new();
        assert_eq!(compute_degraded_text(&m), None);
    }

    #[test]
    fn compute_degraded_single_returns_label() {
        let mut m = BTreeMap::new();
        m.insert("WM-RENDER-002".into(), "Vsync off".into());
        assert_eq!(compute_degraded_text(&m).as_deref(), Some("Vsync off"));
    }

    #[test]
    fn compute_degraded_two_returns_count() {
        let mut m = BTreeMap::new();
        m.insert("A".into(), "labelA".into());
        m.insert("B".into(), "labelB".into());
        assert_eq!(compute_degraded_text(&m).as_deref(), Some("2 issues"));
    }

    #[test]
    fn compute_degraded_three_returns_count() {
        let mut m = BTreeMap::new();
        m.insert("A".into(), "x".into());
        m.insert("B".into(), "y".into());
        m.insert("C".into(), "z".into());
        assert_eq!(compute_degraded_text(&m).as_deref(), Some("3 issues"));
    }

    #[test]
    fn tooltip_lists_all_sorted() {
        let mut m = BTreeMap::new();
        m.insert("Z".into(), "z-label".into());
        m.insert("A".into(), "a-label".into());
        let t = degraded_tooltip(&m);
        let pos_a = t.find("A: a-label").expect("A presente");
        let pos_z = t.find("Z: z-label").expect("Z presente");
        assert!(pos_a < pos_z, "ordem alfabetica");
    }

    #[test]
    fn tooltip_empty_when_no_degraded() {
        let m = BTreeMap::new();
        assert_eq!(degraded_tooltip(&m), "");
    }

    #[test]
    fn freeze_suffix_returns_label_when_pid_frozen() {
        let mut m = BTreeMap::new();
        m.insert(42, "lumo-files".into());
        assert_eq!(freeze_title_suffix(&m, 42), " (Nao responde)");
    }

    #[test]
    fn freeze_suffix_returns_empty_when_pid_not_frozen() {
        let mut m = BTreeMap::new();
        m.insert(42, "x".into());
        assert_eq!(freeze_title_suffix(&m, 99), "");
    }

    #[test]
    fn freeze_suffix_empty_map() {
        let m = BTreeMap::new();
        assert_eq!(freeze_title_suffix(&m, 42), "");
    }
}
