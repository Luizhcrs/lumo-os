//! bar/dropdowns/wifi.rs - Dropdown wifi + struct WifiInfo + dbm/iface helpers.
//!
//! Layout (A23):
//!   y0  Wi-Fi (title bold)
//!   y1  SSID - 78% (medium)   OU   "Desconectado"
//!   sep
//!   y2  IP:         192.168.0.106
//!   y3  Sinal:      -52 dBm
//!   y4  Frequencia: 5 GHz
//!   y5  Velocidade: 433 Mbps

use lumo_foundation::LumoColors;
use tiny_skia::{Paint, PixmapMut, Rect, Transform};

use crate::bar::fonts::{draw_text, draw_text_mono, measure_text_mono, opaque, rgba_hex};
use crate::bar::icons::fill_rrect;
use crate::bar::tokens::*;

// ============================================================
// WifiInfo - leitura via iw dev <iface> link + ip -4 -o addr + sysfs (A23).
// ============================================================

#[derive(Clone, Default, Debug)]
pub struct WifiInfo {
    pub up: bool,
    pub ssid: Option<String>,
    pub signal_dbm: Option<i32>,
    pub signal_pct: Option<u8>,
    pub freq_ghz: Option<f32>,
    pub bitrate_mbps: Option<u32>,
    pub ip: Option<String>,
    pub iface: Option<String>,
}

/// Procura primeira interface wireless (wl*) com operstate=up.
pub fn find_wifi_iface() -> Option<String> {
    let entries = std::fs::read_dir("/sys/class/net").ok()?;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.starts_with("wl") {
            continue;
        }
        let op = std::fs::read_to_string(e.path().join("operstate"))
            .unwrap_or_default();
        if op.trim() == "up" {
            return Some(name);
        }
    }
    None
}

/// Converte dBm em percentual usando rampa linear simples 100..0 em -50..-100.
pub fn dbm_to_pct(dbm: i32) -> u8 {
    if dbm >= -50 {
        100
    } else if dbm <= -100 {
        0
    } else {
        ((dbm + 100) * 2).clamp(0, 100) as u8
    }
}

// ============================================================
// draw_wifi_dropdown (A23).
// ============================================================

pub fn draw_wifi_dropdown(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    palette: &LumoColors,
    info: &WifiInfo,
) {
    let bg = rgba_hex(palette.pill_bg, palette.pill_bg_alpha);
    let fg = opaque(palette.pill_fg);
    let fg_subtle = rgba_hex(palette.pill_fg, 0xA0);
    let sep_color = rgba_hex(palette.pill_sep, palette.pill_sep_alpha);

    fill_rrect(canvas, x, y, w, h, PILL_RADIUS, bg);

    let cx = x + DROPDOWN_PAD;
    let mut cy = y + DROPDOWN_PAD;

    // Title "Wi-Fi" 14px bold.
    draw_text(canvas, cx, cy, "Wi-Fi", FONT_DROPDOWN_TITLE, fg, true);
    cy += FONT_DROPDOWN_TITLE * 1.4;

    if !info.up || info.ssid.is_none() {
        // Estado desconectado.
        draw_text(canvas, cx, cy, "Desconectado", FONT_DROPDOWN_BODY, fg_subtle, false);
        cy += FONT_DROPDOWN_BODY * 1.6;
        if let Some(rect) = Rect::from_xywh(x + DROPDOWN_PAD, cy.round(), w - DROPDOWN_PAD * 2.0, 1.0) {
            let mut p = Paint::default();
            p.set_color(sep_color);
            p.anti_alias = false;
            canvas.fill_rect(rect, &p, Transform::identity(), None);
        }
        cy += 8.0;
        draw_text(canvas, cx, cy, "Sem rede ativa", FONT_DROPDOWN_BODY, fg_subtle, false);
        return;
    }

    // Linha 2: "SSID - signal%".
    let ssid = info.ssid.as_deref().unwrap_or("-");
    let pct_str = info
        .signal_pct
        .map(|p| format!(" - {}%", p))
        .unwrap_or_default();
    let summary = format!("{}{}", ssid, pct_str);
    let mut s = summary.clone();
    if s.chars().count() > 28 {
        s.truncate(s.char_indices().nth(26).map(|(i, _)| i).unwrap_or(s.len()));
        s.push_str("..");
    }
    draw_text(canvas, cx, cy, &s, FONT_DROPDOWN_BODY, fg_subtle, false);
    cy += FONT_DROPDOWN_BODY * 1.6;

    // Separator 1px.
    if let Some(rect) = Rect::from_xywh(x + DROPDOWN_PAD, cy.round(), w - DROPDOWN_PAD * 2.0, 1.0) {
        let mut p = Paint::default();
        p.set_color(sep_color);
        p.anti_alias = false;
        canvas.fill_rect(rect, &p, Transform::identity(), None);
    }
    cy += 8.0;

    // Rows key:value (4 linhas).
    let value_x = x + w - DROPDOWN_PAD;
    let rows: [(&str, String); 4] = [
        ("IP", info.ip.clone().unwrap_or_else(|| "-".into())),
        (
            "Sinal",
            info.signal_dbm.map(|d| format!("{} dBm", d)).unwrap_or_else(|| "-".into()),
        ),
        (
            "Frequencia",
            info.freq_ghz.map(|f| format!("{} GHz", f)).unwrap_or_else(|| "-".into()),
        ),
        (
            "Velocidade",
            info.bitrate_mbps.map(|b| format!("{} Mbps", b)).unwrap_or_else(|| "-".into()),
        ),
    ];
    for (key, value) in rows.iter() {
        draw_text(canvas, cx, cy, key, FONT_DROPDOWN_BODY, fg_subtle, false);
        let mut v = value.clone();
        if v.chars().count() > 22 {
            v.truncate(v.char_indices().nth(20).map(|(i, _)| i).unwrap_or(v.len()));
            v.push_str("..");
        }
        // A29: valor (IP, dBm, GHz, Mbps) = Geist Mono (alinhamento tabular).
        let vw = measure_text_mono(&v, FONT_DROPDOWN_BODY, false);
        draw_text_mono(canvas, value_x - vw, cy, &v, FONT_DROPDOWN_BODY, fg, false);
        cy += DROPDOWN_ROW_H;
    }
}
