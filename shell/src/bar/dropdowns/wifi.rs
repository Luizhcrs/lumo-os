//! bar/dropdowns/wifi.rs - Dropdown wifi (gerenciador de redes A31).
//!
//! Layout A31 (substitui A23 key:value dBm/freq/speed):
//!
//!   +-------------------------------+
//!   | Wi-Fi             [Toggle ON] |
//!   | v BBG_ERICA_5G        100%    |
//!   |                               |
//!   | Outras redes                  |
//!   | > VIVO_5G_NEIGHBOR     78%    |
//!   | > NET_VIRTUA           65%    |
//!   | > FREE_WIFI            45%    |
//!   |                               |
//!   | Conectar a outra rede...      |
//!   +-------------------------------+
//!
//! - Rede atual: prefix "v" (check), bold, fg full
//! - Outras: prefix ">", fg_subtle, % sinal direita Mono
//! - Toggle wifi: pill visual no header right (MVP so visual, sem callback real)
//! - Footer: "Conectar a outra rede..." placeholder (A31.2 future)
//!
//! Memory feedback_design_lapidado: row height 22 = font 13 + 9 padding
//! vertical = aria de click confortavel sem inflar dropdown. Max 6 redes
//! visiveis (truncate apos sort por signal desc).

use lumo_foundation::{LumoColors, i18n::I18n};
use tiny_skia::{Paint, PixmapMut, Rect, Transform};

use crate::bar::fonts::{draw_text, draw_text_mono, measure_text, measure_text_mono, opaque, rgba_hex};
use crate::bar::icons::{fill_rrect, stroke_rrect};
use crate::bar::tokens::*;

// ============================================================
// WifiNetwork - linha individual da lista (A31).
// ============================================================

#[derive(Clone, Default, Debug)]
pub struct WifiNetwork {
    pub ssid: String,
    pub signal_pct: u8,
    pub secured: bool,
    /// True se eh a rede atualmente conectada (IN-USE=*).
    pub connected: bool,
}

/// A31.2: hit-rects retornados pelo draw pra input dispatch.
#[derive(Default, Clone, Debug)]
pub struct WifiHits {
    pub toggle_rect: Option<(f32, f32, f32, f32)>,
    pub disconnect_rect: Option<(f32, f32, f32, f32)>,
    /// (ssid, rect) por rede listada em "Outras redes".
    pub connect_rects: Vec<(String, (f32, f32, f32, f32))>,
}

// ============================================================
// WifiInfo - leitura via iw + ip + nmcli scan (A23 + A31).
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
    /// A31: redes proximas (sorted by signal desc, max 32).
    pub networks: Vec<WifiNetwork>,
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
// draw_wifi_dropdown (A31 redesign - gerenciador redes).
// ============================================================

/// Max redes "outras" visiveis na lista. 6 cabe sem scroll, redes mais
/// fracas que isso usualmente nao conectam de qualquer jeito (signal<30%).
const MAX_OTHER_NETWORKS: usize = 6;

