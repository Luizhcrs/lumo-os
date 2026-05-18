//! lumo-bar - top bar Lumo OS via wlr-layer-shell + SHM + tiny-skia.
//!
//! A18 redesign: **pill-style flutuante** estilo Samsung Galaxy / iOS
//! Dynamic Island. Bar fundo TRANSPARENTE (alpha 0); 2 pills arredondadas
//! escuras semi-translucent com sombra preta neutra (sem accent glow,
//! memory `feedback_zero_neon_glow`).
//!
//! Layout (40px altura total, pills 28px com 6px margem topo):
//!
//!   +------------------------------------------------------+
//!   |  [== . Lumo . 1 ==]                [== ~ 82% 16:42 ==]|
//!   +------------------------------------------------------+
//!
//! Slots (cada constante justificada — `feedback_design_lapidado`):
//!   - Pill esquerda: brand dot 8px (accent) + " Lumo " + dot middle
//!     separator + numero workspace ativo (IPC).
//!   - Pill direita: wifi 16x16 + " 82% " + clock HH:MM mono Geist 13px.
//!     SEM data (compacta).
//!
//! Render pipeline:
//!   - wl_shm Argb8888 (alpha real necessario pra pills semi-translucent).
//!   - tiny-skia Pixmap (premul RGBA). Swap RGBA->BGRA pra wl_shm LE Argb8888.
//!   - Bar background `Color::TRANSPARENT` (compositor pinta atras).
//!   - Sombra: 4 rrects sobrepostos offset y=1..4 alpha decrescente
//!     (simula blur 4px sem shader GPU).
//!
//! Tipografia: Geist Mono / JetBrains Mono Nerd Font / monospace fallback.
//! Cosmic-text 0.12 + tiny-skia. Glyphs grayscale AA (sem rainbow subpixel).

use std::io::{ErrorKind, Read};
use std::os::unix::net::UnixStream;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc, Mutex, OnceLock,
};
use std::time::{Duration, Instant};
extern crate libc;

use chrono::{Datelike, Local, Timelike};
use cosmic_text::{
    Attrs, Buffer as CosmicBuffer, Color as CosmicColor, Family, FontSystem, Metrics, Shaping,
    SwashCache,
};
use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer, delegate_registry,
    delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        pointer::{PointerEvent, PointerEventKind, PointerHandler, ThemedPointer, BTN_LEFT},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, PixmapMut, Rect, Stroke, Transform};
use smithay_client_toolkit::reexports::client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};

use lumo_foundation::{current_colors, LumoColors, LumoTheme};
use lumo_ipc::{default_socket_path, LumoEvent, MAX_WORKSPACES};

// ============================================================
// Layout constants (lapidado: cada valor justificado).
// ============================================================

/// Altura total da bar (layer-shell exclusive zone).
/// 40px = 28px pill + 6px margin topo + 6px margem inferior (sombra cabe).
const BAR_HEIGHT: u32 = 40;

/// Altura de cada pill. 28px = padrao Apple Dynamic Island compact.
const PILL_H: f32 = 28.0;

/// Margem topo: distancia entre topo da bar e topo da pill.
/// 6px = respiro suficiente sem desperdicar real-estate.
const PILL_MARGIN_TOP: f32 = 6.0;

/// Margem lateral: distancia entre borda da bar e a pill.
/// 14px = mesmo PAD_X do design anterior (continuidade visual).
const PILL_MARGIN_X: f32 = 14.0;

/// Border-radius das pills. 16px = bem arredondado, pill-shape (28h / 2 = 14
/// daria capsule pura; 16 amacia mas mantem identidade pill).
const PILL_RADIUS: f32 = 14.0;

/// Padding horizontal interno da pill (entre borda da pill e conteudo).
/// 14px = Apple-grade respiracao.
const PILL_PAD_X: f32 = 14.0;

/// Gap entre items dentro da pill (icone/texto adjacentes).
/// 8px = denso mas legivel.
const PILL_GAP: f32 = 8.0;

/// Brand dot diametro 8px (radius 4). Atomo visual estavel.
const BRAND_DOT_RADIUS: f32 = 4.0;

/// Separator dot middle (entre items dentro da pill esquerda).
/// 4px diametro = sutil mas perceptivel.
const SEP_DOT_RADIUS: f32 = 2.0;

/// Font sizes (px). Conteudo de pill todo em 13px (compact uniform).
const FONT_PILL: f32 = 13.0;
const FONT_DATE: f32 = 13.0; // A19.14 igual clock

/// Wifi icone 16x16 (compact pra caber dentro de pill 28h).
const WIFI_SIZE: f32 = 16.0;

/// Bateria icone 14x8 (proporcional a 28h pill).
const BAT_BODY_W: f32 = 22.0; // A19.14 mais larga Mac-style
const BAT_BODY_H: f32 = 11.0;

// ============================================================
// Dropdown (A20).
// ============================================================
//
// Painel descendente abaixo da pill direita quando icone bat eh clicado.
// Largura 280 (>= pill direita), altura 200 (cabe 5 linhas key:value + header).
// Gap 6px abaixo da pill (respiro visual sem desconectar).
// Padding interno 14 igual PILL_PAD_X (continuidade).
const DROPDOWN_W: f32 = 280.0;
const DROPDOWN_H: f32 = 150.0; // A20.1 menor (3 rows)
const DROPDOWN_GAP: f32 = 6.0;
const DROPDOWN_PAD: f32 = 14.0;
const DROPDOWN_ROW_H: f32 = 18.0;
const FONT_DROPDOWN_TITLE: f32 = 14.0;
const FONT_DROPDOWN_BODY: f32 = 13.0;

// ============================================================
// Dropdown DateTime (A24).
// ============================================================
//
// Largura 280 igual bat/wifi (continuidade visual). Altura 252 acomoda:
//   - Linha 1: weekday + dia + mes (FONT_DROPDOWN_TITLE 14px)
//   - Linha 2: HH:MM:SS clock (FONT_DROPDOWN_CLOCK 22px)
//   - Separator linha
//   - Header weekdays D S T Q Q S S
//   - 6 linhas grid mes x 7 colunas
// Grid cell 32x22 (uniform 7 col * 32 = 224; centralizado em 280).
// Dia atual destacado pill emerald 22x18 radius 9 (sem glow neon).
const DROPDOWN_DATETIME_W: f32 = 280.0;
const DROPDOWN_DATETIME_H: f32 = 252.0;
const DATETIME_CELL_W: f32 = 32.0;
const DATETIME_CELL_H: f32 = 22.0;
const FONT_DROPDOWN_CLOCK: f32 = 22.0;
const FONT_DROPDOWN_CALENDAR: f32 = 12.0;

// ============================================================
// Color helpers.
// ============================================================

fn rgba_hex(hex: u32, alpha: u8) -> Color {
    let r = ((hex >> 16) & 0xff) as f32 / 255.0;
    let g = ((hex >> 8) & 0xff) as f32 / 255.0;
    let b = (hex & 0xff) as f32 / 255.0;
    let a = alpha as f32 / 255.0;
    Color::from_rgba(r, g, b, a).unwrap()
}

fn opaque(hex: u32) -> Color {
    rgba_hex(hex, 0xff)
}

/// tiny-skia Color -> cosmic-text Color (RGBA, sem premul).
fn to_cosmic(c: Color) -> CosmicColor {
    let r = (c.red() * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = (c.green() * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = (c.blue() * 255.0).round().clamp(0.0, 255.0) as u8;
    let a = (c.alpha() * 255.0).round().clamp(0.0, 255.0) as u8;
    CosmicColor::rgba(r, g, b, a)
}

// ============================================================
// FontSystem singleton + SwashCache.
// ============================================================

static FONT_SYSTEM: OnceLock<Mutex<FontSystem>> = OnceLock::new();
static SWASH_CACHE: OnceLock<Mutex<SwashCache>> = OnceLock::new();
static FONT_FAMILY: OnceLock<String> = OnceLock::new();

fn font_system() -> &'static Mutex<FontSystem> {
    FONT_SYSTEM.get_or_init(|| {
        let mut fs = FontSystem::new();
        load_extra_fonts(&mut fs);
        let family = pick_font_family(&fs);
        eprintln!("[lumo-bar] font_family escolhida = {}", family);
        let _ = FONT_FAMILY.set(family);
        Mutex::new(fs)
    })
}

fn swash_cache() -> &'static Mutex<SwashCache> {
    SWASH_CACHE.get_or_init(|| Mutex::new(SwashCache::new()))
}

