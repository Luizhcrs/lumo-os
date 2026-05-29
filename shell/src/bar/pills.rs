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
    _canvas: &mut PixmapMut,
    _x: f32,
    _y: f32,
    _w: f32,
    _h: f32,
    _bg: Color,
    _shadow_alpha: u8,
) {
    // SEM BG/BORDA (pedido Luiz): pills nao pintam fundo nem sombra. So o
    // texto/icones desenhados em paint_frame, direto sobre o preto do topo.
    let _ = (PILL_RADIUS, rgba_hex(0, 0));
    let _ = fill_rrect;
}
