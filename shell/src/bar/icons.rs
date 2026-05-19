//! bar/icons.rs - Glyphs vetoriais (wifi, bateria, brand dot) + primitives
//! de path (fill_circle, fill_rrect, stroke_rrect, stroke_arc).
//!
//! Tudo desenhado via tiny-skia path builder. Sem glow neon (memory
//! feedback_zero_neon_glow). Cores e dimensoes vem de tokens.rs.

use tiny_skia::{Color, FillRule, Paint, PathBuilder, PixmapMut, Stroke, Transform};

use crate::bar::fonts::opaque;
use crate::bar::tokens::*;

// ============================================================
// Vector primitives.
// ============================================================

pub fn fill_circle(canvas: &mut PixmapMut, cx: f32, cy: f32, r: f32, color: Color) {
    let path = match PathBuilder::from_circle(cx.round(), cy.round(), r) {
        Some(p) => p,
        None => return,
    };
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
}

pub fn fill_rrect(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    let x = x.round();
    let y = y.round();
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    let path = match pb.finish() {
        Some(p) => p,
        None => return,
    };
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
}

pub fn stroke_rrect(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    color: Color,
    sw: f32,
) {
    let x = x.round();
    let y = y.round();
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r);
    pb.quad_to(x, y, x + r, y);
    pb.close();
    let path = match pb.finish() {
        Some(p) => p,
        None => return,
    };
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    let st = Stroke {
        width: sw,
        ..Default::default()
    };
    canvas.stroke_path(&path, &p, &st, Transform::identity(), None);
}

pub fn stroke_arc(
    canvas: &mut PixmapMut,
    cx: f32,
    cy: f32,
    r: f32,
    start_deg: f32,
    end_deg: f32,
    color: Color,
    sw: f32,
) {
    let to_rad = |d: f32| d.to_radians();
    let p0 = (cx + r * to_rad(start_deg).cos(), cy + r * to_rad(start_deg).sin());
    let p1 = (cx + r * to_rad(end_deg).cos(), cy + r * to_rad(end_deg).sin());
    let mid = (start_deg + end_deg) * 0.5;
    let delta = (end_deg - start_deg).abs().to_radians();
    let k = ((delta / 2.0).cos()).max(0.0001);
    let r_ctl = r / k;
    let ctrl = (cx + r_ctl * to_rad(mid).cos(), cy + r_ctl * to_rad(mid).sin());

    let mut pb = PathBuilder::new();
    pb.move_to(p0.0, p0.1);
    pb.quad_to(ctrl.0, ctrl.1, p1.0, p1.1);
    let path = match pb.finish() {
        Some(p) => p,
        None => return,
    };
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    let st = Stroke {
        width: sw,
        line_cap: tiny_skia::LineCap::Round,
        ..Default::default()
    };
    canvas.stroke_path(&path, &p, &st, Transform::identity(), None);
}

// ============================================================
// Wifi glyph (compact 16px).
// ============================================================
pub fn draw_wifi(canvas: &mut PixmapMut, x: f32, y: f32, on: bool, fg: Color, fg_subtle: Color) {
    let color = if on { fg } else { fg_subtle };
    let s = WIFI_SIZE;
    let cx = x + s / 2.0;
    let cy = y + s * 0.78;
    let arcs = [
        (s * 0.46, s * 0.085),
        (s * 0.30, s * 0.075),
        (s * 0.155, s * 0.07),
    ];
    for (radius, sw) in arcs {
        stroke_arc(canvas, cx, cy, radius, -135.0, -45.0, color, sw);
    }
    fill_circle(canvas, cx, cy, s * 0.06, color);
}