fn load_extra_fonts(fs: &mut FontSystem) {
    let candidates = [
        std::env::var("HOME").ok().map(|h| format!("{}/.local/share/fonts", h)),
        std::env::var("HOME").ok().map(|h| format!("{}/.fonts", h)),
        Some("/usr/share/fonts/geist-mono".to_string()),
        Some("/usr/local/share/fonts".to_string()),
    ];
    for opt in candidates.iter().flatten() {
        if let Ok(entries) = std::fs::read_dir(opt) {
            for entry in entries.flatten() {
                let p = entry.path();
                let ext_ok = p
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(|e| {
                        let l = e.to_ascii_lowercase();
                        l == "ttf" || l == "otf"
                    })
                    .unwrap_or(false);
                if ext_ok {
                    let name = p.to_string_lossy().to_lowercase();
                    if name.contains("geist") || name.contains("jetbrains") {
                        fs.db_mut().load_font_file(&p).ok();
                    }
                }
            }
        }
    }
}

fn pick_font_family(fs: &FontSystem) -> String {
    let preferred = [
        "Geist Mono",
        "GeistMono Nerd Font",
        "JetBrainsMono Nerd Font",
        "JetBrains Mono",
        "JetBrainsMono Nerd Font Mono",
    ];
    let faces: Vec<String> = fs
        .db()
        .faces()
        .flat_map(|f| f.families.iter().map(|(n, _)| n.clone()).collect::<Vec<_>>())
        .collect();
    for p in preferred {
        if faces.iter().any(|f| f.eq_ignore_ascii_case(p)) {
            return p.to_string();
        }
    }
    for p in preferred {
        let pl = p.to_lowercase();
        let token = pl.split_whitespace().next().unwrap_or("monospace");
        if let Some(found) = faces.iter().find(|f| f.to_lowercase().contains(token)) {
            return found.clone();
        }
    }
    eprintln!("[lumo-bar] warning: nem Geist nem JetBrains Mono encontrada; usando monospace generico");
    "monospace".to_string()
}

fn current_family() -> &'static str {
    FONT_FAMILY.get().map(|s| s.as_str()).unwrap_or("monospace")
}

// ============================================================
// Vector primitives.
// ============================================================

fn fill_circle(canvas: &mut PixmapMut, cx: f32, cy: f32, r: f32, color: Color) {
    let path = match PathBuilder::from_circle(cx.round(), cy.round(), r) {
        Some(p) => p,
        None => return,
    };
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
}

fn fill_rrect(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    let x = x.round();
    let y = y.round();
    let r = r.min(w / 2.0).min(h / 2.0);
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
    let path = match pb.finish() {
        Some(p) => p,
        None => return,
    };
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
}

fn stroke_rrect(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    color: Color,
    sw: f32,
) {
    let x = x.round();
    let y = y.round();
    let r = r.min(w / 2.0).min(h / 2.0);
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
    let path = match pb.finish() {
        Some(p) => p,
        None => return,
    };
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    let st = Stroke {
        width: sw,
        ..Default::default()
    };
    canvas.stroke_path(&path, &p, &st, Transform::identity(), None);
}

fn stroke_arc(
    canvas: &mut PixmapMut,
    cx: f32,
    cy: f32,
    r: f32,
    start_deg: f32,
    end_deg: f32,
    color: Color,
    sw: f32,
) {
    let to_rad = |d: f32| d.to_radians();
    let p0 = (cx + r * to_rad(start_deg).cos(), cy + r * to_rad(start_deg).sin());
    let p1 = (cx + r * to_rad(end_deg).cos(), cy + r * to_rad(end_deg).sin());
    let mid = (start_deg + end_deg) * 0.5;
    let delta = (end_deg - start_deg).abs().to_radians();
    let k = ((delta / 2.0).cos()).max(0.0001);
    let r_ctl = r / k;
    let ctrl = (cx + r_ctl * to_rad(mid).cos(), cy + r_ctl * to_rad(mid).sin());

    let mut pb = PathBuilder::new();
    pb.move_to(p0.0, p0.1);
    pb.quad_to(ctrl.0, ctrl.1, p1.0, p1.1);
    let path = match pb.finish() {
        Some(p) => p,
        None => return,
    };
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    let st = Stroke {
        width: sw,
        line_cap: tiny_skia::LineCap::Round,
        ..Default::default()
    };
    canvas.stroke_path(&path, &p, &st, Transform::identity(), None);
}

// ============================================================
// Text rendering.
// ============================================================

fn measure_text(text: &str, size: f32, bold: bool) -> f32 {
    let mut fs = font_system().lock().expect("font_system poisoned");
    let metrics = Metrics::new(size, size * 1.2);
    let mut buffer = CosmicBuffer::new(&mut fs, metrics);
    let family = current_family().to_string();
    let mut attrs = Attrs::new().family(Family::Name(&family));
    if bold {
        attrs = attrs.weight(cosmic_text::Weight::BOLD);
    }
    buffer.set_text(&mut fs, text, attrs, Shaping::Advanced);
    buffer.set_size(&mut fs, Some(f32::INFINITY), Some(size * 1.4));
    buffer.shape_until_scroll(&mut fs, false);

    let mut w = 0.0f32;
    for run in buffer.layout_runs() {
        for glyph in run.glyphs.iter() {
            let r = glyph.x + glyph.w;
            if r > w {
                w = r;
            }
        }
    }
    w.ceil()
}

fn draw_text(
    canvas: &mut PixmapMut,
    x: f32,
    y_top: f32,
    text: &str,
    size: f32,
    color: Color,
    bold: bool,
) -> f32 {
    let mut fs = font_system().lock().expect("font_system poisoned");
    let mut cache = swash_cache().lock().expect("swash_cache poisoned");
    let metrics = Metrics::new(size, size * 1.2);
    let mut buffer = CosmicBuffer::new(&mut fs, metrics);
    let family = current_family().to_string();
    let mut attrs = Attrs::new().family(Family::Name(&family));
    if bold {
        attrs = attrs.weight(cosmic_text::Weight::BOLD);
    }
    buffer.set_text(&mut fs, text, attrs, Shaping::Advanced);
    buffer.set_size(&mut fs, Some(f32::INFINITY), Some(size * 1.4));
    buffer.shape_until_scroll(&mut fs, false);

    let cosmic_color = to_cosmic(color);
    let mut max_w = 0.0f32;

    buffer.draw(&mut fs, &mut cache, cosmic_color, |gx, gy, gw, gh, gcolor| {
        if gw == 0 || gh == 0 {
            return;
        }
        let a_mask = gcolor.a() as f32 / 255.0;
        if a_mask < 0.01 {
            return;
        }
        let c = Color::from_rgba(
            color.red(),
            color.green(),
            color.blue(),
            color.alpha() * a_mask,
        ).unwrap_or(color);
        let px = (x + gx as f32).round();
        let py = (y_top + gy as f32).round();
        if let Some(rect) = Rect::from_xywh(px, py, gw as f32, gh as f32) {
            let mut p = Paint::default();
            p.set_color(c);
            p.anti_alias = false;
            canvas.fill_rect(rect, &p, Transform::identity(), None);
        }
        let edge = gx as f32 + gw as f32;
        if edge > max_w {
            max_w = edge;
        }
    });
    max_w
}

// ============================================================
// Pill primitive.
// ============================================================
//
// `draw_pill_bg` pinta:
//   1) sombra: 4 rrects empilhados (y offset 1..4), alpha decrescente
//      (40 -> 10) -> simula blur 4px sem shader GPU.
//   2) pill bg fill rounded com cor + alpha do tema.
//
// Sem accent glow (memory feedback_zero_neon_glow). Sombra preta neutra.

