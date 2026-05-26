//! paint.rs - renderiza o launcher overlay.

use crate::desktop::DesktopEntry;
use tiny_skia::{Color, Paint, PathBuilder, PixmapMut, Rect, Transform};

fn rgba(hex: u32, alpha: u8) -> Color {
    Color::from_rgba8(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
        alpha,
    )
}

fn fill_rect_c(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, color: Color) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let mut paint = Paint::default();
    paint.set_color(color);
    if let Some(rect) = Rect::from_xywh(x, y, w, h) {
        canvas.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

fn fill_rrect(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let r = r.min(w * 0.5).min(h * 0.5);
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
    let path = pb.finish().unwrap();
    let mut paint = Paint::default();
    paint.set_color(color);
    canvas.fill_path(
        &path,
        &paint,
        tiny_skia::FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn fill_circle(canvas: &mut PixmapMut, cx: f32, cy: f32, r: f32, color: Color) {
    fill_rrect(canvas, cx - r, cy - r, r * 2.0, r * 2.0, r, color);
}

pub struct PaintInput<'a> {
    pub query: &'a str,
    pub results: &'a [DesktopEntry],
    pub selected_idx: usize,
    pub math_result: Option<&'a str>,
    pub width: u32,
    pub height: u32,
}

pub fn paint_launcher(canvas: &mut PixmapMut, input: &PaintInput) -> Vec<(f32, f32)> {
    use crate::{MAX_RESULTS, PANEL_H_BASE, PANEL_RADIUS, PANEL_W, ROW_H, SEARCH_BOX_H};
    let n = input.results.len().min(MAX_RESULTS);
    let has_math = input.math_result.is_some();
    let math_h = if has_math { ROW_H } else { 0.0 };
    let panel_h = PANEL_H_BASE + n as f32 * ROW_H + math_h + 16.0;
    let sw = input.width as f32;
    let sh = input.height as f32;
    let px = (sw - PANEL_W) * 0.5;
    let py = sh * 0.32;

    canvas.fill(Color::TRANSPARENT);
    fill_rect_c(canvas, 0.0, 0.0, sw, sh, rgba(0x0a0a0c, 0xCC));

    for i in 1..=4u8 {
        let e = i as f32;
        fill_rrect(
            canvas,
            px - e,
            py + e * 2.0,
            PANEL_W + e * 2.0,
            panel_h,
            PANEL_RADIUS,
            rgba(0x000000, 80u8.saturating_sub(i * 15)),
        );
    }
    fill_rrect(
        canvas,
        px,
        py,
        PANEL_W,
        panel_h,
        PANEL_RADIUS,
        rgba(0x1a1a21, 0xF5),
    );

    let sx = px + 16.0;
    let sy = py + 12.0;
    let sw2 = PANEL_W - 32.0;
    fill_rrect(
        canvas,
        sx,
        sy,
        sw2,
        SEARCH_BOX_H,
        10.0,
        rgba(0x0a0a0c, 0xCC),
    );

    // Cursor
    let cursor_x = sx + 12.0 + input.query.len() as f32 * 8.5;
    fill_rect_c(
        canvas,
        cursor_x,
        sy + 10.0,
        2.0,
        SEARCH_BOX_H - 20.0,
        rgba(0x10b981, 0xFF),
    );

    // Placeholder dots
    if input.query.is_empty() {
        for i in 0..12u32 {
            fill_circle(
                canvas,
                sx + 14.0 + i as f32 * 8.0,
                sy + SEARCH_BOX_H * 0.5,
                1.5,
                rgba(0x9596a0, 0x66),
            );
        }
    }

    let accent = rgba(0x10b981, 0xFF);
    let pearl = rgba(0xf5f5f7, 0xFF);
    let muted = rgba(0x9596a0, 0xFF);
    let mut hit_rects = Vec::new();
    let ry = sy + SEARCH_BOX_H + 8.0;

    if let Some(math) = input.math_result {
        fill_rrect(
            canvas,
            px + 8.0,
            ry,
            PANEL_W - 16.0,
            ROW_H,
            8.0,
            rgba(0x10b981, 0x22),
        );
        fill_circle(canvas, px + 24.0, ry + ROW_H * 0.5, 8.0, accent);
        for (i, _) in math.chars().take(20).enumerate() {
            fill_circle(
                canvas,
                px + 44.0 + i as f32 * 9.0,
                ry + ROW_H * 0.5,
                2.5,
                pearl,
            );
        }
        hit_rects.push((ry, ROW_H));
    }

    for (i, entry) in input.results.iter().enumerate().take(MAX_RESULTS) {
        let mo = if has_math { ROW_H } else { 0.0 };
        let row_y = ry + mo + i as f32 * ROW_H;
        let sel = i == input.selected_idx;
        if sel {
            fill_rrect(
                canvas,
                px + 8.0,
                row_y,
                PANEL_W - 16.0,
                ROW_H,
                8.0,
                rgba(0x10b981, 0x2A),
            );
        }
        let icon_bg = if sel {
            rgba(0x10b981, 0x55)
        } else {
            rgba(0x3f3f46, 0xFF)
        };
        fill_rrect(
            canvas,
            px + 14.0,
            row_y + ROW_H * 0.5 - 14.0,
            28.0,
            28.0,
            7.0,
            icon_bg,
        );
        fill_circle(canvas, px + 28.0, row_y + ROW_H * 0.5, 5.0, accent);
        let fg = if sel { pearl } else { muted };
        for (j, _) in entry.name.chars().take(30).enumerate() {
            fill_circle(
                canvas,
                px + 52.0 + j as f32 * 7.5,
                row_y + ROW_H * 0.45,
                2.0,
                fg,
            );
        }
        hit_rects.push((row_y, ROW_H));
    }
    hit_rects
}
