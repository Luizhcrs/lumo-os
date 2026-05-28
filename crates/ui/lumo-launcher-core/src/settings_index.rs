//! settings_index.rs — index estatico de settings keywords pra Spotlight.
//!
//! Query "wifi" → resultado "Settings: Wifi" + acao spawn lumo-settings --tab=wifi.
//! Cobre os paineis principais. Fuzzy match contra labels + keywords.

#[derive(Debug, Clone, PartialEq)]
pub struct SettingsEntry {
    pub label: &'static str,
    pub keywords: &'static [&'static str],
    pub tab: &'static str,
}

pub const SETTINGS: &[SettingsEntry] = &[
    SettingsEntry {
        label: "Wi-Fi",
        keywords: &["wifi", "wireless", "rede", "network", "internet"],
        tab: "wifi",
    },
    SettingsEntry {
        label: "Bluetooth",
        keywords: &["bluetooth", "bt", "bluez", "pair"],
        tab: "bluetooth",
    },
    SettingsEntry {
        label: "Brightness",
        keywords: &["brightness", "brilho", "tela", "display", "lcd", "backlight"],
        tab: "brightness",
    },
    SettingsEntry {
        label: "Sound",
        keywords: &["sound", "som", "audio", "volume", "speaker", "mic", "microfone"],
        tab: "sound",
    },
    SettingsEntry {
        label: "Power",
        keywords: &["power", "energia", "bateria", "battery", "suspend", "sleep"],
        tab: "power",
    },
    SettingsEntry {
        label: "Display",
        keywords: &["display", "monitor", "screen", "resolution", "scale", "hdmi"],
        tab: "display",
    },
    SettingsEntry {
        label: "Keyboard",
        keywords: &["keyboard", "teclado", "layout", "shortcut", "atalho"],
        tab: "keyboard",
    },
    SettingsEntry {
        label: "Mouse / Touchpad",
        keywords: &["mouse", "touchpad", "trackpad", "cursor", "scroll", "gesture"],
        tab: "mouse",
    },
    SettingsEntry {
        label: "Theme",
        keywords: &["theme", "tema", "dark", "light", "color", "appearance"],
        tab: "theme",
    },
    SettingsEntry {
        label: "Accessibility",
        keywords: &[
            "accessibility",
            "acessibilidade",
            "a11y",
            "contrast",
            "magnifier",
            "screen reader",
        ],
        tab: "accessibility",
    },
    SettingsEntry {
        label: "Privacy",
        keywords: &["privacy", "privacidade", "location", "camera", "permission"],
        tab: "privacy",
    },
    SettingsEntry {
        label: "Updates",
        keywords: &["update", "atualizar", "atualizacao", "pacman", "upgrade"],
        tab: "updates",
    },
];

/// Match query contra keywords. Retorna entries que tem qualquer keyword
/// containing query (case insensitive).
pub fn search(query: &str) -> Vec<&'static SettingsEntry> {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return Vec::new();
    }
    SETTINGS
        .iter()
        .filter(|e| {
            e.label.to_lowercase().contains(&q)
                || e.keywords.iter().any(|k| k.contains(&q.as_str()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_empty_returns_empty() {
        assert!(search("").is_empty());
    }

    #[test]
    fn search_wifi_returns_wifi_entry() {
        let r = search("wifi");
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].tab, "wifi");
    }

    #[test]
    fn search_portuguese_brilho() {
        let r = search("brilho");
        assert!(r.iter().any(|e| e.tab == "brightness"));
    }

    #[test]
    fn search_partial_blue_returns_bluetooth() {
        let r = search("blue");
        assert!(r.iter().any(|e| e.tab == "bluetooth"));
    }

    #[test]
    fn search_case_insensitive() {
        let a = search("WIFI");
        let b = search("wifi");
        assert_eq!(a.len(), b.len());
    }

    #[test]
    fn search_no_match_empty() {
        assert!(search("xyzunknown123").is_empty());
    }

    #[test]
    fn search_substring_match_label() {
        let r = search("Bluetooth");
        assert_eq!(r.len(), 1);
    }

    #[test]
    fn search_multiple_results() {
        // "audio" matches Sound; "som" tambem matches Sound only.
        let r = search("audio");
        assert!(r.iter().any(|e| e.tab == "sound"));
    }

    #[test]
    fn search_battery_matches_power() {
        let r = search("battery");
        assert!(r.iter().any(|e| e.tab == "power"));
    }

    #[test]
    fn search_dark_matches_theme() {
        let r = search("dark");
        assert!(r.iter().any(|e| e.tab == "theme"));
    }

    #[test]
    fn search_screen_matches_display() {
        let r = search("screen");
        assert!(r.iter().any(|e| e.tab == "display"));
    }

    #[test]
    fn settings_const_has_12_entries() {
        assert_eq!(SETTINGS.len(), 12);
    }

    #[test]
    fn all_entries_have_unique_tabs() {
        let mut tabs: Vec<&str> = SETTINGS.iter().map(|e| e.tab).collect();
        tabs.sort();
        let original_len = tabs.len();
        tabs.dedup();
        assert_eq!(tabs.len(), original_len, "tabs duplicados");
    }

    #[test]
    fn all_entries_have_non_empty_keywords() {
        for e in SETTINGS {
            assert!(!e.keywords.is_empty(), "tab={} sem keywords", e.tab);
        }
    }
}
