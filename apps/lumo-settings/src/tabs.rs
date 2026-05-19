//! Definicao das 8 abas do Settings.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Display,
    Wifi,
    Bluetooth,
    Audio,
    Battery,
    Appearance,
    Keyboard,
    Touchpad,
}

impl Tab {
    pub const ALL: &'static [Tab] = &[
        Tab::Display,
        Tab::Wifi,
        Tab::Bluetooth,
        Tab::Audio,
        Tab::Battery,
        Tab::Appearance,
        Tab::Keyboard,
        Tab::Touchpad,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Tab::Display    => "Display",
            Tab::Wifi       => "Wi-Fi",
            Tab::Bluetooth  => "Bluetooth",
            Tab::Audio      => "Audio",
            Tab::Battery    => "Bateria",
            Tab::Appearance => "Aparencia",
            Tab::Keyboard   => "Teclado",
            Tab::Touchpad   => "Touchpad",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Tab::Display    => "[disp]",
            Tab::Wifi       => "[wifi]",
            Tab::Bluetooth  => "[bt]",
            Tab::Audio      => "[aud]",
            Tab::Battery    => "[bat]",
            Tab::Appearance => "[apar]",
            Tab::Keyboard   => "[kbd]",
            Tab::Touchpad   => "[tpad]",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_count() {
        assert_eq!(Tab::ALL.len(), 8);
    }

    #[test]
    fn test_tab_labels_nonempty() {
        for tab in Tab::ALL {
            assert!(!tab.label().is_empty());
        }
    }

    #[test]
    fn test_tab_unique_labels() {
        let labels: std::collections::HashSet<_> = Tab::ALL.iter().map(|t| t.label()).collect();
        assert_eq!(labels.len(), Tab::ALL.len());
    }
}
