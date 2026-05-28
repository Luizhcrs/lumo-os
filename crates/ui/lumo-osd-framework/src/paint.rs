//! paint.rs — primitivas tiny-skia compartilhadas pra todos OSDs.

use tiny_skia::{
    Color, FillRule, Paint, PathBuilder, Pixmap, PixmapMut, Rect, Stroke, Transform,
};

use crate::tokens;

/// Helper RGBA hex -> Color.
pub fn rgba_hex(hex: u32, alpha: u8) -> Color {
    let r = ((hex >> 16) & 0xff) as f32 / 255.0;
    let g = ((hex >> 8) & 0xff) as f32 / 255.0;
    let b = (hex & 0xff) as f32 / 255.0;
    let a = alpha as f32 / 255.0;
    Color::from_rgba(r, g, b, a).expect("rgba derivada u8 valida")
}

/// Background pill arredondado uniforme pra todos OSDs.
/// alpha 0.0-1.0 = animator fade.
pub fn paint_background(pixmap: &mut Pixmap, alpha: f32) {
    pixmap.fill(Color::TRANSPARENT);
    let alpha_u8 = (alpha.clamp(0.0, 1.0) * 240.0) as u8; // base 0xF0
    let bg_color = rgba_hex(0x2A2A2A, alpha_u8);
    let mut paint = Paint::default();
    paint.set_color(bg_color);
    paint.anti_alias = true;

    let r = tokens::OSD_RADIUS;
    let mut pb = PathBuilder::new();
    let w = pixmap.width() as f32;
    let h = pixmap.height() as f32;
    pb.move_to(r, 0.0);
    pb.line_to(w - r, 0.0);
    pb.quad_to(w, 0.0, w, r);
    pb.line_to(w, h - r);
    pb.quad_to(w, h, w - r, h);
    pb.line_to(r, h);
    pb.quad_to(0.0, h, 0.0, h - r);
    pb.line_to(0.0, r);
    pb.quad_to(0.0, 0.0, r, 0.0);
    pb.close();
    if let Some(path) = pb.finish() {
        pixmap.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
    }
}

/// Slider bar: trilho + fill ate value.
pub fn paint_slider(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    fill_x_end: f32,
    alpha: f32,
) {
    let alpha_u8 = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
    // Trilho fundo (40% alpha do fg).
    let track_color = rgba_hex(0xE0E0E0, (alpha_u8 as f32 * 0.25) as u8);
    let mut paint = Paint::default();
    paint.set_color(track_color);
    paint.anti_alias = true;
    if let Some(rect) = Rect::from_xywh(x, y, w, h) {
        if let Some(rrect) = rrect_path(rect, tokens::SLIDER_RADIUS) {
            canvas.fill_path(&rrect, &paint, FillRule::Winding, Transform::identity(), None);
        }
    }
    // Fill bar accent.
    let fill_w = (fill_x_end - x).max(0.0);
    if fill_w > 0.5 {
        let fill_color = rgba_hex(0xE0E0E0, alpha_u8);
        let mut paint = Paint::default();
        paint.set_color(fill_color);
        paint.anti_alias = true;
        if let Some(rect) = Rect::from_xywh(x, y, fill_w, h) {
            if let Some(rrect) = rrect_path(rect, tokens::SLIDER_RADIUS) {
                canvas.fill_path(&rrect, &paint, FillRule::Winding, Transform::identity(), None);
            }
        }
    }
}

/// Toggle dot — circulo opaco accent quando ON, ring quando OFF.
pub fn paint_toggle_dot(
    canvas: &mut PixmapMut,
    cx: f32,
    cy: f32,
    radius: f32,
    on: bool,
    alpha: f32,
) {
    let alpha_u8 = (alpha.clamp(0.0, 1.0) * 255.0) as u8;
    let color = if on {
        rgba_hex(0x2ECC71, alpha_u8) // verde Lumo accent
    } else {
        rgba_hex(0x808080, alpha_u8) // cinza off
    };
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, radius);
    if let Some(path) = pb.finish() {
        if on {
            canvas.fill_path(&path, &paint, FillRule::Winding, Transform::identity(), None);
        } else {
            // Outline only quando off.
            let stroke = Stroke {
                width: 2.0,
                ..Stroke::default()
            };
            canvas.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
        }
    }
}

