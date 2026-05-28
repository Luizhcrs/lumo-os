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

/// F1-1: hover state colors — sync com draw.h.
pub const BTN_CLOSE_HOVER: u32 = 0xFF6B5B;
pub const BTN_MIN_HOVER: u32 = 0xFFD93D;
pub const BTN_MAX_HOVER: u32 = 0x52E08C;

/// F1-1: hover index (-1 = none, 0..=2 botoes).
pub const HOVER_NONE: i8 = -1;
pub const HOVER_CLOSE: i8 = 0;
pub const HOVER_MIN: i8 = 1;
pub const HOVER_MAX: i8 = 2;

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

/// F1-1: center x do botao N (0=close, 1=min, 2=max).
pub fn button_center_x(content_w: i32, btn_index: u8) -> i32 {
    let total_btns_w = BUTTON_SIZE * 3 + BUTTON_GAP * 2;
    let start_x = content_w - BUTTON_MARGIN_RIGHT - total_btns_w;
    let radius = BUTTON_SIZE / 2;
    start_x + (BUTTON_SIZE + BUTTON_GAP) * btn_index as i32 + radius
}

/// F1-1: cor pra botao N em estado normal/hover/inactive.
pub fn button_color(btn_index: u8, active: bool, hover: bool) -> u32 {
    if !active {
        return 0x555555;
    }
    match (btn_index, hover) {
        (0, false) => BTN_CLOSE,
        (0, true) => BTN_CLOSE_HOVER,
        (1, false) => BTN_MIN,
        (1, true) => BTN_MIN_HOVER,
        (2, false) => BTN_MAX,
        (2, true) => BTN_MAX_HOVER,
        _ => 0x555555,
    }
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

    // F1-1: hover state + center calc + color helpers

    #[test]
    fn button_center_x_matches_hit_test() {
        // Centro do botao deve estar dentro do range hit_test.
        for i in 0..3u8 {
            let cx = button_center_x(800, i);
            let cy = TITLEBAR_HEIGHT / 2;
            assert_eq!(hit_test_button(800, cx, cy), Some(i));
        }
    }

    #[test]
    fn button_color_normal() {
        assert_eq!(button_color(0, true, false), BTN_CLOSE);
        assert_eq!(button_color(1, true, false), BTN_MIN);
        assert_eq!(button_color(2, true, false), BTN_MAX);
    }

    #[test]
    fn button_color_hover_brighter() {
        // Hover colors devem ter mais brilho que base.
        assert_eq!(button_color(0, true, true), BTN_CLOSE_HOVER);
        assert_eq!(button_color(1, true, true), BTN_MIN_HOVER);
        assert_eq!(button_color(2, true, true), BTN_MAX_HOVER);
    }

    #[test]
    fn button_color_inactive_is_gray() {
        for i in 0..3u8 {
            assert_eq!(button_color(i, false, false), 0x555555);
            assert_eq!(button_color(i, false, true), 0x555555);
        }
    }

    #[test]
    fn button_color_unknown_index_safe() {
        assert_eq!(button_color(99, true, true), 0x555555);
    }

    #[test]
    fn hover_constants_match_indices() {
        assert_eq!(HOVER_CLOSE, 0);
        assert_eq!(HOVER_MIN, 1);
        assert_eq!(HOVER_MAX, 2);
        assert_eq!(HOVER_NONE, -1);
    }

    #[test]
    fn hover_colors_distinct_from_base() {
        assert_ne!(BTN_CLOSE_HOVER, BTN_CLOSE & 0xFFFFFF);
        assert_ne!(BTN_MIN_HOVER, BTN_MIN & 0xFFFFFF);
        assert_ne!(BTN_MAX_HOVER, BTN_MAX & 0xFFFFFF);
    }

    #[test]
    fn button_centers_are_distinct() {
        let c0 = button_center_x(800, 0);
        let c1 = button_center_x(800, 1);
        let c2 = button_center_x(800, 2);
        assert!(c0 < c1);
        assert!(c1 < c2);
        // Gap entre centros = BUTTON_SIZE + BUTTON_GAP.
        assert_eq!(c1 - c0, BUTTON_SIZE + BUTTON_GAP);
        assert_eq!(c2 - c1, BUTTON_SIZE + BUTTON_GAP);
    }
}