// ============================================================
// Battery glyph (compact 22x11 body Mac-style).
// ============================================================
pub fn draw_battery(canvas: &mut PixmapMut, x: f32, y: f32, pct: u8, charging: bool, fg: Color, accent: Color) {
    let body_w = BAT_BODY_W;
    let body_h = BAT_BODY_H;
    stroke_rrect(canvas, x + 0.5, y + 0.5, body_w - 1.0, body_h - 1.0, 2.2, fg, 1.2);
    fill_rrect(canvas, x + body_w + 0.8, y + body_h * 0.28, 2.0, body_h * 0.44, 0.8, fg);
    // A19.14: bateria Mac-style refinada (22x11 body, inset 2px = fill cheio e centralizado)
    let inset_x = 2.0f32;
    let inset_y = 2.0f32;
    let inner_w = body_w - inset_x * 2.0;
    let inner_h = body_h - inset_y * 2.0;
    let fw = (pct as f32 / 100.0).clamp(0.0, 1.0) * inner_w;
    if fw > 0.5 {
        let fill_color = if pct >= 50 {
            opaque(0xF5F5F7) // branco pearl Mac cheio
        } else if pct >= 20 {
            opaque(0xFB923C) // orange-400 medio
        } else {
            opaque(0xEF4444) // red-500 baixo
        };
        let _ = accent;
        fill_rrect(canvas, x + inset_x, y + inset_y, fw, inner_h, 1.2, fill_color);
    }
    // A30: bolt charging icone centralizado no body. Branco (#FFFFFF) pra contraste
    // com qualquer fill (verde/laranja/vermelho). ~6px altura, body inner 18x7.
    if charging {
        draw_bolt(canvas, x + body_w / 2.0, y + body_h / 2.0, 4.4, 6.6, opaque(0xFFFFFF));
    }
}

/// A30: raio (bolt) charging. Centralizado em (cx, cy), tamanho w x h.
/// Path 7-vertex zigzag Material flash_on.
pub fn draw_bolt(canvas: &mut PixmapMut, cx: f32, cy: f32, w: f32, h: f32, color: Color) {
    let x0 = cx - w / 2.0;
    let y0 = cy - h / 2.0;
    // Coords normalizadas (0..1) escaladas. Traversal contorno fechado.
    let pts: [(f32, f32); 7] = [
        (0.40, 0.00), // P0: topo levemente esquerda
        (0.85, 0.00), // P1: topo direita
        (0.50, 0.45), // P2: corte direita interno (meio)
        (0.95, 0.45), // P3: corte direita externo
        (0.60, 1.00), // P4: ponta inferior
        (0.15, 0.55), // P5: corte esquerda externo
        (0.50, 0.55), // P6: corte esquerda interno (meio)
    ];
    let mut pb = PathBuilder::new();
    let (ux, uy) = pts[0];
    pb.move_to(x0 + ux * w, y0 + uy * h);
    for &(ux, uy) in &pts[1..] {
        pb.line_to(x0 + ux * w, y0 + uy * h);
    }
    pb.close();
    let path = match pb.finish() {
        Some(p) => p,
        None => return,
    };
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
}

pub fn battery_total_width() -> f32 {
    BAT_BODY_W + 2.5
}

// ============================================================
// Brand dot (8px accent emerald/blue).
// ============================================================
pub fn draw_brand_dot(canvas: &mut PixmapMut, cx: f32, cy: f32, accent: Color) {
    fill_circle(canvas, cx, cy, BRAND_DOT_RADIUS, accent);
}

// ============================================================
// L5: Brightness sun icon (simple circle + rays).
// ============================================================
/// Draw a minimal sun icon: filled circle + 4 short ray strokes.
/// cx/cy = center. pct used for opacity hint (dim when low).
pub fn draw_brightness_sun(
    canvas: &mut PixmapMut,
    cx: f32,
    cy: f32,
    pct: u8,
    color: Color,
    _accent: Color,
) {
    use tiny_skia::{Paint, Stroke, PathBuilder, Transform};

    let alpha = ((pct as f32 / 100.0) * 0.7 + 0.3).clamp(0.0, 1.0);
    let mut c = color;
    c.set_alpha(alpha);

    // Inner circle radius 3.
    fill_circle(canvas, cx, cy, 3.0, c);

    // 4 rays (N/S/E/W), from r=5 to r=7.
    let offsets: [(f32, f32); 4] = [(0.0, -1.0), (0.0, 1.0), (-1.0, 0.0), (1.0, 0.0)];
    let mut pb = PathBuilder::new();
    for (dx, dy) in offsets {
        pb.move_to(cx + dx * 5.0, cy + dy * 5.0);
        pb.line_to(cx + dx * 7.0, cy + dy * 7.0);
    }
    if let Some(path) = pb.finish() {
        let mut p = Paint::default();
        p.set_color(c);
        p.anti_alias = true;
        let stroke = Stroke { width: 1.5, ..Default::default() };
        canvas.stroke_path(&path, &p, &stroke, Transform::identity(), None);
    }
}
