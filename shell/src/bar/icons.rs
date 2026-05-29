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

/// Path de rrect com raios independentes topo/baixo (rt = cantos de cima,
/// rb = cantos de baixo). Usado pela bar colada: topo reto (rt=0), base
/// curva (rb>0).
fn rrect_tb_path(x: f32, y: f32, w: f32, h: f32, rt: f32, rb: f32) -> Option<tiny_skia::Path> {
    let x = x.round();
    let y = y.round();
    let half = (w / 2.0).min(h / 2.0);
    let rt = rt.clamp(0.0, half);
    let rb = rb.clamp(0.0, half);
    let mut pb = PathBuilder::new();
    pb.move_to(x + rt, y);
    pb.line_to(x + w - rt, y);
    if rt > 0.0 {
        pb.quad_to(x + w, y, x + w, y + rt);
    } else {
        pb.line_to(x + w, y);
    }
    pb.line_to(x + w, y + h - rb);
    if rb > 0.0 {
        pb.quad_to(x + w, y + h, x + w - rb, y + h);
    } else {
        pb.line_to(x + w, y + h);
    }
    pb.line_to(x + rb, y + h);
    if rb > 0.0 {
        pb.quad_to(x, y + h, x, y + h - rb);
    } else {
        pb.line_to(x, y + h);
    }
    pb.line_to(x, y + rt);
    if rt > 0.0 {
        pb.quad_to(x, y, x + rt, y);
    } else {
        pb.line_to(x, y);
    }
    pb.close();
    pb.finish()
}

/// fill rrect com raios independentes topo/baixo.
pub fn fill_rrect_tb(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rt: f32,
    rb: f32,
    color: Color,
) {
    let Some(path) = rrect_tb_path(x, y, w, h, rt, rb) else {
        return;
    };
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
}

