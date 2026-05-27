//! status_pills.rs — helpers UX2/UX3 pra renderizar pills de degraded/freeze.
//!
//! `compute_degraded_text` consolida set de codigos em texto curto pra pill.
//! `freeze_title_suffix` retorna " (Nao responde)" se pid do app em foco
//! esta em freeze set.
//!
//! Funcoes puras pra testar sem hardware Wayland.

use crate::menu::DynMenuItem;
use std::collections::BTreeMap;

/// Items fallback do dropdown appmenu quando app nao expoe dbusmenu items
/// (Chromium sem libdbusmenu, terminais, apps minimal). Mantido em sync
/// com paint_frame em state.rs. Centralizado pra teste + unicidade.
pub fn fallback_menu_items() -> Vec<DynMenuItem<'static>> {
    vec![
        DynMenuItem::action("Sobre"),
        DynMenuItem::action("Versao"),
        DynMenuItem::action("Ajuda"),
        DynMenuItem::separator(),
        DynMenuItem::action("Fechar"),
    ]
}

/// Codigos que sao CONFIG INFO (opt-out por design via ADRs) e NAO
/// devem renderizar pill amber. Bar filtra esses antes computar texto.
/// Sincronizado com Severity::ConfigInfo em lumo-error.
const CONFIG_INFO_CODES: &[&str] = &["WM-COLOR-OFF", "WM-ICON-OFF"];

/// Filtra map descartando config_info codes. Retorna nova map.
pub fn filter_runtime_degraded(degraded: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    degraded
        .iter()
        .filter(|(k, _)| !CONFIG_INFO_CODES.contains(&k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect()
}

/// Texto curto pra pill warning quando ha features degradadas RUNTIME.
/// None = sem degraded runtime, nao mostrar pill.
/// 1 feature = label.
/// 2+ features = "N issues" pra economizar espaco.
/// Filtra config_info codes automaticamente.
pub fn compute_degraded_text(degraded: &BTreeMap<String, String>) -> Option<String> {
    let runtime = filter_runtime_degraded(degraded);
    match runtime.len() {
        0 => None,
        1 => runtime.values().next().cloned(),
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

    #[test]
    fn config_info_codes_filtered_out_of_pill() {
        let mut m = BTreeMap::new();
        m.insert("WM-COLOR-OFF".into(), "Color mgmt off".into());
        m.insert("WM-ICON-OFF".into(), "Icons disabled".into());
        // Sem runtime degraded: pill nao deve aparecer.
        assert_eq!(compute_degraded_text(&m), None);
    }

    #[test]
    fn runtime_degraded_passes_through_filter() {
        let mut m = BTreeMap::new();
        m.insert("WM-COLOR-OFF".into(), "Color mgmt off".into()); // ignored
        m.insert("WM-RENDER-002".into(), "Vsync off".into()); // runtime
        assert_eq!(compute_degraded_text(&m).as_deref(), Some("Vsync off"));
    }

    #[test]
    fn mixed_count_excludes_config_info() {
        let mut m = BTreeMap::new();
        m.insert("WM-COLOR-OFF".into(), "x".into());
        m.insert("WM-ICON-OFF".into(), "y".into());
        m.insert("WM-RENDER-002".into(), "vsync".into());
        m.insert("WM-GPU-LOST".into(), "gpu".into());
        // Apenas 2 runtime, mostra "2 issues".
        assert_eq!(compute_degraded_text(&m).as_deref(), Some("2 issues"));
    }

    #[test]
    fn filter_runtime_returns_only_runtime_keys() {
        let mut m = BTreeMap::new();
        m.insert("WM-COLOR-OFF".into(), "x".into());
        m.insert("WM-RENDER-002".into(), "y".into());
        let r = filter_runtime_degraded(&m);
        assert!(!r.contains_key("WM-COLOR-OFF"));
        assert!(r.contains_key("WM-RENDER-002"));
    }

    #[test]
    fn fallback_menu_has_5_items_with_separator() {
        let items = fallback_menu_items();
        assert_eq!(items.len(), 5);
        assert!(items[3].label.is_empty(), "idx 3 separator");
        assert!(!items[3].is_clickable());
    }

    #[test]
    fn fallback_menu_clickable_items_are_action() {
        let items = fallback_menu_items();
        let clickable: Vec<&str> = items
            .iter()
            .filter(|it| it.is_clickable())
            .map(|it| it.label)
            .collect();
        assert_eq!(clickable, vec!["Sobre", "Versao", "Ajuda", "Fechar"]);
    }

    #[test]
    fn fallback_menu_close_is_last_item() {
        let items = fallback_menu_items();
        let last = items.last().expect("non-empty");
        assert_eq!(last.label, "Fechar");
        assert!(last.is_clickable());
    }
}
