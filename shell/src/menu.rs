// Cada bin compila este modulo via `#[path = "../menu.rs"] mod menu;`,
// entao itens usados so por um dos bins viram dead_code no outro.
// `allow(dead_code)` evita ruido nos builds.
#![allow(dead_code)]

//! Menu compartilhado entre lumo-bar (menu Lumo) e lumo-desktop (right-click).
//!
//! A27: extraido como modulo unico pra evitar duplicacao (memory
//! feedback_lumo_arquitetura_clean: modulos por feature). Inclui modelo +
//! layout + render via callbacks (caller injeta `draw_text_fn` e
//! `fill_rrect_fn` pra preservar o FontSystem/SwashCache local de cada bin).
//!
//! Estilo (foto sistema desktop context menu):
//!   - Hover pill solido accent (sem glow — memory feedback_zero_neon_glow).
//!   - Separator linha 1px border entre grupos.
//!   - Suffix "..." vem ja na string (renderiza como esta).
//!   - Padding interno generoso pra parecer Lumo-grade.
//!   - Font 13px regular.

use lumo_foundation::LumoColors;
use tiny_skia::{Color, PixmapMut};

// ============================================================
// Layout constants (justificativa em memory feedback_design_lapidado).
// ============================================================

/// Largura padrao menu desktop. 220 = cabe textos PT-BR ate ~24ch com pad lateral.
pub const MENU_W_DESKTOP: f32 = 220.0;
/// Largura menu Lumo na bar. 240 = "Preferencias do Sistema..." +
/// "Sobre este Galaxy Book..." (~30ch) cabem com folga.
pub const MENU_W_LUMO: f32 = 240.0;
/// Border-radius pill geral. 14 = identico PILL_RADIUS bar (continuidade A19.12).
pub const MENU_RADIUS: f32 = 14.0;
/// Padding vertical topo/base. 6 = compacto Lumo-grade, abertura/fecho discretos.
pub const MENU_PAD_Y: f32 = 6.0;
/// Padding horizontal interno (margem texto -> borda). 14 = igual PILL_PAD_X bar.
pub const MENU_PAD_X: f32 = 14.0;
/// Altura por row clicavel (Action ou Toggle). 28 = 13px font + respiro vertical 15px.
pub const MENU_ROW_H: f32 = 28.0;
/// Altura visual do bloco separator. 9 = 4 + 1 + 4 (margem-Y + linha + margem-Y).
pub const MENU_SEPARATOR_BLOCK_H: f32 = 9.0;
/// Margem lateral da linha separator (sai um pouco menos que o pad geral pra ficar leve).
pub const MENU_SEPARATOR_INSET_X: f32 = 10.0;
/// Border-radius do hover pill interno (sub-rrect). 6 = mais fechado que MENU_RADIUS 14.
pub const MENU_ROW_HOVER_RADIUS: f32 = 6.0;
/// Inset lateral do hover pill (4px de cada lado).
pub const MENU_ROW_HOVER_INSET: f32 = 4.0;
/// Font size dos labels. 13 = identico FONT_PILL bar (continuidade).
pub const FONT_MENU: f32 = 13.0;

// ============================================================
// Modelo.
// ============================================================

#[derive(Debug, Clone, Copy)]
pub enum MenuItemKind {
    Action,
    /// Toggle (futuro: prefix "v" no label quando on). MVP renderiza igual Action.
    #[allow(dead_code)]
    Toggle(bool),
    Separator,
}

#[derive(Debug, Clone, Copy)]
pub struct MenuItem {
    pub label: &'static str,
    pub kind: MenuItemKind,
}

impl MenuItem {
    pub const fn action(label: &'static str) -> Self {
        Self {
            label,
            kind: MenuItemKind::Action,
        }
    }
    pub const fn separator() -> Self {
        Self {
            label: "",
            kind: MenuItemKind::Separator,
        }
    }
    #[allow(dead_code)]
    pub const fn toggle(label: &'static str, on: bool) -> Self {
        Self {
            label,
            kind: MenuItemKind::Toggle(on),
        }
    }