/// stroke rrect com raios independentes topo/baixo.
pub fn stroke_rrect_tb(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    rt: f32,
    rb: f32,
    color: Color,
    sw: f32,
) {
    let Some(path) = rrect_tb_path(x, y, w, h, rt, rb) else {
        return;
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
    let p0 = (
        cx + r * to_rad(start_deg).cos(),
        cy + r * to_rad(start_deg).sin(),
    );
    let p1 = (
        cx + r * to_rad(end_deg).cos(),
        cy + r * to_rad(end_deg).sin(),
    );
    let mid = (start_deg + end_deg) * 0.5;
    let delta = (end_deg - start_deg).abs().to_radians();
    let k = ((delta / 2.0).cos()).max(0.0001);
    let r_ctl = r / k;
    let ctrl = (
        cx + r_ctl * to_rad(mid).cos(),
        cy + r_ctl * to_rad(mid).sin(),
    );

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

/// W38: stroke rrect com round cap/join (One UI soft corners). O stroke_rrect
/// existente nao seta line_join, deixando cantos secos (miter). Esta variante
/// usa LineJoin::Round pro look squircle/friendly da familia de icones.
fn stroke_rrect_round(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    color: Color,
    sw: f32,
) {
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
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        ..Default::default()
    };
    canvas.stroke_path(&path, &p, &st, Transform::identity(), None);
}

// ============================================================
// Wifi glyph (compact 16px).
// ============================================================
pub fn draw_wifi(canvas: &mut PixmapMut, x: f32, y: f32, on: bool, fg: Color, fg_subtle: Color) {
    // W38 One UI: leque (setor de disco) solido apontando pra cima, vertice
    // arredondado. Span 90 graus (+-45 da vertical), boca ~1.15:1 vs altura.
    // Fill solido + stroke round por cima (squircle friendly), nao 3 arcos.
    let color = if on { fg } else { fg_subtle };
    let s = WIFI_SIZE;
    let cx = (x + s / 2.0).round() + 0.5; // meio-pixel = simetria nitida
    let apex_y = y + s * 0.80; // vertice perto da base do box
    let reach = s * 0.56; // raio do leque (vertice -> topo)

    let half = 45.0_f32.to_radians();
    let sin = half.sin();
    let cos = half.cos();
    let lx = cx - reach * sin;
    let rx = cx + reach * sin;
    let top_y = apex_y - reach * cos;
    // Controle do quad eleva pra casar a curvatura do circulo de raio reach.
    let ctl_y = apex_y - reach / cos;

    let mut pb = PathBuilder::new();
    pb.move_to(cx, apex_y); // vertice
    pb.line_to(lx, top_y); // borda esquerda
    pb.quad_to(cx, ctl_y, rx, top_y); // arco superior concavo
    pb.line_to(cx, apex_y); // borda direita de volta ao vertice
    pb.close();

    if let Some(path) = pb.finish() {
        let mut p = Paint::default();
        p.set_color(color);
        p.anti_alias = true;
        canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
        let st = Stroke {
            width: s * 0.085,
            line_cap: tiny_skia::LineCap::Round,
            line_join: tiny_skia::LineJoin::Round,
            ..Default::default()
        };
        canvas.stroke_path(&path, &p, &st, Transform::identity(), None);
    }
}

// ============================================================
// Battery glyph (compact 22x11 body Mac-style).
// ============================================================
pub fn draw_battery(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    pct: u8,
    charging: bool,
    fg: Color,
    accent: Color,
) {
    // W38 One UI: pilula horizontal. Corpo full-round (r = h/2), outline
    // encorpado com round join. Nub arredondado fora do corpo. Fill de nivel
    // inset com gap ~1px. Cores de nivel preservadas + bolt charging.
    let body_w = BAT_BODY_W;
    let body_h = BAT_BODY_H;
    let sw = 1.4f32; // stroke da familia de icones
    let half = sw / 2.0;
    // Corpo (inset de meio-stroke pra borda nao vazar do box).
    let bx = x + half;
    let by = y + half;
    let bw = body_w - sw;
    let bh = body_h - sw;
    let r_out = bh / 2.0; // full-round => pilula
    stroke_rrect_round(canvas, bx, by, bw, bh, r_out, fg, sw);

    // Nub (polo +): mamilo arredondado a direita, ~42% da altura, FORA do corpo
    // (encostado na cap curva sem flutuar).
    let nub_w = 1.6f32;
    let nub_h = body_h * 0.42;
    fill_rrect(
        canvas,
        x + body_w + 0.6,
        y + (body_h - nub_h) / 2.0,
        nub_w,
        nub_h,
        nub_w / 2.0,
        fg,
    );

    // Fill de carga: rrect inset (stroke + gap ~1px) do interior, mesmo raio.
    let gap = 1.0f32;
    let inset = half + gap;
    let inner_x = x + inset;
    let inner_y = y + inset;
    let inner_w = body_w - inset * 2.0;
    let inner_h = body_h - inset * 2.0;
    let r_in = (inner_h / 2.0).max(0.0);
    let fw = (pct as f32 / 100.0).clamp(0.0, 1.0) * inner_w;
    if fw > 0.8 {
        let fill_color = if pct >= 50 {
            opaque(0xF5F5F7) // claro cheio
        } else if pct >= 20 {
            opaque(0xFB923C) // laranja medio
        } else {
            opaque(0xEF4444) // vermelho baixo
        };
        let _ = accent;
        // Clamp anti-colapso: fw nunca menor que o diametro do raio (senao o
        // fill_rrect vira losango minusculo em pct muito baixo).
        fill_rrect(
            canvas,
            inner_x,
            inner_y,
            fw.max(r_in * 2.0),
            inner_h,
            r_in,
            fill_color,
        );
    }

    // Bolt charging centralizado, branco pra contraste com qualquer fill.
    if charging {
        draw_bolt(
            canvas,
            x + body_w / 2.0,
            y + body_h / 2.0,
            4.2,
            6.4,
            opaque(0xFFFFFF),
        );
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
    // W38 One UI: miolo solido gordo + 8 raios curtos grossos com round cap.
    // pct modula alpha (dim quando baixo). 1:1 radialmente simetrico.
    let alpha = ((pct as f32 / 100.0) * 0.65 + 0.35).clamp(0.0, 1.0);
    let mut c = color;
    c.set_alpha(alpha);

    // Centro INTEIRO: fill_circle re-arredonda o centro pra inteiro internamente.
    // Se passasse .5 aqui, o disco viraria inteiro mas os raios (stroke_path, sem
    // rounding) ficariam em .5 -> disco e raios 0.5px fora de fase. Inteiro nos
    // dois mantem disco e raios alinhados.
    let cx = cx.round();
    let cy = cy.round();

    // Miolo solido (~40% do box de 16 -> r ~3.2).
    let disc_r = 3.2f32;
    fill_circle(canvas, cx, cy, disc_r, c);

    // 8 raios a cada 45deg. Curtos: gap ~1.4 da borda, comprimento ~2.3.
    let r_in = disc_r + 1.4;
    let r_out = r_in + 2.3;
    let mut pb = PathBuilder::new();
    let mut a = 0.0f32;
    while a < 360.0 {
        let rad = a.to_radians();
        let (s, co) = rad.sin_cos();
        pb.move_to(cx + co * r_in, cy + s * r_in);
        pb.line_to(cx + co * r_out, cy + s * r_out);
        a += 45.0;
    }
    if let Some(path) = pb.finish() {
        let mut p = Paint::default();
        p.set_color(c);
        p.anti_alias = true;
        let stroke = Stroke {
            width: 1.4,
            line_cap: tiny_skia::LineCap::Round,
            line_join: tiny_skia::LineJoin::Round,
            ..Default::default()
        };
        canvas.stroke_path(&path, &p, &stroke, Transform::identity(), None);
    }
}