fn draw_pill_bg(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    bg: Color,
    shadow_alpha: u8,
) {
    // Sombra: 4 rrects empilhados, offset y crescente, alpha decrescente.
    // Cada camada simula um "anel" do blur gaussiano discretizado.
    let base = shadow_alpha as f32;
    let layers: [(f32, f32, f32); 4] = [
        // (dy, dx_expand, alpha_factor)
        (1.0, 0.0, 1.0),   // mais perto, mais opaco
        (2.0, 0.5, 0.65),
        (3.0, 1.0, 0.35),
        (4.0, 1.5, 0.15),
    ];
    for (dy, expand, factor) in layers {
        let a = (base * factor).round().clamp(0.0, 255.0) as u8;
        if a == 0 {
            continue;
        }
        let shadow_color = rgba_hex(0x000000, a);
        fill_rrect(
            canvas,
            x - expand,
            y + dy,
            w + expand * 2.0,
            h,
            PILL_RADIUS,
            shadow_color,
        );
    }
    // Pill background.
    fill_rrect(canvas, x, y, w, h, PILL_RADIUS, bg);
}

// ============================================================
// Wifi glyph (compact 16px).
// ============================================================
fn draw_wifi(canvas: &mut PixmapMut, x: f32, y: f32, on: bool, fg: Color, fg_subtle: Color) {
    let color = if on { fg } else { fg_subtle };
    let s = WIFI_SIZE;
    let cx = x + s / 2.0;
    let cy = y + s * 0.78;
    let arcs = [
        (s * 0.46, s * 0.085),
        (s * 0.30, s * 0.075),
        (s * 0.155, s * 0.07),
    ];
    for (radius, sw) in arcs {
        stroke_arc(canvas, cx, cy, radius, -135.0, -45.0, color, sw);
    }
    fill_circle(canvas, cx, cy, s * 0.06, color);
}

// ============================================================
// Battery glyph (compact 14x8).
// ============================================================
fn draw_battery(canvas: &mut PixmapMut, x: f32, y: f32, pct: u8, fg: Color, accent: Color) {
    let body_w = BAT_BODY_W;
    let body_h = BAT_BODY_H;
    stroke_rrect(canvas, x + 0.5, y + 0.5, body_w - 1.0, body_h - 1.0, 2.2, fg, 1.2);
    fill_rrect(canvas, x + body_w + 0.8, y + body_h * 0.28, 2.0, body_h * 0.44, 0.8, fg);
    // A19.14: bateria Mac-style refinada (22x11 body, inset 2px = fill cheio e centralizado)
    let inset_x = 2.0f32;
    let inset_y = 2.0f32;
    let inner_w = body_w - inset_x * 2.0;
    let inner_h = body_h - inset_y * 2.0;
    let fw = (pct as f32 / 100.0).clamp(0.0, 1.0) * inner_w;
    if fw > 0.5 {
        let fill_color = if pct >= 50 {
            opaque(0xF5F5F7) // branco pearl Mac cheio
        } else if pct >= 20 {
            opaque(0xFB923C) // orange-400 medio
        } else {
            opaque(0xEF4444) // red-500 baixo
        };
        let _ = accent;
        fill_rrect(canvas, x + inset_x, y + inset_y, fw, inner_h, 1.2, fill_color);
    }
}

fn battery_total_width() -> f32 {
    BAT_BODY_W + 2.5
}

// ============================================================
// Dropdown state (A20).
// ============================================================

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum DropdownActive {
    None,
    Battery,
    Wifi,     // A23
    DateTime, // A24 - calendario + hora detalhada
}

// ============================================================
// BatteryInfo - leitura completa /sys/class/power_supply/BAT0
// ============================================================

#[derive(Clone, Default, Debug)]
pub struct BatteryInfo {
    pub pct: u8,
    pub status: String,
    pub cycles: Option<u32>,
    // Algumas baterias expoem energy_* (mWh), outras charge_* (mAh).
    // Normalizamos pra "full"/"now"/"full_design" + "power_now" (mW
    // equivalente) usando voltage_now quando charge_*.
    pub full: Option<u32>,         // mWh
    pub now: Option<u32>,          // mWh
    pub full_design: Option<u32>,  // mWh
    pub power_now: Option<u32>,    // mW
    pub voltage_now_mv: Option<u32>,
    pub model: Option<String>,
    pub manufacturer: Option<String>,
}

fn sys_read_string(path: &str) -> Option<String> {
    std::fs::read_to_string(path)
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn sys_read_u32(path: &str) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse::<u32>().ok())
}

fn read_battery_info() -> BatteryInfo {
    let mut dir = String::new();
    for bat in &["BAT0", "BAT1", "BAT2"] {
        let p = format!("/sys/class/power_supply/{}", bat);
        if std::path::Path::new(&format!("{}/capacity", p)).exists() {
            dir = p;
            break;
        }
    }
    if dir.is_empty() {
        return BatteryInfo {
            pct: 100,
            status: "Sem bateria".to_string(),
            ..Default::default()
        };
    }
    let voltage_uv = sys_read_u32(&format!("{}/voltage_now", dir));
    let voltage_mv = voltage_uv.map(|v| v / 1000);

    // Tenta energy_* (mWh, microWh dividido). Fallback charge_* (uAh)
    // convertendo via voltage_mv (mV) -> mWh = mAh * V = mAh * mV / 1000.
    let energy_now = sys_read_u32(&format!("{}/energy_now", dir));
    let energy_full = sys_read_u32(&format!("{}/energy_full", dir));
    let energy_full_design = sys_read_u32(&format!("{}/energy_full_design", dir));
    let power_now_uw = sys_read_u32(&format!("{}/power_now", dir));

    let charge_now = sys_read_u32(&format!("{}/charge_now", dir));
    let charge_full = sys_read_u32(&format!("{}/charge_full", dir));
    let charge_full_design = sys_read_u32(&format!("{}/charge_full_design", dir));
    let current_now_ua = sys_read_u32(&format!("{}/current_now", dir));

    // Convert uWh -> mWh (divide 1000); uAh + mV -> uWh -> mWh.
    fn uwh_to_mwh(uwh: Option<u32>) -> Option<u32> { uwh.map(|v| v / 1000) }
    fn uah_to_mwh(uah: Option<u32>, mv: Option<u32>) -> Option<u32> {
        let a = uah? as u64;
        let v = mv? as u64;
        // uAh * mV = nWh; / 1_000_000 = mWh.
        Some(((a * v) / 1_000_000) as u32)
    }

    let full = uwh_to_mwh(energy_full).or_else(|| uah_to_mwh(charge_full, voltage_mv));
    let now = uwh_to_mwh(energy_now).or_else(|| uah_to_mwh(charge_now, voltage_mv));
    let full_design = uwh_to_mwh(energy_full_design)
        .or_else(|| uah_to_mwh(charge_full_design, voltage_mv));
    let power_now = uwh_to_mwh(power_now_uw)
        .or_else(|| uah_to_mwh(current_now_ua, voltage_mv));

    BatteryInfo {
        pct: sys_read_u32(&format!("{}/capacity", dir)).unwrap_or(100) as u8,
        status: sys_read_string(&format!("{}/status", dir)).unwrap_or_else(|| "?".into()),
        cycles: sys_read_u32(&format!("{}/cycle_count", dir)),
        full,
        now,
        full_design,
        power_now,
        voltage_now_mv: voltage_mv,
        model: sys_read_string(&format!("{}/model_name", dir)),
        manufacturer: sys_read_string(&format!("{}/manufacturer", dir)),
    }
}

// ============================================================
// DateTimeInfo (A24) - calendario + hora detalhada.
// ============================================================

#[derive(Clone)]
pub struct DateTimeInfo {
    pub weekday_full: String, // "domingo"
    pub day: u32,             // 17
    pub month_full: String,   // "maio"
    pub year: i32,            // 2026
    pub hour: u8,             // 17
    pub minute: u8,           // 50
    pub second: u8,           // 32
    pub month_grid: Vec<Vec<Option<u32>>>, // 6 weeks x 7 days, None = padding
    pub today_day: u32,
}

impl Default for DateTimeInfo {
    fn default() -> Self {
        DateTimeInfo {
            weekday_full: String::new(),
            day: 1,
            month_full: String::new(),
            year: 2026,
            hour: 0,
            minute: 0,
            second: 0,
            month_grid: vec![vec![None; 7]; 6],
            today_day: 1,
        }
    }
}

fn weekday_full_pt(w: chrono::Weekday) -> &'static str {
    use chrono::Weekday::*;
    match w {
        Mon => "segunda-feira", Tue => "terca-feira", Wed => "quarta-feira",
        Thu => "quinta-feira", Fri => "sexta-feira", Sat => "sabado", Sun => "domingo",
    }
}

