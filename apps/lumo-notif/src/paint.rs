//! paint.rs - renderiza toasts de notificacao.

use tiny_skia::{Color, Paint, PathBuilder, PixmapMut, Rect, Transform};

fn rgba(hex: u32, alpha: u8) -> Color {
    Color::from_rgba8(((hex >> 16) & 0xFF) as u8, ((hex >> 8) & 0xFF) as u8, (hex & 0xFF) as u8, alpha)
}

fn fill_rrect(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    if w <= 0.0 || h <= 0.0 { return; }
    let r = r.min(w * 0.5).min(h * 0.5);
    let mut pb = PathBuilder::new();
    pb.move_to(x + r, y); pb.line_to(x + w - r, y); pb.quad_to(x + w, y, x + w, y + r);
    pb.line_to(x + w, y + h - r); pb.quad_to(x + w, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h); pb.quad_to(x, y + h, x, y + h - r);
    pb.line_to(x, y + r); pb.quad_to(x, y, x + r, y); pb.close();
    let path = pb.finish().unwrap();
    let mut paint = Paint::default(); paint.set_color(color);
    canvas.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
}

fn fill_circle(canvas: &mut PixmapMut, cx: f32, cy: f32, r: f32, color: Color) {
    fill_rrect(canvas, cx - r, cy - r, r * 2.0, r * 2.0, r, color);
}

fn fill_rect_c(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, color: Color) {
    if w <= 0.0 || h <= 0.0 { return; }
    let mut paint = Paint::default(); paint.set_color(color);
    if let Some(rect) = Rect::from_xywh(x, y, w, h) { canvas.fill_rect(rect, &paint, Transform::identity(), None); }
}

pub const TOAST_W: f32 = 320.0;
pub const TOAST_H: f32 = 80.0;
pub const TOAST_RADIUS: f32 = 14.0;
pub const TOAST_MARGIN_RIGHT: f32 = 16.0;
pub const TOAST_MARGIN_TOP: f32 = 16.0;
pub const TOAST_GAP: f32 = 8.0;

pub struct ToastRender {
    pub id: u32,
    pub slide_x: f32,
    pub summary: String,
    pub app_name: String,
    pub body: String,
}

pub fn paint_toasts(canvas: &mut PixmapMut, toasts: &[ToastRender], width: u32, _height: u32) {
    canvas.fill(Color::TRANSPARENT);
    let w = width as f32;
    let accent = rgba(0x10b981, 0xFF);
    let pearl = rgba(0xf5f5f7, 0xFF);
    let muted = rgba(0x9596a0, 0xCC);
    let toast_bg = rgba(0x1a1a21, 0xF2);
    for (i, toast) in toasts.iter().enumerate() {
        let y = TOAST_MARGIN_TOP + i as f32 * (TOAST_H + TOAST_GAP);
        let tx = w - TOAST_W - TOAST_MARGIN_RIGHT + toast.slide_x;
        for sh in 1..=3u8 {
            fill_rrect(canvas, tx - sh as f32 * 0.5, y + sh as f32 * 1.5, TOAST_W + sh as f32, TOAST_H, TOAST_RADIUS, rgba(0x000000, 60u8.saturating_sub(sh * 15)));
        }
        fill_rrect(canvas, tx, y, TOAST_W, TOAST_H, TOAST_RADIUS, toast_bg);
        fill_rrect(canvas, tx, y + 8.0, 3.0, TOAST_H - 16.0, 2.0, accent);
        for (j, _) in toast.app_name.chars().take(20).enumerate() {
            fill_circle(canvas, tx + 14.0 + j as f32 * 5.5, y + 20.0, 1.5, muted);
        }
        for (j, _) in toast.summary.chars().take(32).enumerate() {
            fill_circle(canvas, tx + 14.0 + j as f32 * 7.0, y + 42.0, 2.2, pearl);
        }
        for (j, _) in toast.body.chars().take(40).enumerate() {
            fill_circle(canvas, tx + 14.0 + j as f32 * 5.5, y + 62.0, 1.5, muted);
        }
        fill_circle(canvas, tx + TOAST_W - 20.0, y + 20.0, 7.0, rgba(0x3f3f46, 0xCC));
        fill_rect_c(canvas, tx + TOAST_W - 25.0, y + 19.0, 10.0, 2.0, muted);
        fill_rect_c(canvas, tx + TOAST_W - 20.5, y + 14.5, 2.0, 10.0, muted);
    }
}
