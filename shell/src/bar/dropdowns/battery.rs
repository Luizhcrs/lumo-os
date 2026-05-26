//! bar/dropdowns/battery.rs - Dropdown bateria + struct BatteryInfo.
//!
//! Layout (L5):
//!   y0  Bateria (title bold)
//!   y1  [icone] 100% . Carregando
//!   sep
//!   y2  Saude:    98%
//!   y3  Ciclos:   47
//!   y4  Tempo:    4h 15min
//!   sep
//!   y5  [toggle] Cuidar bateria (80%)
//!   y6  Perfil: Equilibrado  [cycle ->]
//!   sep
//!   y7  Temp CPU: 52C (visivel so se > 70, cor por threshold)

use lumo_foundation::{i18n::I18n, LumoColors};
use tiny_skia::{Paint, PixmapMut, Rect, Transform};

use crate::bar::fonts::{draw_text, draw_text_mono, measure_text_mono, opaque, rgba_hex};
use crate::bar::icons::{draw_battery, fill_rrect};
use crate::bar::tokens::*;

#[derive(Clone, Default, Debug)]
pub struct BatteryInfo {
    pub pct: u8,
    pub status: String,
    pub cycles: Option<u32>,
    pub full: Option<u32>,
    pub now: Option<u32>,
    pub full_design: Option<u32>,
    pub power_now: Option<u32>,
    pub current_now: Option<u32>,
    pub voltage_now_mv: Option<u32>,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
    // L5: Samsung Galaxy Book 4 specific fields.
    pub charge_limit: Option<u8>,
    /// Days until next scheduled cell balance (Fri 22h). None if not applicable.
    pub balance_days: Option<u32>,
    pub platform_profile: Option<String>,
    pub cpu_temp_c: Option<f32>,
}

pub fn status_pt(s: &str) -> String {
    match s {
        "Charging" => I18n::get("battery.charging"),
        "Full" => I18n::get("battery.full"),
        "Not charging" => I18n::get("battery.not_charging"),
        "Discharging" => I18n::get("battery.discharging"),
        "Unknown" => I18n::get("battery.unknown"),
        other => other.to_string(),
    }
}

pub fn battery_health(info: &BatteryInfo) -> Option<u8> {
    let full = info.full? as f32;
    let design = info.full_design? as f32;
    if design < 1.0 {
        return None;
    }
    Some(((full / design) * 100.0).round().clamp(0.0, 100.0) as u8)
}