fn month_full_pt(m: u32) -> &'static str {
    match m {
        1 => "janeiro", 2 => "fevereiro", 3 => "marco", 4 => "abril",
        5 => "maio", 6 => "junho", 7 => "julho", 8 => "agosto",
        9 => "setembro", 10 => "outubro", 11 => "novembro", 12 => "dezembro",
        _ => "?",
    }
}

/// Constroi grid 6x7 do mes. Coluna 0 = Domingo (padrao PT-BR D S T Q Q S S).
fn month_grid_for(year: i32, month: u32) -> Vec<Vec<Option<u32>>> {
    use chrono::{Datelike, NaiveDate};
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    // num_days_from_sunday: Sun=0 .. Sat=6 (alinha com nosso layout D S T Q Q S S).
    let first_weekday = first.weekday().num_days_from_sunday() as usize;
    let next_month_first = if month == 12 {
        NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
    } else {
        NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
    };
    let days_in_month = next_month_first.pred_opt().unwrap().day();

    let mut grid = vec![vec![None; 7]; 6];
    let mut day = 1u32;
    for week in 0..6 {
        for col in 0..7 {
            if (week == 0 && col < first_weekday) || day > days_in_month {
                grid[week][col] = None;
            } else {
                grid[week][col] = Some(day);
                day += 1;
            }
        }
    }
    grid
}

fn read_datetime_info() -> DateTimeInfo {
    let now = Local::now();
    DateTimeInfo {
        weekday_full: weekday_full_pt(now.weekday()).to_string(),
        day: now.day(),
        month_full: month_full_pt(now.month()).to_string(),
        year: now.year(),
        hour: now.hour() as u8,
        minute: now.minute() as u8,
        second: now.second() as u8,
        month_grid: month_grid_for(now.year(), now.month()),
        today_day: now.day(),
    }
}

/// Status traduzido PT-BR.
fn status_pt(s: &str) -> &str {
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
fn battery_health(info: &BatteryInfo) -> Option<u8> {
    let full = info.full? as f32;
    let design = info.full_design? as f32;
    if design < 1.0 {
        return None;
    }
    Some(((full / design) * 100.0).round().clamp(0.0, 100.0) as u8)
}

/// Tempo restante string PT-BR ("2h 15min", "cheia", "-").
fn battery_time_left(info: &BatteryInfo) -> String {
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
//
// Layout:
//   y0  Bateria (title bold)
//   y1  100% . Carregando (medium)
//   sep
//   y2  Saude:      92%
//   y3  Ciclos:     142
//   y4  Tempo:      cheia
//   y5  Voltagem:   12.4 V
//   y6  Modelo:     Samsung SLA1NV2DR
//
// Mesma cor pill_bg (consistente). Sem accent glow (memory feedback_zero_neon_glow).
fn draw_battery_dropdown(
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
        let vw = measure_text(&v, FONT_DROPDOWN_BODY, false);
        draw_text(canvas, value_x - vw, cy, &v, FONT_DROPDOWN_BODY, fg, false);
        cy += DROPDOWN_ROW_H;
    }
}

// ============================================================
// WifiInfo - leitura via `iw dev <iface> link` + `ip -4 -o addr` + sysfs (A23).
// ============================================================
//
// Estrategia: sem dep extra (nl80211 crate pesa). Exec processos curtos
// `iw` e `ip` que ja sao trans dependency userland Arch. Parse stdout linha
// a linha. Falha qualquer = campo None, dropdown mostra "-".

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
fn find_wifi_iface() -> Option<String> {
    let entries = std::fs::read_dir("/sys/class/net").ok()?;
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().to_string();
        if !name.starts_with("wl") {
            continue;
        }
        // Filtra so wireless real: precisa ter subdir "wireless" OU
        // phy80211 (nl80211). wlan0 ok, mas evita confusao.
        let op = std::fs::read_to_string(e.path().join("operstate"))
            .unwrap_or_default();
        if op.trim() == "up" {
            return Some(name);
        }
    }
    None
}

/// Converte dBm em percentual usando rampa linear simples 100..0 em -50..-100.
fn dbm_to_pct(dbm: i32) -> u8 {
    if dbm >= -50 {
        100
    } else if dbm <= -100 {
        0
    } else {
        ((dbm + 100) * 2).clamp(0, 100) as u8
    }
}

/// Le info real do wifi via `iw dev <iface> link` + `ip -4 -o addr show <iface>`.
fn read_wifi_info() -> WifiInfo {
    let iface = match find_wifi_iface() {
        Some(n) => n,
        None => return WifiInfo::default(),
    };

    let mut info = WifiInfo {
        up: true,
        iface: Some(iface.clone()),
        ..Default::default()
    };

    // ---- iw dev <iface> link ----
    if let Ok(out) = std::process::Command::new("iw")
        .args(["dev", &iface, "link"])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            // Se "Not connected" => up mas sem rede ativa.
            if s.contains("Not connected") {
                info.up = false;
            } else {
                for raw in s.lines() {
                    let line = raw.trim();
                    if let Some(v) = line.strip_prefix("SSID:") {
                        info.ssid = Some(v.trim().to_string());
                    } else if let Some(v) = line.strip_prefix("freq:") {
                        // "freq: 5180" ou "freq: 5180.0"
                        let mhz: f32 = v.trim().parse().unwrap_or(0.0);
                        if mhz > 0.0 {
                            info.freq_ghz = Some((mhz / 1000.0 * 10.0).round() / 10.0);
                        }
                    } else if let Some(v) = line.strip_prefix("signal:") {
                        // "-49 dBm"
                        let tok = v.trim().split_whitespace().next().unwrap_or("");
                        if let Ok(d) = tok.parse::<i32>() {
                            info.signal_dbm = Some(d);
                            info.signal_pct = Some(dbm_to_pct(d));
                        }
                    } else if let Some(v) = line.strip_prefix("tx bitrate:") {
                        // "433.3 MBit/s VHT-MCS 9 ..."
                        let tok = v.trim().split_whitespace().next().unwrap_or("");
                        if let Ok(f) = tok.parse::<f32>() {
                            info.bitrate_mbps = Some(f.round() as u32);
                        }
                    }
                }
            }
        }
    }

    // ---- ip -4 -o addr show <iface> ----
    if let Ok(out) = std::process::Command::new("ip")
        .args(["-4", "-o", "addr", "show", &iface])
        .output()
    {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout);
            // Linha tipica: "3: wlan0    inet 192.168.0.106/24 brd ..."
            for line in s.lines() {
                if let Some(pos) = line.find("inet ") {
                    let rest = &line[pos + 5..];
                    let ip = rest
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .split('/')
                        .next()
                        .unwrap_or("")
                        .to_string();
                    if !ip.is_empty() {
                        info.ip = Some(ip);
                        break;
                    }
                }
            }
        }
    }

    eprintln!(
        "[lumo-bar] read_wifi_info: iface={:?} ssid={:?} dbm={:?} pct={:?} freq={:?} bitrate={:?} ip={:?}",
        info.iface, info.ssid, info.signal_dbm, info.signal_pct, info.freq_ghz, info.bitrate_mbps, info.ip
    );

    info
}

// ============================================================
// draw_wifi_dropdown (A23).
// ============================================================
//
// Layout (mesma largura/altura DROPDOWN_W/H = bateria; consistencia visual):
//
//   y0  Wi-Fi (title bold)
//   y1  SSID - 78% (medium)   OU   "Desconectado"
//   sep
//   y2  IP:         192.168.0.106
//   y3  Sinal:      -52 dBm
//   y4  Frequencia: 5 GHz
//   y5  Velocidade: 433 Mbps
//
// Se !info.up: so titulo + "Sem rede ativa" centralizado fg_subtle.
fn draw_wifi_dropdown(
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
    // Truncar SSID longo pra caber.
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
        let vw = measure_text(&v, FONT_DROPDOWN_BODY, false);
        draw_text(canvas, value_x - vw, cy, &v, FONT_DROPDOWN_BODY, fg, false);
        cy += DROPDOWN_ROW_H;
    }
}

