//! bar/dropdowns/lumo_menu.rs - Menu Lumo aberto pelo click no brand
//! "Lumo" da pill esquerda. Items definidos em tokens::MENU_LUMO_ITEMS.
//!
//! Render via crate::menu::draw_menu (modulo compartilhado com lumo-desktop)
//! com callbacks fill_rrect + draw_text locais.

use lumo_foundation::LumoColors;
use tiny_skia::PixmapMut;

use crate::bar::fonts::draw_text;
use crate::bar::icons::fill_rrect;
use crate::bar::tokens::{MENU_LUMO_ITEMS, MENU_LUMO_W};
use crate::menu;

/// Pinta o menu Lumo em (mx, my) com hover_idx (usize::MAX = sem hover).
pub fn draw_lumo_menu(
    canvas: &mut PixmapMut,
    mx: f32,
    my: f32,
    palette: &LumoColors,
    hover_idx: usize,
) {
    menu::draw_menu(
        canvas,
        mx,
        my,
        MENU_LUMO_W,
        MENU_LUMO_ITEMS,
        hover_idx,
        palette,
        fill_rrect,
        |c, x, y, label, size, color| {
            draw_text(c, x, y, label, size, color, false);
        },
    );
}