pub fn draw_wifi_dropdown(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    palette: &LumoColors,
    info: &WifiInfo,
) -> WifiHits {
    let mut hits = WifiHits::default();
    let bg = rgba_hex(palette.pill_bg, palette.pill_bg_alpha);
    let fg = opaque(palette.pill_fg);
    let fg_subtle = rgba_hex(palette.pill_fg, 0xA0);
    let fg_dim = rgba_hex(palette.pill_fg, 0x60);
    let accent = opaque(palette.accent);

    fill_rrect(canvas, x, y, w, h, PILL_RADIUS, bg);

    let pad = DROPDOWN_PAD;
    let cx = x + pad;
    let value_x = x + w - pad;

    // ============================================================
    // Header: "Wi-Fi" + toggle pill direita (A31 - MVP visual so).
    // ============================================================
    let mut cy = y + pad;
    draw_text(canvas, cx, cy, &I18n::get("wifi.title"), FONT_DROPDOWN_TITLE, fg, true);

    // Toggle pill: 36x18 (capsule switch), arredondamento total = capsule.
    // Estado: on = wifi up, off = down. Visual only (TODO A31 hookup nmcli radio).
    let toggle_w = 36.0;
    let toggle_h = 18.0;
    let toggle_x = value_x - toggle_w;
    let toggle_y = cy + (FONT_DROPDOWN_TITLE - toggle_h) / 2.0 + 1.0;
    let toggle_on = info.up;
    let knob_r = (toggle_h - 4.0) / 2.0;

    // Trilho.
    let trail_color = if toggle_on {
        accent
    } else {
        rgba_hex(palette.pill_sep, 0xC0)
    };
    fill_rrect(canvas, toggle_x, toggle_y, toggle_w, toggle_h, toggle_h / 2.0, trail_color);
    // A31.2: hit-area do toggle (toda capsule inclui knob).
    hits.toggle_rect = Some((toggle_x, toggle_y, toggle_w, toggle_h));

    // Knob branco (sempre claro, contraste). Slide direita = on, esquerda = off.
    let knob_cx = if toggle_on {
        toggle_x + toggle_w - knob_r - 2.0
    } else {
        toggle_x + knob_r + 2.0
    };
    let knob_cy = toggle_y + toggle_h / 2.0;
    crate::bar::icons::fill_circle(canvas, knob_cx, knob_cy, knob_r, opaque(0xFFFFFF));

    cy += FONT_DROPDOWN_TITLE * 1.6;

    // ============================================================
    // Conteudo: depende de up/down.
    // ============================================================
    if !info.up {
        draw_text(canvas, cx, cy, "Wi-Fi desligado", FONT_DROPDOWN_BODY, fg_subtle, false);
        return hits;
    }

    // ============================================================
    // Rede conectada (se houver).
    // ============================================================
    let connected_net = info.networks.iter().find(|n| n.connected);
    let connected_ssid = info.ssid.as_deref();

    if let Some(ssid) = connected_ssid {
        // A31.2: row click = disconnect.
        hits.disconnect_rect = Some((x + pad / 2.0, cy - 4.0, w - pad, DROPDOWN_WIFI_ROW_H));
        // Linha: "v SSID    pct%"
        let prefix = "v ";
        draw_text(canvas, cx, cy, prefix, FONT_DROPDOWN_BODY, accent, true);
        let prefix_w = measure_text(prefix, FONT_DROPDOWN_BODY, true);

        // SSID truncado pra deixar espaco pro pct.
        let s = truncate_ssid(ssid, 22);
        draw_text(canvas, cx + prefix_w, cy, &s, FONT_DROPDOWN_BODY, fg, false);

        // Pct: prioriza signal_pct do iw (mais preciso pra rede atual);
        // fallback connected_net (vem do nmcli).
        let pct = info
            .signal_pct
            .or(connected_net.map(|n| n.signal_pct))
            .unwrap_or(0);
        let pct_str = format!("{}%", pct);
        let vw = measure_text_mono(&pct_str, FONT_DROPDOWN_BODY, false);
        draw_text_mono(canvas, value_x - vw, cy, &pct_str, FONT_DROPDOWN_BODY, fg, false);
        cy += DROPDOWN_WIFI_ROW_H;
    } else {
        draw_text(canvas, cx, cy, &I18n::get("wifi.disconnected"), FONT_DROPDOWN_BODY, fg_subtle, false);
        cy += DROPDOWN_WIFI_ROW_H;
    }

    // Spacer + label "Outras redes".
    cy += 6.0;

    // ============================================================
    // Outras redes.
    // ============================================================
    let others: Vec<&WifiNetwork> = info
        .networks
        .iter()
        .filter(|n| !n.connected && !n.ssid.is_empty())
        .take(MAX_OTHER_NETWORKS)
        .collect();

    if !others.is_empty() {
        draw_text(canvas, cx, cy, "Outras redes", FONT_DROPDOWN_BODY, fg_dim, false);
        cy += FONT_DROPDOWN_BODY * 1.5;

        for net in &others {
            // A31.2: row click = connect a essa rede.
            hits.connect_rects.push((
                net.ssid.clone(),
                (x + pad / 2.0, cy - 4.0, w - pad, DROPDOWN_WIFI_ROW_H),
            ));
            // Prefix ">" subtle pra outras (nao bold).
            let prefix = "> ";
            draw_text(canvas, cx, cy, prefix, FONT_DROPDOWN_BODY, fg_dim, false);
            let prefix_w = measure_text(prefix, FONT_DROPDOWN_BODY, false);

            let s = truncate_ssid(&net.ssid, 22);
            draw_text(canvas, cx + prefix_w, cy, &s, FONT_DROPDOWN_BODY, fg_subtle, false);

            let pct_str = format!("{}%", net.signal_pct);
            let vw = measure_text_mono(&pct_str, FONT_DROPDOWN_BODY, false);
            draw_text_mono(canvas, value_x - vw, cy, &pct_str, FONT_DROPDOWN_BODY, fg_subtle, false);
            cy += DROPDOWN_WIFI_ROW_H;
        }
    }

    // ============================================================
    // Separator + footer "Conectar a outra rede..." (A31.2 placeholder).
    // ============================================================
    cy += 4.0;
    if let Some(rect) = Rect::from_xywh(x + pad, cy.round(), w - pad * 2.0, 1.0) {
        let mut p = Paint::default();
        p.set_color(rgba_hex(palette.pill_sep, palette.pill_sep_alpha));
        p.anti_alias = false;
        canvas.fill_rect(rect, &p, Transform::identity(), None);
    }
    cy += 8.0;

    // TODO A31.2: footer interativo com input de senha (precisa keyboard).
    draw_text(
        canvas,
        cx,
        cy,
        "Conectar a outra rede...",
        FONT_DROPDOWN_BODY,
        fg_dim,
        false,
    );

    // Suprime warning sobre h/stroke_rrect ate hover ser implementado em A31.2.
    let _ = h;
    let _ = stroke_rrect;
    hits
}

/// Trunca SSID preservando UTF-8 + adiciona ".." se passou de max_chars.
fn truncate_ssid(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars.saturating_sub(2)).collect();
    out.push_str("..");
    out
}