// ============================================================
// draw_datetime_dropdown (A24).
// ============================================================
//
// Layout (DROPDOWN_DATETIME_W=280, _H=252):
//   pad 14 top
//   linha 1: "{weekday_full}, {day} de {month_full}" 14px bold
//   linha 2: "HH:MM:SS" 22px (FONT_DROPDOWN_CLOCK) realtime
//   separator linha 1px
//   header weekdays "D S T Q Q S S" 12px subtle, 7 colunas uniform
//   grid 6 linhas x 7 colunas, dia atual pill emerald solido
//
// Constantes justificadas (memory feedback_design_lapidado):
//   DATETIME_CELL_W=32 -> 7*32=224, centralizado em 280 sem PAD_X.
//   DATETIME_CELL_H=22 -> respiro vertical, 6 linhas = 132 + header 22 = 154.
// Sem glow neon (memory feedback_zero_neon_glow): pill emerald solido.
fn draw_datetime_dropdown(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    palette: &LumoColors,
    info: &DateTimeInfo,
) {
    let bg = rgba_hex(palette.pill_bg, palette.pill_bg_alpha);
    let fg = opaque(palette.pill_fg);
    let fg_subtle = rgba_hex(palette.pill_fg, 0xA0);
    let sep_color = rgba_hex(palette.pill_sep, palette.pill_sep_alpha);
    let accent = opaque(palette.accent);
    // FG sobre pill accent: branco (contraste forte, sem glow).
    let on_accent = opaque(0xFFFFFF);

    // Background rounded rect.
    fill_rrect(canvas, x, y, w, h, PILL_RADIUS, bg);

    let cx = x + DROPDOWN_PAD;
    let mut cy = y + DROPDOWN_PAD;

    // Linha 1: weekday + dia + mes (bold).
    let title = format!("{}, {} de {}", info.weekday_full, info.day, info.month_full);
    draw_text(canvas, cx, cy, &title, FONT_DROPDOWN_TITLE, fg, true);
    cy += FONT_DROPDOWN_TITLE * 1.5;

    // Linha 2: HH:MM:SS grande.
    let clock = format!("{:02}:{:02}:{:02}", info.hour, info.minute, info.second);
    draw_text(canvas, cx, cy, &clock, FONT_DROPDOWN_CLOCK, fg, false);
    cy += FONT_DROPDOWN_CLOCK * 1.3;

    // Separator 1px.
    if let Some(rect) = Rect::from_xywh(x + DROPDOWN_PAD, cy.round(), w - DROPDOWN_PAD * 2.0, 1.0) {
        let mut p = Paint::default();
        p.set_color(sep_color);
        p.anti_alias = false;
        canvas.fill_rect(rect, &p, Transform::identity(), None);
    }
    cy += 10.0;

    // Grid horizontal centralizado em w. 7 colunas * DATETIME_CELL_W.
    let grid_total_w = DATETIME_CELL_W * 7.0;
    let grid_x = x + (w - grid_total_w) / 2.0;

    // Header weekdays: D S T Q Q S S (col 0 = Dom).
    let weekday_labels = ["D", "S", "T", "Q", "Q", "S", "S"];
    for (i, label) in weekday_labels.iter().enumerate() {
        let cell_x = grid_x + DATETIME_CELL_W * i as f32;
        let label_w = measure_text(label, FONT_DROPDOWN_CALENDAR, true);
        let lx = cell_x + (DATETIME_CELL_W - label_w) / 2.0;
        draw_text(canvas, lx, cy, label, FONT_DROPDOWN_CALENDAR, fg_subtle, true);
    }
    cy += DATETIME_CELL_H;

    // Grid 6x7 dias.
    for week in 0..6 {
        for col in 0..7 {
            if let Some(day) = info.month_grid[week][col] {
                let cell_x = grid_x + DATETIME_CELL_W * col as f32;
                let cell_y = cy + DATETIME_CELL_H * week as f32;
                let is_today = day == info.today_day;

                let day_str = day.to_string();
                let day_w = measure_text(&day_str, FONT_DROPDOWN_CALENDAR, is_today);
                let dx = cell_x + (DATETIME_CELL_W - day_w) / 2.0;
                // Texto baseline alinha topo + folga pra centralizar dentro de 22.
                let dy = cell_y + (DATETIME_CELL_H - FONT_DROPDOWN_CALENDAR) / 2.0 - 1.0;

                if is_today {
                    // Pill emerald 22x18 radius 9 (sem glow).
                    let pill_w = 22.0;
                    let pill_h = 18.0;
                    let px = cell_x + (DATETIME_CELL_W - pill_w) / 2.0;
                    let py = cell_y + (DATETIME_CELL_H - pill_h) / 2.0;
                    fill_rrect(canvas, px, py, pill_w, pill_h, 9.0, accent);
                    draw_text(canvas, dx, dy, &day_str, FONT_DROPDOWN_CALENDAR, on_accent, true);
                } else {
                    draw_text(canvas, dx, dy, &day_str, FONT_DROPDOWN_CALENDAR, fg, false);
                }
            }
        }
    }

    let _ = h; // h passado pela API, background usa w via fill_rrect.
}

// ============================================================
// BarSnapshot.
// ============================================================
struct BarSnapshot {
    width: u32,
    height: u32,
    battery_pct: u8,
    wifi_on: bool,
    palette: LumoColors,
    theme: LumoTheme,
    clock_hh: u8,
    clock_mm: u8,
    active_ws: u8,
    date_str: String,
    dropdown: DropdownActive,
    battery_info: BatteryInfo,
    wifi_info: WifiInfo, // A23
    datetime_info: DateTimeInfo, // A24
}

/// Resultado de paint_frame: posicoes calculadas pra hit-test no proximo frame.
#[derive(Default, Clone)]
struct PaintResult {
    bat_hit_rect: Option<(f32, f32, f32, f32)>,
    wifi_hit_rect: Option<(f32, f32, f32, f32)>,     // A23
    datetime_hit_rect: Option<(f32, f32, f32, f32)>, // A24
    last_click_at: Option<Instant>,
}