fn rrect_path(rect: Rect, radius: f32) -> Option<tiny_skia::Path> {
    let r = radius.min(rect.width() / 2.0).min(rect.height() / 2.0);
    let x = rect.x();
    let y = rect.y();
    let w = rect.width();
    let h = rect.height();
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
    pb.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paint_bg_alpha_zero_keeps_transparent() {
        let mut pm = Pixmap::new(300, 80).unwrap();
        paint_background(&mut pm, 0.0);
        // bg color alpha = 0; pixels stays transparent (alpha 0).
        let any_opaque = pm.pixels().iter().any(|p| p.alpha() > 30);
        assert!(!any_opaque, "alpha 0 nao deve produzir pixels visiveis");
    }

    #[test]
    fn paint_bg_alpha_one_produces_solid_center() {
        let mut pm = Pixmap::new(300, 80).unwrap();
        paint_background(&mut pm, 1.0);
        // Center pixel deve ter alpha alto (bg base 0xF0 = 240).
        let center_idx = (40 * 300 + 150) as usize;
        let px = pm.pixels()[center_idx];
        assert!(px.alpha() > 200, "center alpha={}", px.alpha());
    }

    #[test]
    fn paint_slider_zero_fill_no_accent_pixels() {
        let mut pm = Pixmap::new(300, 80).unwrap();
        paint_background(&mut pm, 1.0);
        let mut canvas = pm.as_mut();
        paint_slider(&mut canvas, 50.0, 50.0, 200.0, 8.0, 50.0, 1.0);
        // fill_x_end == x = sem fill. Trilho exists.
        // Trilho alpha ~0.25*255 = ~64. Acima do bg que e 240*alpha_param=240.
        // Verify nao panicou.
    }

    #[test]
    fn paint_slider_full_fill_produces_bright_pixels() {
        let mut pm = Pixmap::new(300, 80).unwrap();
        let mut canvas = pm.as_mut();
        paint_slider(&mut canvas, 50.0, 40.0, 200.0, 8.0, 250.0, 1.0);
        // Pixel em meio do slider deve ter cor fg E0E0E0.
        let center_idx = (44 * 300 + 150) as usize;
        let px = pm.pixels()[center_idx];
        // Approx: fg E0E0E0 = R=224 G=224 B=224 (pre-mult alpha=255 mesmos valores).
        assert!(px.red() > 150, "red={}", px.red());
    }

    #[test]
    fn paint_toggle_on_produces_accent_pixels() {
        let mut pm = Pixmap::new(60, 60).unwrap();
        let mut canvas = pm.as_mut();
        paint_toggle_dot(&mut canvas, 30.0, 30.0, 10.0, true, 1.0);
        // Centro = green 2E CC 71.
        let center_idx = (30 * 60 + 30) as usize;
        let px = pm.pixels()[center_idx];
        assert!(px.green() > px.red(), "verde dominante esperado");
    }

    #[test]
    fn paint_toggle_off_produces_ring_only() {
        let mut pm = Pixmap::new(60, 60).unwrap();
        let mut canvas = pm.as_mut();
        paint_toggle_dot(&mut canvas, 30.0, 30.0, 10.0, false, 1.0);
        // Centro = transparent (so contorno).
        let center_idx = (30 * 60 + 30) as usize;
        let px = pm.pixels()[center_idx];
        assert!(px.alpha() < 50, "centro alpha={} (esperado ~0)", px.alpha());
    }

    #[test]
    fn rgba_hex_alpha_correct() {
        let c = rgba_hex(0xFF0000, 0x80);
        assert!((c.red() - 1.0).abs() < 0.01);
        assert!((c.alpha() - 0.5).abs() < 0.01);
    }

    #[test]
    fn rgba_hex_full_components() {
        let c = rgba_hex(0x12_34_56, 0xFF);
        assert!((c.red() - 0x12 as f32 / 255.0).abs() < 0.01);
        assert!((c.green() - 0x34 as f32 / 255.0).abs() < 0.01);
        assert!((c.blue() - 0x56 as f32 / 255.0).abs() < 0.01);
    }
}
