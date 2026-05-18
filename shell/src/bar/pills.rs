//! bar/pills.rs - Pill primitive (background com sombra 4-layer) +
//! render das duas pills (esquerda Lumo+workspace, direita bat+wifi+clock).
//!
//! `draw_pill_bg` pinta:
//!   1) sombra: 4 rrects empilhados (y offset 1..4), alpha decrescente
//!      (simula blur 4px sem shader GPU).
//!   2) pill bg fill rounded com cor + alpha do tema.
//!
//! Sem accent glow (memory feedback_zero_neon_glow). Sombra preta neutra.

use tiny_skia::{Color, PixmapMut};

use crate::bar::fonts::rgba_hex;
use crate::bar::icons::fill_rrect;
use crate::bar::tokens::*;

pub fn draw_pill_bg(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    bg: Color,
    shadow_alpha: u8,
) {
    // Sombra: 4 rrects empilhados, offset y crescente, alpha decrescente.
    // Cada camada simula um "anel" do blur gaussiano discretizado.
    let base = shadow_alpha as f32;
    let layers: [(f32, f32, f32); 4] = [
        // (dy, dx_expand, alpha_factor)
        (1.0, 0.0, 1.0),   // mais perto, mais opaco
        (2.0, 0.5, 0.65),
        (3.0, 1.0, 0.35),
        (4.0, 1.5, 0.15),
    ];
    for (dy, expand, factor) in layers {
        let a = (base * factor).round().clamp(0.0, 255.0) as u8;
        if a == 0 {
            continue;
        }
        let shadow_color = rgba_hex(0x000000, a);
        fill_rrect(
            canvas,
            x - expand,
            y + dy,
            w + expand * 2.0,
            h,
            PILL_RADIUS,
            shadow_color,
        );
    }
    // Pill background.
    fill_rrect(canvas, x, y, w, h, PILL_RADIUS, bg);
}