// ============================================================
// paint_frame: pinta as 2 pills sobre fundo transparente.
// ============================================================
fn paint_frame(pixmap: &mut Pixmap, snap: &BarSnapshot) -> PaintResult {
    let palette = &snap.palette;
    // BAR BACKGROUND TRANSPARENTE (A18 — alpha 0). Compositor pinta atras.
    pixmap.fill(Color::TRANSPARENT);

    let mut result = PaintResult::default();
    let h = snap.height as f32;
    let pill_y = PILL_MARGIN_TOP;
    let pill_cy = pill_y + PILL_H / 2.0;

    // Cor pill bg: hex + alpha do tema. Mesma cor pra ambas as pills.
    let pill_bg = rgba_hex(palette.pill_bg, palette.pill_bg_alpha);
    let pill_fg = opaque(palette.pill_fg);
    let pill_fg_subtle = rgba_hex(palette.pill_fg, 0xB0); // 70% pra dim sobre pill
    let pill_sep = rgba_hex(palette.pill_sep, palette.pill_sep_alpha);
    let shadow_a = palette.pill_shadow_alpha;
    let accent = opaque(palette.accent);

    // Topo y do texto dentro da pill (centralizado vertical).
    let text_h = FONT_PILL * 1.2;
    let text_top = pill_y + (PILL_H - text_h) / 2.0;
    let text_top = text_top.round();

    // ============================================================
    // PILL ESQUERDA: [dot] Lumo . 1
    // ============================================================
    let workspace_str = snap.active_ws.to_string();
    let lumo_w = measure_text("Lumo", FONT_PILL, true);
    let ws_w = measure_text(&workspace_str, FONT_PILL, false);

    // Largura interna pill esquerda:
    //   pad + brand_dot(8) + gap + Lumo + gap + sep(4) + gap + ws + pad
    let pill_l_content_w =
        BRAND_DOT_RADIUS * 2.0
        + PILL_GAP
        + lumo_w
        + PILL_GAP
        + SEP_DOT_RADIUS * 2.0
        + PILL_GAP
        + ws_w;
    let pill_l_w = pill_l_content_w + PILL_PAD_X * 2.0;
    let pill_l_x = PILL_MARGIN_X;

    {
        let mut canvas = pixmap.as_mut();
        draw_pill_bg(&mut canvas, pill_l_x, pill_y, pill_l_w, PILL_H, pill_bg, 0);

        let mut cx = pill_l_x + PILL_PAD_X;
        // Brand dot (accent emerald/blue).
        fill_circle(&mut canvas, cx + BRAND_DOT_RADIUS, pill_cy, BRAND_DOT_RADIUS, accent);
        cx += BRAND_DOT_RADIUS * 2.0 + PILL_GAP;
        // "Lumo" bold.
        draw_text(&mut canvas, cx, text_top, "Lumo", FONT_PILL, pill_fg, true);
        cx += lumo_w + PILL_GAP;
        // Separator dot middle.
        fill_circle(&mut canvas, cx + SEP_DOT_RADIUS, pill_cy, SEP_DOT_RADIUS, pill_sep);
        cx += SEP_DOT_RADIUS * 2.0 + PILL_GAP;
        // Workspace numero.
        draw_text(&mut canvas, cx, text_top, &workspace_str, FONT_PILL, pill_fg, false);
    }

    // ============================================================
    // PILL DIREITA: [wifi] [bat icone] HH:MM (A19.8: removido texto %)
    // ============================================================
    let bat_icon_w = battery_total_width();
    let clock_s = format!("{:02}:{:02}", snap.clock_hh, snap.clock_mm);
    let clock_w = measure_text(&clock_s, FONT_PILL, false);

    let date_w = measure_text(&snap.date_str, FONT_DATE, false);
    let pill_r_content_w =
        bat_icon_w + PILL_GAP + WIFI_SIZE + PILL_GAP + date_w + 8.0 + clock_w;
    let pill_r_w = pill_r_content_w + PILL_PAD_X * 2.0;
    let pill_r_x = snap.width as f32 - PILL_MARGIN_X - pill_r_w;

    {
        let mut canvas = pixmap.as_mut();
        draw_pill_bg(&mut canvas, pill_r_x, pill_y, pill_r_w, PILL_H, pill_bg, 0);
        let mut cx = pill_r_x + PILL_PAD_X;
        // A19.10: ordem bat -> wifi -> data -> hora (Mac-style)
        // A20: salvar bat_hit_rect (cx atual + bat_icon_w, altura PILL_H pra click facil)
        let bat_x_start = cx;
        draw_battery(&mut canvas, cx, pill_cy - BAT_BODY_H / 2.0, snap.battery_pct, pill_fg, accent);
        // A20.13: hit area = SO o icone bateria (era pill inteira A20.4)
        // Y expande pra PILL_H pra facilitar click vertical sem precisar bater exato no icone.
        result.bat_hit_rect = Some((bat_x_start - 4.0, pill_y, bat_icon_w + 8.0, PILL_H));
        cx += bat_icon_w + PILL_GAP;
        // A23: salvar wifi_hit_rect igual bat (Y = PILL_H pra facilitar click).
        let wifi_x_start = cx;
        draw_wifi(&mut canvas, cx, pill_cy - WIFI_SIZE / 2.0, snap.wifi_on, pill_fg, pill_fg_subtle);
        result.wifi_hit_rect = Some((wifi_x_start - 4.0, pill_y, WIFI_SIZE + 8.0, PILL_H));
        cx += WIFI_SIZE + PILL_GAP;
        // A24: hit area cobre data + hora juntas (mesmo dropdown calendario).
        let datetime_x_start = cx;
        draw_text(&mut canvas, cx, text_top, &snap.date_str, FONT_DATE, pill_fg, false);
        cx += date_w + 8.0;
        draw_text(&mut canvas, cx, text_top, &clock_s, FONT_PILL, pill_fg, false);
        let datetime_end = cx + clock_w;
        result.datetime_hit_rect = Some((
            datetime_x_start - 4.0,
            pill_y,
            (datetime_end - datetime_x_start) + 8.0,
            PILL_H,
        ));
    }

    // ============================================================
    // DROPDOWN (A20/A23) — render abaixo da pill direita se ativo.
    // ============================================================
    match snap.dropdown {
        DropdownActive::Battery => {
            if let Some((rx, ry, rw, rh)) = result.bat_hit_rect {
                let want_x = rx + rw / 2.0 - DROPDOWN_W / 2.0;
                let max_x = snap.width as f32 - PILL_MARGIN_X - DROPDOWN_W;
                let dropdown_x = want_x.max(PILL_MARGIN_X).min(max_x.max(PILL_MARGIN_X));
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                let mut canvas = pixmap.as_mut();
                draw_battery_dropdown(
                    &mut canvas,
                    dropdown_x,
                    dropdown_y,
                    DROPDOWN_W,
                    DROPDOWN_H,
                    palette,
                    &snap.battery_info,
                );
            }
        }
        DropdownActive::Wifi => {
            if let Some((rx, ry, rw, rh)) = result.wifi_hit_rect {
                let want_x = rx + rw / 2.0 - DROPDOWN_W / 2.0;
                let max_x = snap.width as f32 - PILL_MARGIN_X - DROPDOWN_W;
                let dropdown_x = want_x.max(PILL_MARGIN_X).min(max_x.max(PILL_MARGIN_X));
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                let mut canvas = pixmap.as_mut();
                draw_wifi_dropdown(
                    &mut canvas,
                    dropdown_x,
                    dropdown_y,
                    DROPDOWN_W,
                    DROPDOWN_H,
                    palette,
                    &snap.wifi_info,
                );
            }
        }
        // A24: dropdown calendario+horario (mesmo painel pra click em data OU hora).
        DropdownActive::DateTime => {
            if let Some((rx, ry, rw, rh)) = result.datetime_hit_rect {
                let want_x = rx + rw / 2.0 - DROPDOWN_DATETIME_W / 2.0;
                let max_x = snap.width as f32 - PILL_MARGIN_X - DROPDOWN_DATETIME_W;
                let dropdown_x = want_x.max(PILL_MARGIN_X).min(max_x.max(PILL_MARGIN_X));
                let dropdown_y = ry + rh + DROPDOWN_GAP;
                let mut canvas = pixmap.as_mut();
                draw_datetime_dropdown(
                    &mut canvas,
                    dropdown_x,
                    dropdown_y,
                    DROPDOWN_DATETIME_W,
                    DROPDOWN_DATETIME_H,
                    palette,
                    &snap.datetime_info,
                );
            }
        }
        DropdownActive::None => {}
    }

    // Suppress unused warns nos campos do snapshot (theme so usado pra debug log).
    let _ = (snap.theme, h);
    result
}

// ============================================================
// Sensors.
// ============================================================

fn weekday_abbr_pt(d: chrono::Weekday) -> &'static str {
    use chrono::Weekday::*;
    match d {
        Mon => "seg", Tue => "ter", Wed => "qua", Thu => "qui",
        Fri => "sex", Sat => "sab", Sun => "dom",
    }
}

fn month_abbr_pt(m: u32) -> &'static str {
    match m {
        1 => "jan", 2 => "fev", 3 => "mar", 4 => "abr",
        5 => "mai", 6 => "jun", 7 => "jul", 8 => "ago",
        9 => "set", 10 => "out", 11 => "nov", 12 => "dez",
        _ => "?",
    }
}

fn format_date_pt(dt: &chrono::DateTime<Local>) -> String {
    format!("{} {} {}", weekday_abbr_pt(dt.weekday()), dt.day(), month_abbr_pt(dt.month()))
}

fn read_wifi() -> bool {
    if let Ok(entries) = std::fs::read_dir("/sys/class/net") {
        for e in entries.flatten() {
            let name = e.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("wl") {
                let s =
                    std::fs::read_to_string(e.path().join("operstate")).unwrap_or_default();
                if s.trim() == "up" {
                    return true;
                }
            }
        }
    }
    false
}

// ============================================================
// IPC client.
// ============================================================

fn connect_ipc() -> Option<UnixStream> {
    let path = default_socket_path()?;
    match UnixStream::connect(&path) {
        Ok(s) => {
            s.set_nonblocking(true).ok()?;
            eprintln!("[lumo-bar] IPC conectado em {}", path.display());
            Some(s)
        }
        Err(e) => {
            eprintln!("[lumo-bar] IPC nao conectou ({}): standalone mode", e);
            None
        }
    }
}

