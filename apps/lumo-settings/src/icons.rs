//! Icones SVG embutidos para as abas do Settings.
//!
//! Todos os bytes sao compilados via `include_bytes!`.
//! Cores aplicadas via `Svg::style` + `currentColor` no SVG.

pub const DISPLAY:       &[u8] = include_bytes!("../icons/display.svg");
pub const WIFI:          &[u8] = include_bytes!("../icons/wifi.svg");
pub const BLUETOOTH:     &[u8] = include_bytes!("../icons/bluetooth.svg");
pub const AUDIO:         &[u8] = include_bytes!("../icons/audio.svg");
pub const BATTERY:       &[u8] = include_bytes!("../icons/battery.svg");
pub const APPEARANCE:    &[u8] = include_bytes!("../icons/appearance.svg");
pub const KEYBOARD:      &[u8] = include_bytes!("../icons/keyboard.svg");
pub const TOUCHPAD:      &[u8] = include_bytes!("../icons/touchpad.svg");
pub const ACCESSIBILITY: &[u8] = include_bytes!("../icons/accessibility.svg");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_icons_nonempty_svg() {
        let all: &[(&str, &[u8])] = &[
            ("display",       DISPLAY),
            ("wifi",          WIFI),
            ("bluetooth",     BLUETOOTH),
            ("audio",         AUDIO),
            ("battery",       BATTERY),
            ("appearance",    APPEARANCE),
            ("keyboard",      KEYBOARD),
            ("touchpad",      TOUCHPAD),
            ("accessibility", ACCESSIBILITY),
        ];
        for (name, bytes) in all {
            assert!(bytes.len() > 16, "SVG vazio para {}", name);
            assert!(
                bytes.starts_with(b"<svg") || bytes.starts_with(b"<?xml"),
                "bytes nao parecem SVG para {}",
                name
            );
        }
    }
}