    pub fn is_clickable(&self) -> bool {
        matches!(self.kind, MenuItemKind::Action | MenuItemKind::Toggle(_))
    }
}

// ============================================================
// Layout.
// ============================================================

/// Altura total exata pra um slice de items (inclui separators).
pub fn menu_height(items: &[MenuItem]) -> f32 {
    let mut h = MENU_PAD_Y * 2.0;
    for it in items {
        h += match it.kind {
            MenuItemKind::Separator => MENU_SEPARATOR_BLOCK_H,
            _ => MENU_ROW_H,
        };
    }
    h
}

/// Y-offset (relativo a y do menu, ANTES do MENU_PAD_Y) onde o item `idx`
/// comeca. Pra Action/Toggle = topo do row 28px. Pra Separator = topo do
/// bloco 9px.
pub fn item_y_offset(items: &[MenuItem], idx: usize) -> f32 {
    let mut acc = 0.0;
    for (i, it) in items.iter().enumerate() {
        if i == idx {
            return acc;
        }
        acc += match it.kind {
            MenuItemKind::Separator => MENU_SEPARATOR_BLOCK_H,
            _ => MENU_ROW_H,
        };
    }
    acc
}

/// Hit-test: pra (px, py) absolutos e origem (mx, my) + width w, retorna
/// indice do item clicavel sob o cursor, ou None se fora ou em separator.
pub fn hit_test(items: &[MenuItem], mx: f32, my: f32, w: f32, px: f32, py: f32) -> Option<usize> {
    let h = menu_height(items);
    if px < mx || px > mx + w || py < my || py > my + h {
        return None;
    }
    let py_rel = py - my - MENU_PAD_Y;
    if py_rel < 0.0 {
        return None;
    }
    let mut acc = 0.0;
    for (i, it) in items.iter().enumerate() {
        let block = match it.kind {
            MenuItemKind::Separator => MENU_SEPARATOR_BLOCK_H,
            _ => MENU_ROW_H,
        };
        if py_rel >= acc && py_rel < acc + block {
            if it.is_clickable() {
                return Some(i);
            }
            return None;
        }
        acc += block;
    }
    None
}

/// Clamp origem do menu pra caber dentro da surface (mexe X se overflow direita,
/// Y se overflow embaixo).
pub fn clamp_menu_origin(
    items: &[MenuItem],
    desired_x: f32,
    desired_y: f32,
    menu_w: f32,
    surf_w: u32,
    surf_h: u32,
    offset: f32,
) -> (f32, f32) {
    let mh = menu_height(items);
    let mut mx = desired_x + offset;
    let mut my = desired_y + offset;
    if mx + menu_w > surf_w as f32 {
        mx = (desired_x - menu_w - offset).max(0.0);
    }
    if my + mh > surf_h as f32 {
        my = (desired_y - mh - offset).max(0.0);
    }
    (mx, my)
}

// ============================================================
// Dynamic-label variant (submenu appmenu, labels nao-static).
// ============================================================

/// Versao dinamica de MenuItem (label nao-static). Usada pelo submenu
/// appmenu pill da bar (labels vem de dbusmenu IPC em runtime).
#[derive(Debug, Clone)]
pub struct DynMenuItem<'a> {
    pub label: &'a str,
    pub kind: MenuItemKind,
}

impl<'a> DynMenuItem<'a> {
    pub fn action(label: &'a str) -> Self {
        Self {
            label,
            kind: MenuItemKind::Action,
        }
    }
    pub fn separator() -> Self {
        Self {
            label: "",
            kind: MenuItemKind::Separator,
        }
    }
    pub fn is_clickable(&self) -> bool {
        matches!(self.kind, MenuItemKind::Action | MenuItemKind::Toggle(_))
    }
}

