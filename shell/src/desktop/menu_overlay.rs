//! desktop/menu_overlay.rs - Render do menu contextual right-click.
//!
//! Items MVP (A27): trocar wallpaper / sobre / atualizar / store. Render
//! compartilhado com lumo-bar via `crate::menu`.

use lumo_foundation::LumoColors;
use tiny_skia::PixmapMut;

use crate::desktop::state::{draw_text, fill_rrect, MenuActive, MENU_OFFSET};
use crate::menu;

/// Largura do menu desktop (vem do modulo compartilhado).
pub const MENU_W: f32 = menu::MENU_W_DESKTOP;

/// Items do menu desktop Lumo.
///
/// A27: items MVP (futuro: despachar comandos reais wallpaper picker / About
/// dialog / lumo-store launch via IPC).
pub const MENU_ITEMS: &[menu::MenuItem] = &[
    menu::MenuItem::action("Criar pasta"),
    menu::MenuItem::separator(),
    menu::MenuItem::action("Trocar wallpaper"),
    menu::MenuItem::action("Sobre este Galaxy Book"),
    menu::MenuItem::separator(),
    menu::MenuItem::action("Atualizar Lumo"),
    menu::MenuItem::action("Lumo Store"),
];

/// Indice do item "Criar pasta" no MENU_ITEMS.
pub const MENU_ITEM_CREATE_FOLDER: usize = 0;

pub fn paint_menu_at(
    canvas: &mut PixmapMut,
    menu_active: MenuActive,
    surf_w: u32,
    surf_h: u32,
    palette: &LumoColors,
) {
    let (mx, my) = menu::clamp_menu_origin(
        MENU_ITEMS,
        menu_active.x,
        menu_active.y,
        MENU_W,
        surf_w,
        surf_h,
        MENU_OFFSET,
    );

    menu::draw_menu(
        canvas,
        mx,
        my,
        MENU_W,
        MENU_ITEMS,
        menu_active.hover_idx,
        palette,
        |c, x, y, w, h, r, color| fill_rrect(c, x, y, w, h, r, color),
        |c, x, y, label, size, color| draw_text(c, x, y, label, size, color),
    );
}

/// Handler de click em item do menu.
/// Retorna true se acao e "Criar pasta" (caller deve invocar icons.create_folder()).
pub fn handle_item(idx: usize) -> bool {
    if idx == MENU_ITEM_CREATE_FOLDER {
        eprintln!("[lumo-desktop] menu: Criar pasta");
        return true;
    }
    if let Some(item) = MENU_ITEMS.get(idx) {
        eprintln!("[lumo-desktop] menu item: '{}' (stub)", item.label);
    }
    false
}
