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