/// Altura total de um slice dynamic.
pub fn menu_height_dyn(items: &[DynMenuItem<'_>]) -> f32 {
    let mut h = MENU_PAD_Y * 2.0;
    for it in items {
        h += match it.kind {
            MenuItemKind::Separator => MENU_SEPARATOR_BLOCK_H,
            _ => MENU_ROW_H,
        };
    }
    h
}

/// Hit-test dynamic.
pub fn hit_test_dyn(
    items: &[DynMenuItem<'_>],
    mx: f32,
    my: f32,
    w: f32,
    px: f32,
    py: f32,
) -> Option<usize> {
    let h = menu_height_dyn(items);
    if px < mx || px > mx + w || py < my || py > my + h {
        return None;
    }
    let py_rel = py - my - MENU_PAD_Y;
    if py_rel < 0.0 {
        return None;
    }
    let mut acc = 0.0;
    for (i, it) in items.iter().enumerate() {
        let block = match it.kind {
            MenuItemKind::Separator => MENU_SEPARATOR_BLOCK_H,
            _ => MENU_ROW_H,
        };
        if py_rel >= acc && py_rel < acc + block {
            if it.is_clickable() {
                return Some(i);
            }
            return None;
        }
        acc += block;
    }
    None
}

// ============================================================
// Render via callbacks.
// ============================================================
//
// Caller passa `draw_text_fn(canvas, x, y, label, size, color)` e
// `fill_rrect_fn(canvas, x, y, w, h, r, color)`. Isso evita arrastar o
// FontSystem/SwashCache statico de cada bin pra dentro do modulo.

/// Helper RGBA hex -> tiny_skia::Color (igual usado nos bins).
pub fn rgba_hex(hex: u32, alpha: u8) -> Color {
    let r = ((hex >> 16) & 0xff) as f32 / 255.0;
    let g = ((hex >> 8) & 0xff) as f32 / 255.0;
    let b = (hex & 0xff) as f32 / 255.0;
    let a = alpha as f32 / 255.0;
    Color::from_rgba(r, g, b, a).expect("r,g,b,a derivados de u8: sempre em [0,1]")
}

pub fn opaque(hex: u32) -> Color {
    rgba_hex(hex, 0xff)
}

/// Versao dynamic (label runtime): identica a draw_menu mas com slice
/// de DynMenuItem<'a>. Reusada pelo submenu appmenu pill da bar.
#[allow(clippy::too_many_arguments)]
pub fn draw_menu_dyn<TextFn, RrectFn>(
    canvas: &mut PixmapMut,
    mx: f32,
    my: f32,
    w: f32,
    items: &[DynMenuItem<'_>],
    hover_idx: usize,
    palette: &LumoColors,
    mut fill_rrect_fn: RrectFn,
    mut draw_text_fn: TextFn,
) where
    TextFn: FnMut(&mut PixmapMut, f32, f32, &str, f32, Color),
    RrectFn: FnMut(&mut PixmapMut, f32, f32, f32, f32, f32, Color),
{
    let mh = menu_height_dyn(items);
    let bg = rgba_hex(palette.pill_bg, palette.pill_bg_alpha);
    fill_rrect_fn(canvas, mx, my, w, mh, MENU_RADIUS, bg);
    let fg = opaque(palette.pill_fg);
    let hover_bg = opaque(palette.accent);
    let hover_fg = opaque(0xffffff);
    let sep_color = rgba_hex(palette.border, 0xff);
    let mut cur_y = my + MENU_PAD_Y;
    for (i, it) in items.iter().enumerate() {
        match it.kind {
            MenuItemKind::Separator => {
                let line_y = (cur_y + 4.0).round();
                fill_rrect_fn(
                    canvas,
                    mx + MENU_SEPARATOR_INSET_X,
                    line_y,
                    w - MENU_SEPARATOR_INSET_X * 2.0,
                    1.0,
                    0.5,
                    sep_color,
                );
                cur_y += MENU_SEPARATOR_BLOCK_H;
            }
            MenuItemKind::Action | MenuItemKind::Toggle(_) => {
                let is_hover = i == hover_idx;
                if is_hover {
                    fill_rrect_fn(
                        canvas,
                        mx + MENU_ROW_HOVER_INSET,
                        cur_y + 1.0,
                        w - MENU_ROW_HOVER_INSET * 2.0,
                        MENU_ROW_H - 2.0,
                        MENU_ROW_HOVER_RADIUS,
                        hover_bg,
                    );
                }
                let text_color = if is_hover { hover_fg } else { fg };
                let text_y = (cur_y + (MENU_ROW_H - FONT_MENU * 1.4) / 2.0).round();
                draw_text_fn(canvas, mx + MENU_PAD_X, text_y, it.label, FONT_MENU, text_color);
                cur_y += MENU_ROW_H;
            }
        }
    }
}

/// Pinta o menu inteiro. `draw_text_fn(canvas, x, y, label, size, color)`,
/// `fill_rrect_fn(canvas, x, y, w, h, r, color)`.
///
/// `hover_idx`: indice do item Action/Toggle em hover. Se nao houver,
/// passa `usize::MAX` ou qualquer fora de range.
#[allow(clippy::too_many_arguments)]
pub fn draw_menu<TextFn, RrectFn>(
    canvas: &mut PixmapMut,
    mx: f32,
    my: f32,
    w: f32,
    items: &[MenuItem],
    hover_idx: usize,
    palette: &LumoColors,
    mut fill_rrect_fn: RrectFn,
    mut draw_text_fn: TextFn,
) where
    TextFn: FnMut(&mut PixmapMut, f32, f32, &str, f32, Color),
    RrectFn: FnMut(&mut PixmapMut, f32, f32, f32, f32, f32, Color),
{
    let mh = menu_height(items);

    // Background pill (cor + alpha do tema, igual pill bar A19.15).
    let bg = rgba_hex(palette.pill_bg, palette.pill_bg_alpha);
    fill_rrect_fn(canvas, mx, my, w, mh, MENU_RADIUS, bg);

    let fg = opaque(palette.pill_fg);
    // Hover SOLIDO accent (memory feedback_zero_neon_glow: zero glow).
    let hover_bg = opaque(palette.accent);
    let hover_fg = opaque(0xffffff);
    let sep_color = rgba_hex(palette.border, 0xff);

    let mut cur_y = my + MENU_PAD_Y;
    for (i, it) in items.iter().enumerate() {
        match it.kind {
            MenuItemKind::Separator => {
                // Linha 1px no meio do bloco 9 (4 margem + 1 linha + 4 margem).
                let line_y = (cur_y + 4.0).round();
                fill_rrect_fn(
                    canvas,
                    mx + MENU_SEPARATOR_INSET_X,
                    line_y,
                    w - MENU_SEPARATOR_INSET_X * 2.0,
                    1.0,
                    0.5,
                    sep_color,
                );
                cur_y += MENU_SEPARATOR_BLOCK_H;
            }
            MenuItemKind::Action | MenuItemKind::Toggle(_) => {
                let is_hover = i == hover_idx;
                if is_hover {
                    fill_rrect_fn(
                        canvas,
                        mx + MENU_ROW_HOVER_INSET,
                        cur_y + 1.0,
                        w - MENU_ROW_HOVER_INSET * 2.0,
                        MENU_ROW_H - 2.0,
                        MENU_ROW_HOVER_RADIUS,
                        hover_bg,
                    );
                }
                let text_color = if is_hover { hover_fg } else { fg };
                // Centralizado vertical: row_h 28, font 13 => baseline aprox row_h/2 - font*0.4
                let text_y = (cur_y + (MENU_ROW_H - FONT_MENU * 1.4) / 2.0).round();

                // Toggle on -> prefixo "v " (marker ASCII, memory feedback_zero_emoji).
                let label_owned = if let MenuItemKind::Toggle(true) = it.kind {
                    format!("v  {}", it.label)
                } else if let MenuItemKind::Toggle(false) = it.kind {
                    format!("   {}", it.label)
                } else {
                    it.label.to_string()
                };
                draw_text_fn(
                    canvas,
                    mx + MENU_PAD_X,
                    text_y,
                    &label_owned,
                    FONT_MENU,
                    text_color,
                );
                cur_y += MENU_ROW_H;
            }
        }
    }
}

// ============================================================
// Tests.
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn w37_6_menu_height_dyn_action_e_separator() {
        let items = vec![
            DynMenuItem::action("Arquivo"),
            DynMenuItem::separator(),
            DynMenuItem::action("Editar"),
        ];
        let h = menu_height_dyn(&items);
        // pad(6*2) + row(28) + sep(9) + row(28) = 12 + 28 + 9 + 28 = 77
        assert_eq!(h, MENU_PAD_Y * 2.0 + MENU_ROW_H * 2.0 + MENU_SEPARATOR_BLOCK_H);
    }

    #[test]
    fn w37_6_hit_test_dyn_skip_separator() {
        let items = vec![
            DynMenuItem::action("A"),
            DynMenuItem::separator(),
            DynMenuItem::action("B"),
        ];
        // (10, 10) origem; w 200.
        // Item 0: y rel 0..28 -> abs 16..44
        // Sep: y rel 28..37 -> abs 44..53
        // Item 1: y rel 37..65 -> abs 53..81
        assert_eq!(hit_test_dyn(&items, 10.0, 10.0, 200.0, 50.0, 30.0), Some(0));
        // hit no separator -> None
        assert_eq!(hit_test_dyn(&items, 10.0, 10.0, 200.0, 50.0, 48.0), None);
        // hit no item 1
        assert_eq!(hit_test_dyn(&items, 10.0, 10.0, 200.0, 50.0, 60.0), Some(2));
    }

    #[test]
    fn w37_6_hit_test_dyn_fora_e_none() {
        let items = vec![DynMenuItem::action("A")];
        // px < mx
        assert_eq!(hit_test_dyn(&items, 100.0, 100.0, 200.0, 50.0, 105.0), None);
        // py < my
        assert_eq!(hit_test_dyn(&items, 100.0, 100.0, 200.0, 150.0, 90.0), None);
    }

    #[test]
    fn w37_6_menu_radius_e_padroes_unificados() {
        // Confirma constantes que driving identidade visual unica.
        assert_eq!(MENU_RADIUS, 14.0);
        assert_eq!(MENU_ROW_H, 28.0);
        assert_eq!(FONT_MENU, 13.0);
        assert_eq!(MENU_ROW_HOVER_RADIUS, 6.0);
        assert_eq!(MENU_PAD_X, 14.0);
    }

    // --- W37.21 +tests: static fns nao cobertos ---

    fn menu_static(items: &[(&'static str, MenuItemKind)]) -> Vec<MenuItem> {
        items
            .iter()
            .map(|(l, k)| MenuItem {
                label: l,
                kind: *k,
            })
            .collect()
    }

    #[test]
    fn menu_height_static_matches_dyn() {
        let static_items =
            menu_static(&[("A", MenuItemKind::Action), ("B", MenuItemKind::Action)]);
        let dyn_items = vec![DynMenuItem::action("A"), DynMenuItem::action("B")];
        assert_eq!(menu_height(&static_items), menu_height_dyn(&dyn_items));
    }

    #[test]
    fn item_y_offset_action_only() {
        let items = menu_static(&[
            ("A", MenuItemKind::Action),
            ("B", MenuItemKind::Action),
            ("C", MenuItemKind::Action),
        ]);
        assert_eq!(item_y_offset(&items, 0), 0.0);
        assert_eq!(item_y_offset(&items, 1), MENU_ROW_H);
        assert_eq!(item_y_offset(&items, 2), MENU_ROW_H * 2.0);
    }

    #[test]
    fn item_y_offset_with_separator() {
        let items = menu_static(&[
            ("A", MenuItemKind::Action),
            ("", MenuItemKind::Separator),
            ("B", MenuItemKind::Action),
        ]);
        assert_eq!(item_y_offset(&items, 2), MENU_ROW_H + MENU_SEPARATOR_BLOCK_H);
    }

    #[test]
    fn hit_test_static_skip_separator() {
        let items = menu_static(&[
            ("A", MenuItemKind::Action),
            ("", MenuItemKind::Separator),
            ("B", MenuItemKind::Action),
        ]);
        // py em area do separator deve retornar None.
        let sep_y = 10.0 + MENU_PAD_Y + MENU_ROW_H + 2.0;
        assert_eq!(hit_test(&items, 10.0, 10.0, 200.0, 50.0, sep_y), None);
        // py em item 0
        let item0_y = 10.0 + MENU_PAD_Y + 5.0;
        assert_eq!(hit_test(&items, 10.0, 10.0, 200.0, 50.0, item0_y), Some(0));
    }

    #[test]
    fn clamp_origin_inside_screen_no_change() {
        let items = menu_static(&[("A", MenuItemKind::Action)]);
        let (x, y) = clamp_menu_origin(&items, 100.0, 100.0, 200.0, 1920, 1080, 0.0);
        assert_eq!(x, 100.0);
        assert_eq!(y, 100.0);
    }

    #[test]
    fn clamp_origin_overflow_right_flips() {
        let items = menu_static(&[("A", MenuItemKind::Action)]);
        // desired_x=1850 + width 200 = 2050 > 1920 -> flip esq.
        let (x, _) = clamp_menu_origin(&items, 1850.0, 100.0, 200.0, 1920, 1080, 0.0);
        // desired_x - width = 1650
        assert_eq!(x, 1650.0);
    }

    #[test]
    fn clamp_origin_overflow_bottom_flips() {
        let items = menu_static(&[
            ("A", MenuItemKind::Action),
            ("B", MenuItemKind::Action),
            ("C", MenuItemKind::Action),
        ]);
        let mh = menu_height(&items);
        // desired_y muito proximo da base -> flip pra cima.
        let (_, y) = clamp_menu_origin(&items, 100.0, 1070.0, 200.0, 1920, 1080, 0.0);
        // Espera y < 1070 (flipped)
        assert!(y < 1070.0);
        let _ = mh;
    }

    #[test]
    fn rgba_hex_full_components() {
        let c = rgba_hex(0x112233, 0xFF);
        // tiny_skia Color usa f32 [0,1]; verifica reverso pra u8.
        // r=0x11, g=0x22, b=0x33, a=0xFF
        assert!((c.red() - 0x11 as f32 / 255.0).abs() < 1e-4);
        assert!((c.green() - 0x22 as f32 / 255.0).abs() < 1e-4);
        assert!((c.blue() - 0x33 as f32 / 255.0).abs() < 1e-4);
        assert!((c.alpha() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn opaque_helper_returns_full_alpha() {
        let c = opaque(0xFF0000);
        assert!((c.alpha() - 1.0).abs() < 1e-4);
        assert!((c.red() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn menu_item_constructors() {
        let act = MenuItem::action("Open");
        assert!(act.is_clickable());
        assert_eq!(act.label, "Open");

        let sep = MenuItem::separator();
        assert!(!sep.is_clickable());

        let tog_on = MenuItem::toggle("Bold", true);
        assert!(tog_on.is_clickable());
    }

    #[test]
    fn menu_height_empty_is_pad_only() {
        let items: Vec<MenuItem> = vec![];
        assert_eq!(menu_height(&items), MENU_PAD_Y * 2.0);
    }
}