pub fn battery_time_left(info: &BatteryInfo) -> String {
    let current = info.current_now.or(info.power_now);
    let cur = match current {
        Some(v) if v > 0 => v as f32,
        _ => {
            return match info.status.as_str() {
                "Full" => "cheia".into(),
                _ => "-".into(),
            }
        }
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
    let hours_total = energy / cur;
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

pub fn profile_display(s: &str) -> &str {
    match s {
        "low-power" => "Economia",
        "quiet" => "Silencioso",
        "balanced" => "Equilibrado",
        "performance" => "Performance",
        _ => s,
    }
}

#[derive(Default)]
pub struct BatteryDropdownHits {
    pub charge_limit_toggle_rect: Option<(f32, f32, f32, f32)>,
    pub profile_cycle_rect: Option<(f32, f32, f32, f32)>,
}

pub fn draw_battery_dropdown(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    palette: &LumoColors,
    info: &BatteryInfo,
) -> BatteryDropdownHits {
    let bg = rgba_hex(palette.pill_bg, palette.pill_bg_alpha);
    let fg = opaque(palette.pill_fg);
    let fg_subtle = rgba_hex(palette.pill_fg, 0xA0);
    let sep_color = rgba_hex(palette.pill_sep, palette.pill_sep_alpha);
    let accent = opaque(palette.accent);
    let mut hits = BatteryDropdownHits::default();

    fill_rrect(canvas, x, y, w, h, PILL_RADIUS, bg);

    let cx = x + DROPDOWN_PAD;
    let value_x = x + w - DROPDOWN_PAD;
    let mut cy = y + DROPDOWN_PAD;

    draw_text(
        canvas,
        cx,
        cy,
        &I18n::get("battery.title"),
        FONT_DROPDOWN_TITLE,
        fg,
        true,
    );
    cy += FONT_DROPDOWN_TITLE * 1.4;

    let charging = info.status == "Charging";
    // pct 18px bold + status 11px muted abaixo: bloco total ~33px, icone centralizado.
    let block_h = 18.0 + 4.0 + 11.0;
    let icon_y = cy + block_h / 2.0 - BAT_BODY_H / 2.0;
    draw_battery(canvas, cx, icon_y, info.pct, charging, fg, accent);
    let summary_x = cx + BAT_BODY_W + 4.0 + 8.0;
    let pct_str = format!("{}%", info.pct);
    draw_text(canvas, summary_x, cy, &pct_str, 18.0, fg, true);
    let status_str = status_pt(&info.status);
    draw_text(
        canvas,
        summary_x,
        cy + 22.0,
        &status_str,
        11.0,
        fg_subtle,
        false,
    );
    cy += block_h + 6.0;

    draw_separator(canvas, x, cy, w, sep_color);
    cy += 8.0;

    {
        let val = battery_health(info)
            .map(|hv| format!("{}%", hv))
            .unwrap_or_else(|| "-".into());
        draw_kv_row(canvas, cx, cy, value_x, "Saude", &val, fg_subtle, fg);
        cy += DROPDOWN_ROW_H;
    }
    {
        let val = info
            .cycles
            .map(|c| c.to_string())
            .unwrap_or_else(|| "-".into());
        draw_kv_row(canvas, cx, cy, value_x, "Ciclos", &val, fg_subtle, fg);
        cy += DROPDOWN_ROW_H;
    }
    {
        let val = battery_time_left(info);
        draw_kv_row(canvas, cx, cy, value_x, "Tempo", &val, fg_subtle, fg);
        cy += DROPDOWN_ROW_H;
    }

    draw_separator(canvas, x, cy, w, sep_color);
    cy += 8.0;

    {
        let active = info.charge_limit.map(|l| l <= 80).unwrap_or(false);
        let label = if active {
            "[x] Cuidar bateria (80%)"
        } else {
            "[ ] Cuidar bateria (80%)"
        };
        let row_color = if active { accent } else { fg_subtle };
        draw_text(canvas, cx, cy, label, FONT_DROPDOWN_BODY, row_color, false);
        hits.charge_limit_toggle_rect = Some((x, cy - 2.0, w, DROPDOWN_ROW_H + 4.0));
        cy += DROPDOWN_ROW_H;
        if active {
            let bal_str = match info.balance_days {
                Some(0) => "Cell balance: hoje".to_string(),
                Some(1) => "Cell balance: amanha".to_string(),
                Some(n) => format!("Cell balance em {} dias", n),
                None => String::new(),
            };
            if !bal_str.is_empty() {
                draw_text(
                    canvas,
                    cx + 12.0,
                    cy,
                    &bal_str,
                    FONT_DROPDOWN_BODY,
                    fg_subtle,
                    false,
                );
                cy += DROPDOWN_ROW_H;
            }
        }
    }

    {
        let profile_name = info
            .platform_profile
            .as_deref()
            .map(profile_display)
            .unwrap_or("-");
        let val = format!("{}  ->", profile_name);
        draw_text(
            canvas,
            cx,
            cy,
            "Perfil",
            FONT_DROPDOWN_BODY,
            fg_subtle,
            false,
        );
        let vw = measure_text_mono(&val, FONT_DROPDOWN_BODY, false);
        draw_text_mono(
            canvas,
            value_x - vw,
            cy,
            &val,
            FONT_DROPDOWN_BODY,
            accent,
            false,
        );
        hits.profile_cycle_rect = Some((x, cy - 2.0, w, DROPDOWN_ROW_H + 4.0));
        cy += DROPDOWN_ROW_H;
    }

    if let Some(temp) = info.cpu_temp_c {
        if temp > 70.0 {
            draw_separator(canvas, x, cy, w, sep_color);
            cy += 8.0;
            let temp_color = if temp > 85.0 {
                rgba_hex(0xFF8C00, 0xFF)
            } else {
                rgba_hex(0xCCAA00, 0xFF)
            };
            let temp_str = format!("Temp CPU: {:.0}C", temp);
            draw_text(
                canvas,
                cx,
                cy,
                &temp_str,
                FONT_DROPDOWN_BODY,
                temp_color,
                false,
            );
        }
    }

    let _ = h;
    hits
}

fn draw_separator(canvas: &mut PixmapMut, x: f32, cy: f32, w: f32, color: tiny_skia::Color) {
    if let Some(rect) = Rect::from_xywh(x + DROPDOWN_PAD, cy.round(), w - DROPDOWN_PAD * 2.0, 1.0) {
        let mut p = Paint::default();
        p.set_color(color);
        p.anti_alias = false;
        canvas.fill_rect(rect, &p, Transform::identity(), None);
    }
}

fn draw_kv_row(
    canvas: &mut PixmapMut,
    cx: f32,
    cy: f32,
    value_x: f32,
    key: &str,
    value: &str,
    key_color: tiny_skia::Color,
    val_color: tiny_skia::Color,
) {
    draw_text(canvas, cx, cy, key, FONT_DROPDOWN_BODY, key_color, false);
    let mut v = value.to_string();
    if v.chars().count() > 22 {
        v.truncate(v.char_indices().nth(20).map(|(i, _)| i).unwrap_or(v.len()));
        v.push_str("..");
    }
    let vw = measure_text_mono(&v, FONT_DROPDOWN_BODY, false);
    draw_text_mono(
        canvas,
        value_x - vw,
        cy,
        &v,
        FONT_DROPDOWN_BODY,
        val_color,
        false,
    );
}
