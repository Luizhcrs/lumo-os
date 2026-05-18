//! bar/dropdowns/brightness.rs - Dropdown brilho (L5).
//!
//! Layout:
//!   y0  Brilho (title)
//!   y1  [=====|====] N%  (slider fill bar + label)
//!   sep
//!   y2  [Dia 80%]  [Noite 35%]  (preset buttons)

use lumo_foundation::LumoColors;
use tiny_skia::{Paint, PixmapMut, Rect, Transform};

use crate::bar::fonts::{draw_text, draw_text_mono, measure_text, measure_text_mono, opaque, rgba_hex};
use crate::bar::icons::fill_rrect;
use crate::bar::tokens::*;

#[derive(Clone, Debug)]
pub struct BrightnessInfo {
    /// Current brightness 0-100.
    pub pct: u8,
}

impl Default for BrightnessInfo {
    fn default() -> Self {
        Self { pct: 80 }
    }
}

/// Hit-rects returned from draw_brightness_dropdown.
#[derive(Default)]
pub struct BrightnessDropdownHits {
    /// Clickable slider track area (user clicks to set brightness).
    pub slider_rect: Option<(f32, f32, f32, f32)>,
    /// Preset "Dia 80%" button.
    pub preset_day_rect: Option<(f32, f32, f32, f32)>,
    /// Preset "Noite 35%" button.
    pub preset_night_rect: Option<(f32, f32, f32, f32)>,
}

pub fn draw_brightness_dropdown(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    palette: &LumoColors,
    info: &BrightnessInfo,
) -> BrightnessDropdownHits {
    let bg = rgba_hex(palette.pill_bg, palette.pill_bg_alpha);
    let fg = opaque(palette.pill_fg);
    let fg_subtle = rgba_hex(palette.pill_fg, 0xA0);
    let sep_color = rgba_hex(palette.pill_sep, palette.pill_sep_alpha);
    let accent = opaque(palette.accent);
    let track_bg = rgba_hex(palette.pill_fg, 0x28);
    let mut hits = BrightnessDropdownHits::default();

    fill_rrect(canvas, x, y, w, h, PILL_RADIUS, bg);

    let cx = x + DROPDOWN_PAD;
    let mut cy = y + DROPDOWN_PAD;

    // Title.
    draw_text(canvas, cx, cy, "Brilho", FONT_DROPDOWN_TITLE, fg, true);
    cy += FONT_DROPDOWN_TITLE * 1.4;

    // Slider: horizontal fill bar.
    let track_x = cx;
    let track_w = w - DROPDOWN_PAD * 2.0 - 40.0; // leave space for pct label
    let track_y = cy + (FONT_DROPDOWN_BODY - BRIGHTNESS_SLIDER_H) / 2.0;

    // Track background.
    if let Some(rect) = Rect::from_xywh(track_x, track_y.round(), track_w, BRIGHTNESS_SLIDER_H) {
        let mut p = Paint::default();
        p.set_color(track_bg);
        p.anti_alias = false;
        canvas.fill_rect(rect, &p, Transform::identity(), None);
    }
    // Track fill (accent color).
    let fill_w = (track_w * info.pct as f32 / 100.0).clamp(2.0, track_w);
    if let Some(rect) = Rect::from_xywh(track_x, track_y.round(), fill_w, BRIGHTNESS_SLIDER_H) {
        let mut p = Paint::default();
        p.set_color(accent);
        p.anti_alias = false;
        canvas.fill_rect(rect, &p, Transform::identity(), None);
    }
    // Pct label right of track.
    let pct_str = format!("{}%", info.pct);
    let pct_w = measure_text_mono(&pct_str, FONT_DROPDOWN_BODY, false);
    let pct_x = track_x + track_w + 8.0;
    draw_text_mono(canvas, pct_x, cy, &pct_str, FONT_DROPDOWN_BODY, fg, false);

    hits.slider_rect = Some((track_x, track_y - 4.0, track_w, BRIGHTNESS_SLIDER_H + 8.0));
    cy += FONT_DROPDOWN_BODY + 8.0;
    let _ = pct_w;

    // Separator.
    if let Some(rect) = Rect::from_xywh(x + DROPDOWN_PAD, cy.round(), w - DROPDOWN_PAD * 2.0, 1.0) {
        let mut p = Paint::default();
        p.set_color(sep_color);
        p.anti_alias = false;
        canvas.fill_rect(rect, &p, Transform::identity(), None);
    }
    cy += 8.0;

    // Preset buttons: [Dia 80%]   [Noite 35%].
    let btn_h = 20.0;
    let btn_radius = 6.0;
    let half_w = (w - DROPDOWN_PAD * 2.0 - 8.0) / 2.0;

    let day_x = cx;
    let night_x = cx + half_w + 8.0;

    // Day button.
    let day_bg = if info.pct == 80 { accent } else { track_bg };
    let day_fg = if info.pct == 80 { bg } else { fg_subtle };
    fill_rrect(canvas, day_x, cy, half_w, btn_h, btn_radius, day_bg);
    let day_label = "Dia  80%";
    let day_lw = measure_text(day_label, FONT_DROPDOWN_BODY, false);
    draw_text(
        canvas,
        day_x + (half_w - day_lw) / 2.0,
        cy + (btn_h - FONT_DROPDOWN_BODY) / 2.0,
        day_label,
        FONT_DROPDOWN_BODY,
        day_fg,
        false,
    );
    hits.preset_day_rect = Some((day_x, cy, half_w, btn_h));

    // Night button.
    let night_bg = if info.pct == 35 { accent } else { track_bg };
    let night_fg = if info.pct == 35 { bg } else { fg_subtle };
    fill_rrect(canvas, night_x, cy, half_w, btn_h, btn_radius, night_bg);
    let night_label = "Noite  35%";
    let night_lw = measure_text(night_label, FONT_DROPDOWN_BODY, false);
    draw_text(
        canvas,
        night_x + (half_w - night_lw) / 2.0,
        cy + (btn_h - FONT_DROPDOWN_BODY) / 2.0,
        night_label,
        FONT_DROPDOWN_BODY,
        night_fg,
        false,
    );
    hits.preset_night_rect = Some((night_x, cy, half_w, btn_h));

    let _ = h;
    hits
}
