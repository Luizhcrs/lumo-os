//! bar/dropdowns/battery.rs - Dropdown bateria + struct BatteryInfo.
//!
//! Layout (A20.1):
//!   y0  Bateria (title bold)
//!   y1  100% . Carregando (medium)
//!   sep
//!   y2  Saude:      92%
//!   y3  Ciclos:     142
//!   y4  Tempo:      cheia
//!
//! Mesma cor pill_bg (consistente). Sem accent glow.

use lumo_foundation::LumoColors;
use tiny_skia::{Paint, PixmapMut, Rect, Transform};

use crate::bar::fonts::{draw_text, draw_text_mono, measure_text_mono, opaque, rgba_hex};
use crate::bar::icons::fill_rrect;
use crate::bar::tokens::*;

// ============================================================
// BatteryInfo - leitura completa /sys/class/power_supply/BAT0.
// ============================================================

#[derive(Clone, Default, Debug)]
pub struct BatteryInfo {
    pub pct: u8,
    pub status: String,
    pub cycles: Option<u32>,
    pub full: Option<u32>,         // mWh
    pub now: Option<u32>,          // mWh
    pub full_design: Option<u32>,  // mWh
    pub power_now: Option<u32>,    // mW
    pub voltage_now_mv: Option<u32>,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
}

/// Status traduzido PT-BR.
pub fn status_pt(s: &str) -> &str {
    match s {
        "Charging" => "Carregando",
        "Discharging" => "Descarregando",
        "Full" => "Cheia",
        "Not charging" => "Pausada",
        "Unknown" => "Desconhecido",
        other => other,
    }
}

/// Saude % = full / full_design * 100 (mWh normalizado).
pub fn battery_health(info: &BatteryInfo) -> Option<u8> {
    let full = info.full? as f32;
    let design = info.full_design? as f32;
    if design < 1.0 {
        return None;
    }
    Some(((full / design) * 100.0).round().clamp(0.0, 100.0) as u8)
}

/// Tempo restante string PT-BR ("2h 15min", "cheia", "-").
pub fn battery_time_left(info: &BatteryInfo) -> String {
    let p = match info.power_now {
        Some(v) if v > 0 => v as f32,
        _ => return match info.status.as_str() {
            "Full" => "cheia".into(),
            _ => "-".into(),
        },
    };
    let (numer, label) = match info.status.as_str() {
        "Discharging" => (info.now.map(|v| v as f32), "restante"),
        "Charging" => {
            let now = info.now.map(|v| v as f32);
            let full = info.full.map(|v| v as f32);
            match (now, full) {
                (Some(n), Some(f)) => (Some((f - n).max(0.0)), "ate cheia"),
                _ => (None, ""),
            }
        }
        "Full" => return "cheia".into(),
        _ => (None, ""),
    };
    let energy = match numer {
        Some(e) => e,
        None => return "-".into(),
    };
    let hours_total = energy / p; // mWh / mW = h
    if hours_total < 0.01 {
        return "menos 1min".into();
    }
    let h = hours_total.floor() as u32;
    let m = ((hours_total - h as f32) * 60.0).round() as u32;
    if h == 0 {
        format!("{}min {}", m, label)
    } else {
        format!("{}h {:02}min {}", h, m, label)
    }
}

// ============================================================
// draw_battery_dropdown (A20).
// ============================================================

pub fn draw_battery_dropdown(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    palette: &LumoColors,
    info: &BatteryInfo,
) {
    let bg = rgba_hex(palette.pill_bg, palette.pill_bg_alpha);
    let fg = opaque(palette.pill_fg);
    let fg_subtle = rgba_hex(palette.pill_fg, 0xA0);
    let sep_color = rgba_hex(palette.pill_sep, palette.pill_sep_alpha);

    // Background rounded rect (mesma radius pill).
    fill_rrect(canvas, x, y, w, h, PILL_RADIUS, bg);

    // Title "Bateria" 14px bold.
    let cx = x + DROPDOWN_PAD;
    let mut cy = y + DROPDOWN_PAD;
    draw_text(canvas, cx, cy, "Bateria", FONT_DROPDOWN_TITLE, fg, true);
    cy += FONT_DROPDOWN_TITLE * 1.4;

    // Linha "{N}% . {status}".
    let summary = format!("{}%  .  {}", info.pct, status_pt(&info.status));
    draw_text(canvas, cx, cy, &summary, FONT_DROPDOWN_BODY, fg_subtle, false);
    cy += FONT_DROPDOWN_BODY * 1.6;

    // Separator linha cinza 1px.
    if let Some(rect) = Rect::from_xywh(x + DROPDOWN_PAD, cy.round(), w - DROPDOWN_PAD * 2.0, 1.0) {
        let mut p = Paint::default();
        p.set_color(sep_color);
        p.anti_alias = false;
        canvas.fill_rect(rect, &p, Transform::identity(), None);
    }
    cy += 8.0;

    // Linhas key:value.
    let value_x = x + w - DROPDOWN_PAD;
    let rows: [(&str, String); 3] = [
        (
            "Saude",
            battery_health(info)
                .map(|h| format!("{}%", h))
                .unwrap_or_else(|| "-".into()),
        ),
        (
            "Ciclos",
            info.cycles.map(|c| c.to_string()).unwrap_or_else(|| "-".into()),
        ),
        // A20.1: removido Voltagem/Modelo (irrelevantes ao usuario)
        ("Tempo", battery_time_left(info)),
    ];
    for (key, value) in rows.iter() {
        draw_text(canvas, cx, cy, key, FONT_DROPDOWN_BODY, fg_subtle, false);
        // Truncar valor longo (modelo): max ~24 chars no espaco disponivel.
        let mut v = value.clone();
        if v.chars().count() > 22 {
            v.truncate(v.char_indices().nth(20).map(|(i, _)| i).unwrap_or(v.len()));
            v.push_str("..");
        }
        // A29: valor numerico = Geist Mono (alinhamento tabular).
        let vw = measure_text_mono(&v, FONT_DROPDOWN_BODY, false);
        draw_text_mono(canvas, value_x - vw, cy, &v, FONT_DROPDOWN_BODY, fg, false);
        cy += DROPDOWN_ROW_H;
    }
}
