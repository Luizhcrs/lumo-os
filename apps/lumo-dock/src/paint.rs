//! paint.rs - renderiza o dock com tiny-skia.

use std::collections::HashMap;
use tiny_skia::{Color, Paint, PathBuilder, PixmapMut, Rect, Transform};
use lumo_animation::Spring;
use crate::config::SlotConfig;

fn rgba(hex: u32, alpha: u8) -> Color {
    let r = ((hex >> 16) & 0xFF) as u8;
    let g = ((hex >> 8) & 0xFF) as u8;
    let b = (hex & 0xFF) as u8;
    Color::from_rgba8(r, g, b, alpha)
}

fn fill_rect(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, color: Color) {
    if w <= 0.0 || h <= 0.0 { return; }
    let mut paint = Paint::default();
    paint.set_color(color);
    if let Some(rect) = Rect::from_xywh(x, y, w, h) {
        canvas.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

fn fill_rrect(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    if w <= 0.0 || h <= 0.0 { return; }
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
    canvas.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
}

fn fill_circle(canvas: &mut PixmapMut, cx: f32, cy: f32, r: f32, color: Color) {
    fill_rrect(canvas, cx - r, cy - r, r * 2.0, r * 2.0, r, color);
}

fn draw_icon(canvas: &mut PixmapMut, cx: f32, cy: f32, size: f32, name: &str, fg: Color) {
    let h = size * 0.5;
    let q = size * 0.25;
    match name {
        "home" => {
            let mut pb = PathBuilder::new();
            pb.move_to(cx, cy - h * 0.7);
            pb.line_to(cx + h * 0.8, cy + h * 0.1);
            pb.line_to(cx - h * 0.8, cy + h * 0.1);
            pb.close();
            if let Some(path) = pb.finish() {
                let mut p = Paint::default(); p.set_color(fg);
                canvas.fill_path(&path, &p, tiny_skia::FillRule::Winding, Transform::identity(), None);
            }
            fill_rrect(canvas, cx - h * 0.5, cy + h * 0.05, h, h * 0.7, 2.0, fg);
        }
        "calc" => {
            let sq = size * 0.18; let gap = size * 0.08;
            for row in 0..2i32 { for col in 0..2i32 {
                let bx = cx - sq - gap * 0.5 + col as f32 * (sq + gap);
                let by = cy - sq - gap * 0.5 + row as f32 * (sq + gap);
                fill_rrect(canvas, bx, by, sq, sq, 2.0, fg);
            }}
        }
        "settings" => {
            fill_circle(canvas, cx, cy, q * 0.8, fg);
            fill_circle(canvas, cx, cy, q * 0.4, rgba(0x131318, 0xFF));
            let tw = size * 0.12; let th = size * 0.22;
            for i in 0..4i32 {
                let angle = (i as f32) * std::f32::consts::FRAC_PI_2;
                fill_rrect(canvas, cx + angle.cos() * q * 0.75 - tw * 0.5, cy + angle.sin() * q * 0.75 - th * 0.5, tw, th, 1.0, fg);
            }
        }
        "browser" => {
            fill_circle(canvas, cx, cy, h * 0.75, fg);
            fill_circle(canvas, cx, cy, h * 0.55, rgba(0x131318, 0xFF));
            fill_rect(canvas, cx - h * 0.7, cy - 1.5, h * 1.4, 3.0, fg);
        }
        "term" => {
            fill_rrect(canvas, cx - h * 0.6, cy - h * 0.6, h * 1.2, h * 1.2, 4.0, fg);
            fill_rrect(canvas, cx - h * 0.45, cy - h * 0.45, h * 0.9, h * 0.9, 2.0, rgba(0x131318, 0xFF));
            let mut pb = PathBuilder::new();
            pb.move_to(cx - q * 0.3, cy - q * 0.4);
            pb.line_to(cx + q * 0.3, cy);
            pb.line_to(cx - q * 0.3, cy + q * 0.4);
            if let Some(path) = pb.finish() {
                let stroke = tiny_skia::Stroke { width: 2.0, ..Default::default() };
                let mut p = Paint::default(); p.set_color(fg);
                canvas.stroke_path(&path, &p, &stroke, Transform::identity(), None);
            }
        }
        "calendar" => {
            fill_rrect(canvas, cx - h * 0.7, cy - h * 0.6, h * 1.4, h * 1.3, 3.0, fg);
            fill_rect(canvas, cx - h * 0.7, cy - h * 0.2, h * 1.4, h * 1.1, rgba(0x131318, 0xFF));
            for row in 0..2i32 { for col in 0..3i32 {
                fill_circle(canvas, cx - h * 0.55 + col as f32 * h * 0.42, cy - h * 0.08 + row as f32 * h * 0.35, 2.5, fg);
            }}
        }
        "trash" => {
            fill_rrect(canvas, cx - q * 0.8, cy - q * 0.3, q * 1.6, h * 0.9, 3.0, fg);
            fill_rect(canvas, cx - h * 0.55, cy - q * 0.55, h * 1.1, h * 0.18, fg);
            fill_rrect(canvas, cx - q * 0.5, cy - h * 0.7, q, h * 0.28, 2.0, fg);
        }
        _ => { fill_rrect(canvas, cx - h * 0.6, cy - h * 0.6, h * 1.2, h * 1.2, 6.0, fg); }
    }
    let _ = q;
}

pub fn paint_dock(
    canvas: &mut PixmapMut, width: u32, height: u32,
    slots: &[SlotConfig], scales: &[Spring], hover_idx: i32,
    running_procs: &HashMap<String, bool>,
) -> (Vec<(f32, f32)>, Option<(f32, f32)>) {
    use crate::{DOCK_RADIUS, DOT_R, ICON_MARGIN, ICON_SIZE, SEPARATOR_H, SEPARATOR_W};
    canvas.fill(Color::TRANSPARENT);
    let n = slots.len();
    let slot_w = ICON_SIZE + ICON_MARGIN * 2.0;
    let sep_w = SEPARATOR_W + ICON_MARGIN * 2.0;
    let pill_w = slot_w * (n + 1) as f32 + sep_w;
    let pill_h = height as f32 - 8.0;
    let pill_x = (width as f32 - pill_w) * 0.5;
    let pill_y = (height as f32 - pill_h) * 0.5;
    for i in 1..=4u8 {
        let expand = i as f32 * 0.5;
        fill_rrect(canvas, pill_x - expand, pill_y + i as f32, pill_w + expand * 2.0, pill_h, DOCK_RADIUS, rgba(0x000000, 80u8.saturating_sub(i * 15)));
    }
    fill_rrect(canvas, pill_x, pill_y, pill_w, pill_h, DOCK_RADIUS, rgba(0x131318, 0xEE));
    let mut slot_rects = Vec::with_capacity(n + 1);
    let cy = height as f32 * 0.5;
    let accent = rgba(0x10b981, 0xFF);
    let pearl = rgba(0xf5f5f7, 0xFF);
    let muted = rgba(0x9596a0, 0x99);
    for (i, slot) in slots.iter().enumerate() {
        let scale = scales.get(i).map(|s| s.value).unwrap_or(1.0);
        let cx = pill_x + ICON_MARGIN + ICON_SIZE * 0.5 + i as f32 * slot_w;
        let icon_cy = cy - ICON_SIZE * (scale - 1.0) * 0.5;
        draw_icon(canvas, cx, icon_cy, ICON_SIZE * scale, &slot.icon, if hover_idx == i as i32 { pearl } else { muted });
        if !slot.process.is_empty() && running_procs.get(&slot.process).copied().unwrap_or(false) {
            fill_circle(canvas, cx, pill_y + pill_h - DOT_R - 3.0, DOT_R, accent);
        }
        slot_rects.push((pill_x + i as f32 * slot_w, slot_w));
    }
    let sep_cx = pill_x + n as f32 * slot_w + ICON_MARGIN;
    fill_rect(canvas, sep_cx, cy - SEPARATOR_H * 0.5, SEPARATOR_W, SEPARATOR_H, muted);
    let ti = n;
    let ts = scales.get(ti).map(|s| s.value).unwrap_or(1.0);
    let tcx = sep_cx + sep_w * 0.5 + ICON_SIZE * 0.5;
    draw_icon(canvas, tcx, cy - ICON_SIZE * (ts - 1.0) * 0.5, ICON_SIZE * ts, "trash", if hover_idx == ti as i32 { pearl } else { muted });
    (slot_rects, Some((sep_cx + sep_w * 0.5, slot_w)))
}