/// A25: retorna (alive, close_dropdowns_requested). Caller usa flag pra
/// fechar dropdown ativo + redraw imediato (memory feedback_input_feedback_imediato).
fn drain_ipc(stream: &mut UnixStream, rx_buf: &mut Vec<u8>, active_ws: &Arc<AtomicU8>) -> (bool, bool) {
    let mut tmp = [0u8; 256];
    let mut alive = true;
    let mut close_dropdowns = false;
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => {
                alive = false;
                break;
            }
            Ok(n) => rx_buf.extend_from_slice(&tmp[..n]),
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(_) => {
                alive = false;
                break;
            }
        }
    }
    while let Some(nl) = rx_buf.iter().position(|b| *b == b'\n') {
        let line: Vec<u8> = rx_buf.drain(..=nl).collect();
        if let Ok(s) = std::str::from_utf8(&line[..line.len() - 1]) {
            if let Ok(ev) = serde_json::from_str::<LumoEvent>(s.trim()) {
                match ev {
                    LumoEvent::Workspaces { active, .. } => {
                        active_ws.store(active.clamp(1, MAX_WORKSPACES), Ordering::Relaxed);
                    }
                    LumoEvent::CloseDropdowns => {
                        // A25: lumo-desktop pediu fechar dropdowns via IPC.
                        // Sinaliza pra loop main fechar + redraw imediato.
                        close_dropdowns = true;
                    }
                }
            }
        }
    }
    (alive, close_dropdowns)
}

// ============================================================
// LumoBar state + handlers
// ============================================================
struct LumoBar {
    registry: RegistryState,
    output_state: OutputState,
    shm: Shm,
    seat_state: SeatState,
    layer: LayerSurface,
    pool: SlotPool,
    width: u32,
    height: u32,
    active_workspace: Arc<AtomicU8>,
    battery_pct: u8,
    battery_info: BatteryInfo,
    wifi_on: bool,
    wifi_info: WifiInfo, // A23
    running: bool,
    first_configured: bool,
    pointer: Option<ThemedPointer>,
    pointer_x: f32,
    pointer_pos: Option<(f64, f64)>,
    bat_hit_rect: Option<(f32, f32, f32, f32)>,
    wifi_hit_rect: Option<(f32, f32, f32, f32)>,     // A23
    datetime_hit_rect: Option<(f32, f32, f32, f32)>, // A24
    last_click_at: Option<Instant>,
    dropdown: DropdownActive,
    ipc_stream: Option<UnixStream>,
    ipc_rx_buf: Vec<u8>,
    theme: LumoTheme,
    palette: LumoColors,
}

impl LumoBar {
    fn refresh(&mut self) {
        // A20: leitura completa /sys/class/power_supply.
        self.battery_info = read_battery_info();
        self.battery_pct = self.battery_info.pct;
        self.wifi_on = read_wifi();
        // A23: leitura wifi via iw + ip.
        self.wifi_info = read_wifi_info();
    }

    /// Altura efetiva da surface (bar + dropdown opcional).
    /// Reserva exclusive_zone original = BAR_HEIGHT (toplevels nao afetados).
    /// A20.11 + A24: surface ja eh sempre altura max, helper so pra referencia.
    fn computed_height(&self) -> u32 {
        let max_drop = DROPDOWN_H.max(DROPDOWN_DATETIME_H) as u32; // A24
        match self.dropdown {
            DropdownActive::None => BAR_HEIGHT,
            DropdownActive::Battery | DropdownActive::Wifi => {
                BAR_HEIGHT + DROPDOWN_GAP as u32 + DROPDOWN_H as u32 + 8
            }
            DropdownActive::DateTime => {
                BAR_HEIGHT + DROPDOWN_GAP as u32 + DROPDOWN_DATETIME_H as u32 + 8
            }
        }
        .max(BAR_HEIGHT + DROPDOWN_GAP as u32 + max_drop + 8)
    }

    /// Reconfigura tamanho do layer e redesenha (toggle dropdown).
    /// IMPORTANTE: exclusive_zone fixo = BAR_HEIGHT (DEPS.md A19.18).
    fn update_size_and_redraw(&mut self, qh: &QueueHandle<Self>) {
        // A20.11: surface SEMPRE altura max (BAR_HEIGHT + DROPDOWN). NAO faz
        // set_size dinamico (causava flicker open/close cycle). Renderiza
        // dropdown so se ativo; resto da surface fica transparente alpha 0.
        self.redraw(qh);
    }

    fn redraw(&mut self, _qh: &QueueHandle<Self>) {
        let now = Local::now();
        let snap = BarSnapshot {
            width: self.width,
            height: self.height,
            battery_pct: self.battery_pct,
            wifi_on: self.wifi_on,
            palette: self.palette,
            theme: self.theme,
            clock_hh: now.hour() as u8,
            clock_mm: now.minute() as u8,
            active_ws: self.active_workspace.load(Ordering::Relaxed),
            date_str: format_date_pt(&now),
            dropdown: self.dropdown,
            battery_info: self.battery_info.clone(),
            wifi_info: self.wifi_info.clone(),  // A23
            datetime_info: read_datetime_info(), // A24: realtime per frame
        };

        let stride = self.width as i32 * 4;
        // A18: VOLTA pra Argb8888 (alpha real pra pills semi-translucent).
        let (buffer, canvas) = match self.pool.create_buffer(
            self.width as i32,
            self.height as i32,
            stride,
            wl_shm::Format::Argb8888,
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[lumo-bar] create_buffer falhou: {e:?}");
                return;
            }
        };

        if let Some(mut px) = Pixmap::new(self.width, self.height) {
            let paint_result = paint_frame(&mut px, &snap);
            self.bat_hit_rect = paint_result.bat_hit_rect;
            self.wifi_hit_rect = paint_result.wifi_hit_rect;     // A23
            self.datetime_hit_rect = paint_result.datetime_hit_rect; // A24
            let src = px.data();
            let dst = canvas;
            let n = (self.width * self.height) as usize;
            // tiny-skia Pixmap = RGBA premul. wl_shm Argb8888 LE = BGRA na
            // memoria. Swap canais; alpha preservado (premul ja correto).
            for i in 0..n {
                let o = i * 4;
                if o + 3 < dst.len() && o + 3 < src.len() {
                    dst[o]     = src[o + 2]; // B
                    dst[o + 1] = src[o + 1]; // G
                    dst[o + 2] = src[o];     // R
                    dst[o + 3] = src[o + 3]; // A
                }
            }
        }

        let surface = self.layer.wl_surface();
        surface.damage_buffer(0, 0, self.width as i32, self.height as i32);
        buffer.attach_to(surface).ok();
        surface.commit();
    }
}

impl CompositorHandler for LumoBar {
    fn scale_factor_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: wl_output::Transform,
    ) {
    }
    fn frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: u32,
    ) {
        self.redraw(qh);
    }
    fn surface_enter(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: &wl_surface::WlSurface,
        _: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for LumoBar {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {
    }
}

impl LayerShellHandler for LumoBar {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.running = false;
    }

    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &LayerSurface,
        cfg: LayerSurfaceConfigure,
        _: u32,
    ) {
        let (w, h) = cfg.new_size;
        // A19.13: forca 1920 sempre (compositor passa width parcial as vezes)
        self.width = 1920;
        // A20.11 + A24: altura max cobre maior dropdown (DateTime 252 > Battery/Wifi 150).
        let max_drop = DROPDOWN_H.max(DROPDOWN_DATETIME_H) as u32;
        self.height = BAR_HEIGHT + DROPDOWN_GAP as u32 + max_drop + 8;
        self.first_configured = true;
        eprintln!("[lumo-bar] configured cfg_size=({},{}) FORCED width=1920 height={}", w, h, self.height);
        self.refresh();
        self.redraw(qh);
    }
}

