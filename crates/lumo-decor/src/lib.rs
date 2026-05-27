//! lumo-decor: libdecor plugin Lumo OS.
//!
//! Implementacao real em c-src/lumo-decor.c — exporta symbol
//! `libdecor_plugin_description` que libdecor loader procura via dlsym.
//!
//! Este modulo Rust e wrapper minimo (testes de helpers puros + scaffold
//! de build). Plugin compila como cdylib `liblumo_decor.so`.
//!
//! Install:
//!   sudo cp target/release/liblumo_decor.so /usr/lib/libdecor/plugins-1/libdecor-lumo.so
//!
//! Apps que usam libdecor (Firefox/mpv/Blender/GTK4/SDL3) automaticamente
//! pegam plugin Lumo (loader prioritiza por desktop = "lumo" via XDG_CURRENT_DESKTOP).

/// Layout constants — sync com c-src/draw.h e SSD do compositor (lumo-wm).
pub const TITLEBAR_HEIGHT: i32 = 32;
pub const BUTTON_SIZE: i32 = 14;
pub const BUTTON_GAP: i32 = 8;
pub const BUTTON_MARGIN_RIGHT: i32 = 12;

/// Cores Lumo tema dark — sync com lumo-foundation::LumoColors default.
pub const TITLEBAR_BG_DARK: u32 = 0x2A2A2A;
pub const TITLEBAR_FG_DARK: u32 = 0xE0E0E0;
pub const BTN_CLOSE: u32 = 0xE74C3C; // vermelho
pub const BTN_MIN: u32 = 0xF1C40F; // amarelo
pub const BTN_MAX: u32 = 0x2ECC71; // verde

/// Detecta button index pela posicao relativa do click. Pura, testavel.
/// content_w = largura total da titlebar. x = pixel x relativo a titlebar.
/// Retorna 0=close, 1=min, 2=max, ou None.
pub fn hit_test_button(content_w: i32, x: i32, y: i32) -> Option<u8> {
    if y < (TITLEBAR_HEIGHT - BUTTON_SIZE) / 2
        || y >= (TITLEBAR_HEIGHT + BUTTON_SIZE) / 2
    {
        return None;
    }
    let total_btns_w = BUTTON_SIZE * 3 + BUTTON_GAP * 2;
    let btns_start_x = content_w - BUTTON_MARGIN_RIGHT - total_btns_w;
    for i in 0..3u8 {
        let bx = btns_start_x + (BUTTON_SIZE + BUTTON_GAP) * i as i32;
        if x >= bx && x < bx + BUTTON_SIZE {
            return Some(i);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit_test_outside_y_range_returns_none() {
        assert!(hit_test_button(800, 700, 0).is_none());
        assert!(hit_test_button(800, 700, 31).is_none());
    }

    #[test]
    fn hit_test_max_button_right() {
        // 3 botoes em sequencia. Max = idx 2, rightmost.
        let cy = TITLEBAR_HEIGHT / 2;
        let total = BUTTON_SIZE * 3 + BUTTON_GAP * 2;
        let start = 800 - BUTTON_MARGIN_RIGHT - total;
        let max_x = start + (BUTTON_SIZE + BUTTON_GAP) * 2 + BUTTON_SIZE / 2;
        assert_eq!(hit_test_button(800, max_x, cy), Some(2));
    }

    #[test]
    fn hit_test_close_button_left_of_btns() {
        let cy = TITLEBAR_HEIGHT / 2;
        let total = BUTTON_SIZE * 3 + BUTTON_GAP * 2;
        let start = 800 - BUTTON_MARGIN_RIGHT - total;
        assert_eq!(hit_test_button(800, start + 2, cy), Some(0));
    }

    #[test]
    fn hit_test_middle_button() {
        let cy = TITLEBAR_HEIGHT / 2;
        let total = BUTTON_SIZE * 3 + BUTTON_GAP * 2;
        let start = 800 - BUTTON_MARGIN_RIGHT - total;
        let mid_x = start + BUTTON_SIZE + BUTTON_GAP + BUTTON_SIZE / 2;
        assert_eq!(hit_test_button(800, mid_x, cy), Some(1));
    }

    #[test]
    fn hit_test_left_area_no_btn() {
        let cy = TITLEBAR_HEIGHT / 2;
        assert!(hit_test_button(800, 50, cy).is_none());
    }

    #[test]
    fn hit_test_gap_between_buttons() {
        let cy = TITLEBAR_HEIGHT / 2;
        let total = BUTTON_SIZE * 3 + BUTTON_GAP * 2;
        let start = 800 - BUTTON_MARGIN_RIGHT - total;
        // Gap entre btn 0 e btn 1.
        let gap_x = start + BUTTON_SIZE + BUTTON_GAP / 2;
        assert!(hit_test_button(800, gap_x, cy).is_none());
    }

    #[test]
    fn titlebar_height_matches_lumo_ssd() {
        // Lumo SSD compositor usa 32px (W37). Sync obrigatorio.
        assert_eq!(TITLEBAR_HEIGHT, 32);
    }

    #[test]
    fn buttons_macos_color_order() {
        // close=vermelho, min=amarelo, max=verde (Mac-style).
        // Sync com SSD compositor.
        assert_eq!(BTN_CLOSE & 0xFF0000, 0xE70000 & 0xFF0000);
        assert_eq!(BTN_MIN & 0xFF0000, 0xF10000 & 0xFF0000);
        assert_eq!(BTN_MAX & 0xFF0000, 0x2E0000 & 0xFF0000);
    }
}
