//! lumo-lock render module W10.A -- paint_lock draws the lock screen frame.
//!
//! Layout:
//!   - Full-screen dark gradient backdrop (opacity 0.92, simulates blur)
//!   - Clock: time string centered using 5x7 pixel font blocks
//!   - Date: secondary line below clock
//!   - Password dots: centered row of filled circles, 1 per char
//!   - Error message: red text below dots when last_fail_msg is non-empty

use chrono::Local;
use tiny_skia::{Color, FillRule, Paint, PathBuilder, PixmapMut, Rect, Transform};

const BACKDROP_ALPHA: u8 = 235;
const DOT_RADIUS: f32 = 6.0;
const DOT_SPACING: f32 = 18.0;
const DOT_Y_OFFSET: f32 = 80.0;
const ERROR_Y_OFFSET: f32 = 120.0;

fn fill_rect(pixmap: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, color: Color) {
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = false;
    if let Some(rect) = Rect::from_xywh(x, y, w.max(1.0), h.max(1.0)) {
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

fn fill_circle(pixmap: &mut PixmapMut, cx: f32, cy: f32, r: f32, color: Color) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    let path = match pb.finish() {
        Some(p) => p,
        None => return,
    };
    let mut paint = Paint::default();
    paint.set_color(color);
    paint.anti_alias = true;
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

fn draw_pixel_char(pixmap: &mut PixmapMut, x: f32, y: f32, scale: f32, ch: char, color: Color) {
    let bitmap = pixel_char_bitmap(ch);
    for (row, &bits) in bitmap.iter().enumerate() {
        for col in 0..5u8 {
            if bits & (1 << (4 - col)) != 0 {
                fill_rect(
                    pixmap,
                    x + col as f32 * scale,
                    y + row as f32 * scale,
                    scale - 0.5,
                    scale - 0.5,
                    color,
                );
            }
        }
    }
}

fn pixel_char_bitmap(ch: char) -> [u8; 7] {
    if ch == ':' {
        return [
            0b00000, 0b00100, 0b00000, 0b00000, 0b00000, 0b00100, 0b00000,
        ];
    }
    match ch {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111,
        ],
        '3' => [
            0b11111, 0b00010, 0b00100, 0b00010, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        _ => [0b00000; 7],
    }
}

fn draw_digits_clock(pixmap: &mut PixmapMut, cx: f32, cy: f32) {
    let now = Local::now();
    let hh = now.format("%H").to_string();
    let mm = now.format("%M").to_string();
    let time_str = format!("{}:{}", hh, mm);

    const SCALE: f32 = 10.0;
    const CHAR_W: f32 = 5.0 * SCALE;
    const CHAR_H: f32 = 7.0 * SCALE;
    const CHAR_GAP: f32 = 2.0 * SCALE;

    let chars: Vec<char> = time_str.chars().collect();
    let total_w = chars.len() as f32 * (CHAR_W + CHAR_GAP) - CHAR_GAP;
    let start_x = cx - total_w / 2.0;
    let start_y = cy - CHAR_H / 2.0;

    for (i, ch) in chars.iter().enumerate() {
        let bx = start_x + i as f32 * (CHAR_W + CHAR_GAP);
        draw_pixel_char(pixmap, bx, start_y, SCALE, *ch, Color::WHITE);
    }
}

fn draw_date_line(pixmap: &mut PixmapMut, cx: f32, cy: f32) {
    let now = Local::now();
    let date_str = now.format("%d/%m/%Y").to_string();
    const SCALE: f32 = 2.5;
    const CHAR_W: f32 = 5.0 * SCALE;
    const CHAR_H: f32 = 7.0 * SCALE;
    const CHAR_GAP: f32 = 1.5 * SCALE;
    let chars: Vec<char> = date_str.chars().collect();
    let total_w = chars.len() as f32 * (CHAR_W + CHAR_GAP) - CHAR_GAP;
    let start_x = cx - total_w / 2.0;
    let start_y = cy - CHAR_H / 2.0;
    let color = Color::from_rgba8(200, 200, 200, 255);
    for (i, ch) in chars.iter().enumerate() {
        let bx = start_x + i as f32 * (CHAR_W + CHAR_GAP);
        draw_pixel_char(pixmap, bx, start_y, SCALE, *ch, color);
    }
}

fn draw_password_dots(pixmap: &mut PixmapMut, cx: f32, cy: f32, len: usize, shake_offset: f32) {
    if len == 0 {
        let box_w = 200.0f32;
        let box_h = 28.0f32;
        let bx = cx - box_w / 2.0 + shake_offset;
        let by = cy - box_h / 2.0;
        let color = Color::from_rgba8(255, 255, 255, 60);
        fill_rect(pixmap, bx, by, box_w, box_h, color);
        return;
    }
    let n = len as f32;
    let total_w = n * DOT_RADIUS * 2.0 + (n - 1.0) * DOT_SPACING;
    let start_x = cx - total_w / 2.0 + shake_offset;
    for i in 0..len {
        let dot_cx = start_x + i as f32 * (DOT_RADIUS * 2.0 + DOT_SPACING) + DOT_RADIUS;
        fill_circle(pixmap, dot_cx, cy, DOT_RADIUS, Color::WHITE);
    }
}

fn draw_error_text(pixmap: &mut PixmapMut, cx: f32, cy: f32, msg: &str) {
    if msg.is_empty() {
        return;
    }
    let color = Color::from_rgba8(255, 100, 100, 220);
    const SCALE: f32 = 2.0;
    const CHAR_W: f32 = 5.0 * SCALE;
    const CHAR_H: f32 = 7.0 * SCALE;
    const CHAR_GAP: f32 = 1.0 * SCALE;
    let chars: Vec<char> = msg.chars().collect();
    let total_w = chars.len() as f32 * (CHAR_W + CHAR_GAP);
    let start_x = cx - total_w / 2.0;
    let start_y = cy - CHAR_H / 2.0;
    for (i, ch) in chars.iter().enumerate() {
        let bx = start_x + i as f32 * (CHAR_W + CHAR_GAP);
        draw_pixel_char(pixmap, bx, start_y, SCALE, *ch, color);
    }
}

/// Main paint function called each frame.
pub fn paint_lock(
    pixmap: &mut PixmapMut,
    width: u32,
    height: u32,
    password: &str,
    last_fail_msg: &str,
    shake_offset: f32,
) {
    let w = width as f32;
    let h = height as f32;
    let cx = w / 2.0;
    let cy = h / 2.0 - 40.0;

    // Dark backdrop.
    let backdrop = Color::from_rgba8(10, 10, 18, BACKDROP_ALPHA);
    fill_rect(pixmap, 0.0, 0.0, w, h, backdrop);

    // Vignette edges.
    let vig = Color::from_rgba8(0, 0, 0, 50);
    fill_rect(pixmap, 0.0, 0.0, w * 0.25, h, vig);
    fill_rect(pixmap, w * 0.75, 0.0, w * 0.25, h, vig);
    fill_rect(pixmap, 0.0, 0.0, w, h * 0.15, vig);
    fill_rect(pixmap, 0.0, h * 0.85, w, h * 0.15, vig);

    // Clock.
    draw_digits_clock(pixmap, cx, cy - 20.0);

    // Date.
    draw_date_line(pixmap, cx, cy + 65.0);

    // Separator.
    let sep = Color::from_rgba8(255, 255, 255, 40);
    fill_rect(
        pixmap,
        cx - 120.0,
        cy + DOT_Y_OFFSET - 10.0,
        240.0,
        1.0,
        sep,
    );

    // Password dots.
    draw_password_dots(
        pixmap,
        cx,
        cy + DOT_Y_OFFSET + 20.0,
        password.len(),
        shake_offset,
    );

    // Error message.
    if !last_fail_msg.is_empty() {
        draw_error_text(pixmap, cx, cy + ERROR_Y_OFFSET + 30.0, last_fail_msg);
    }
}