impl ShmHandler for LumoBar {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

impl SeatHandler for LumoBar {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Pointer && self.pointer.is_none() {
            if let Ok(p) = self.seat_state.get_pointer_with_theme(
                qh,
                &seat,
                self.shm.wl_shm(),
                self.layer.wl_surface().clone(),
                smithay_client_toolkit::seat::pointer::ThemeSpec::System,
            ) {
                self.pointer = Some(p);
                eprintln!("[lumo-bar] pointer adquirido ThemedPointer");
            }
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        _: Capability,
    ) {
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for LumoBar {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        eprintln!("[lumo-bar] pointer_frame {} events", events.len());
        for ev in events {
            match ev.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.pointer_x = ev.position.0 as f32;
                    self.pointer_pos = Some(ev.position);
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer_pos = None;
                }
                PointerEventKind::Press { button, serial, time } => {
                    eprintln!("[lumo-bar] Press button={} serial={} time={} pos={:?} bat_rect={:?} wifi_rect={:?}", button, serial, time, ev.position, self.bat_hit_rect, self.wifi_hit_rect);
                    if button != BTN_LEFT { continue; }
                    // A20.10: debounce 200ms (re-size surface multipla = bug visual)
                    let now = Instant::now();
                    if let Some(last) = self.last_click_at {
                        if now.duration_since(last) < Duration::from_millis(200) {
                            eprintln!("[lumo-bar] click debounced");
                            continue;
                        }
                    }
                    self.last_click_at = Some(now);
                    let (px, py) = (ev.position.0 as f32, ev.position.1 as f32);
                    let mut handled = false;
                    if let Some((rx, ry, rw, rh)) = self.bat_hit_rect {
                        if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                            self.dropdown = if self.dropdown == DropdownActive::Battery {
                                DropdownActive::None
                            } else {
                                // Atualiza info bateria no momento do click
                                // (memory feedback_input_feedback_imediato).
                                self.refresh();
                                DropdownActive::Battery
                            };
                            self.update_size_and_redraw(qh);
                            handled = true;
                        }
                    }
                    // A23: hit wifi icone toggle dropdown wifi.
                    if !handled {
                        if let Some((rx, ry, rw, rh)) = self.wifi_hit_rect {
                            if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                self.dropdown = if self.dropdown == DropdownActive::Wifi {
                                    DropdownActive::None
                                } else {
                                    // Feedback imediato: leitura no instante do click.
                                    self.refresh();
                                    DropdownActive::Wifi
                                };
                                self.update_size_and_redraw(qh);
                                handled = true;
                            }
                        }
                    }
                    // A24: click data OU hora -> dropdown calendario+horario.
                    if !handled {
                        if let Some((rx, ry, rw, rh)) = self.datetime_hit_rect {
                            if px >= rx && px <= rx + rw && py >= ry && py <= ry + rh {
                                self.dropdown = if self.dropdown == DropdownActive::DateTime {
                                    DropdownActive::None
                                } else {
                                    // datetime_info eh sempre lido por frame em redraw,
                                    // entao nao precisa refresh aqui. Click ja gera
                                    // frame imediato via update_size_and_redraw.
                                    DropdownActive::DateTime
                                };
                                self.update_size_and_redraw(qh);
                                handled = true;
                            }
                        }
                    }
                    if !handled && self.dropdown != DropdownActive::None {
                        // Click fora -> fecha dropdown.
                        self.dropdown = DropdownActive::None;
                        self.update_size_and_redraw(qh);
                    }
                }
                _ => {}
            }
        }
    }
}

impl ProvidesRegistryState for LumoBar {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers!(OutputState, SeatState);
}

delegate_compositor!(LumoBar);
delegate_output!(LumoBar);
delegate_shm!(LumoBar);
delegate_layer!(LumoBar);
delegate_seat!(LumoBar);
delegate_pointer!(LumoBar);
delegate_registry!(LumoBar);

fn main() {
    let _ = font_system();
    let _ = swash_cache();

    let conn = Connection::connect_to_env().expect("conectar wayland");
    let (globals, mut queue) = registry_queue_init::<LumoBar>(&conn).expect("registry init");
    let qh = queue.handle();

    let compositor =
        CompositorState::bind(&globals, &qh).expect("wl_compositor nao disponivel");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("wlr_layer_shell nao disponivel");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm nao disponivel");

    let surface = compositor.create_surface(&qh);
    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Top, Some("lumo-bar"), None);
    layer.set_anchor(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
    // A24: altura max = MAX(DROPDOWN_H, DROPDOWN_DATETIME_H). DateTime mais alto (252).
    let surface_max_h = BAR_HEIGHT + DROPDOWN_GAP as u32 + DROPDOWN_H.max(DROPDOWN_DATETIME_H) as u32 + 8;
    layer.set_size(1920, surface_max_h);
    layer.set_exclusive_zone(BAR_HEIGHT as i32);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.commit();

    // A20/A24: pool dimensionado pra acomodar bar EXPANDIDA com maior dropdown.
    let max_height = surface_max_h as usize;
    let pool = SlotPool::new(1920 * max_height * 4 * 2, &shm)
        .expect("SlotPool init");

    let active_workspace = Arc::new(AtomicU8::new(1));
    let theme = lumo_foundation::current_theme();
    let palette = current_colors();
    eprintln!(
        "[lumo-bar] A18: pill-style activated; tema = {:?}, pill_bg = #{:06X}, alpha = 0x{:02X}",
        theme, palette.pill_bg, palette.pill_bg_alpha
    );

    let mut state = LumoBar {
        registry: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        seat_state: SeatState::new(&globals, &qh),
        layer,
        pool,
        width: 1920, // A19.18 default = output Galaxy nativo
        height: BAR_HEIGHT,
        active_workspace: active_workspace.clone(),
        battery_pct: 100,
        battery_info: BatteryInfo::default(),
        wifi_on: true,
        wifi_info: WifiInfo::default(), // A23
        running: true,
        first_configured: false,
        pointer: None,
        pointer_x: 0.0,
        pointer_pos: None,
        bat_hit_rect: None,
        wifi_hit_rect: None,     // A23
        datetime_hit_rect: None, // A24
        last_click_at: None,
        dropdown: DropdownActive::None,
        ipc_stream: connect_ipc(),
        ipc_rx_buf: Vec::with_capacity(256),
        theme,
        palette,
    };

    let mut last_tick = Instant::now();
    let mut last_clock_tick = Instant::now();
    let mut last_ipc_tick = Instant::now();
    while state.running {
        // Ticks PRIMEIRO (antes do dispatch nao bloquear demais)
        if last_clock_tick.elapsed() >= Duration::from_secs(1) {
            last_clock_tick = Instant::now();
            state.redraw(&qh);
        }
        if last_tick.elapsed() >= Duration::from_secs(30) {
            state.refresh();
            state.redraw(&qh);
            last_tick = Instant::now();
        }

        conn.flush().ok();
        // A20.9: poll com timeout 50ms = events processados sem bloqueio infinito
        if let Some(guard) = queue.prepare_read() {
            use std::os::fd::AsFd;
            let fd = conn.as_fd();
            let mut pfd = [nix::poll::PollFd::new(fd, nix::poll::PollFlags::POLLIN)];
            let _ = nix::poll::poll(&mut pfd, nix::poll::PollTimeout::try_from(50i32).unwrap());
            let _ = guard.read();
        }
        if let Err(e) = queue.dispatch_pending(&mut state) {
            let msg = format!("{e:?}");
            // A20.14: connection reset / broken pipe = compositor saiu, sair limpo (nao loop infinito)
            if msg.contains("ConnectionReset") || msg.contains("BrokenPipe") || msg.contains("InvalidObject") {
                eprintln!("[lumo-bar] compositor desconectou ({e:?}), saindo");
                break;
            }
            eprintln!("[lumo-bar] dispatch_pending warn: {e:?}");
        }
        // Detectar disconnect via flush tambem
        if conn.flush().is_err() {
            eprintln!("[lumo-bar] flush falhou, compositor encerrou - saindo");
            break;
        }

        if last_ipc_tick.elapsed() >= Duration::from_millis(8) {
            last_ipc_tick = Instant::now();
            if let Some(mut s) = state.ipc_stream.take() {
                let (alive, close_dropdowns) = drain_ipc(&mut s, &mut state.ipc_rx_buf, &state.active_workspace);
                if alive {
                    state.ipc_stream = Some(s);
                } else {
                    eprintln!("[lumo-bar] IPC peer fechou; bar continua standalone");
                }
                // A25: CloseDropdowns IPC (lumo-desktop click esquerdo desktop).
                if close_dropdowns && state.dropdown != DropdownActive::None {
                    state.dropdown = DropdownActive::None;
                    state.update_size_and_redraw(&qh);
                    eprintln!("[lumo-bar] CloseDropdowns recebido -> dropdown fechado");
                }
            }
        }

    }
    let _ = active_workspace;
}
