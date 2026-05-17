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
        pointer::{PointerEvent, PointerEventKind, PointerHandler, ThemedPointer},
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
const FONT_DATE: f32 = 11.0;

/// Wifi icone 16x16 (compact pra caber dentro de pill 28h).
const WIFI_SIZE: f32 = 16.0;

/// Bateria icone 14x8 (proporcional a 28h pill).
const BAT_BODY_W: f32 = 18.0; // A19.5 maior + visivel
const BAT_BODY_H: f32 = 10.0;

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
    stroke_rrect(canvas, x + 0.5, y + 0.5, body_w - 1.0, body_h - 1.0, 1.6, fg, 1.0);
    fill_rrect(canvas, x + body_w + 0.6, y + body_h * 0.3, 1.8, body_h * 0.4, 0.6, fg);
    // A19.8: branco quando cheio, laranja medio, vermelho baixo + inset_y maior (linha de baixo desapareceu)
    let inset_x = 1.5f32;
    let inset_y = 2.8f32; // mais pra baixo ainda
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
        fill_rrect(canvas, x + inset_x, y + inset_y, fw, inner_h, 0.8, fill_color);
    }
}

fn battery_total_width() -> f32 {
    BAT_BODY_W + 1.4
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
}

// ============================================================
// paint_frame: pinta as 2 pills sobre fundo transparente.
// ============================================================
fn paint_frame(pixmap: &mut Pixmap, snap: &BarSnapshot) {
    let palette = &snap.palette;
    // BAR BACKGROUND TRANSPARENTE (A18 — alpha 0). Compositor pinta atras.
    pixmap.fill(Color::TRANSPARENT);

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
        draw_battery(&mut canvas, cx, pill_cy - BAT_BODY_H / 2.0, snap.battery_pct, pill_fg, accent);
        cx += bat_icon_w + PILL_GAP;
        draw_wifi(&mut canvas, cx, pill_cy - WIFI_SIZE / 2.0, snap.wifi_on, pill_fg, pill_fg_subtle);
        cx += WIFI_SIZE + PILL_GAP;
        draw_text(&mut canvas, cx, text_top + 1.0, &snap.date_str, FONT_DATE, pill_fg_subtle, false);
        cx += date_w + 8.0;
        draw_text(&mut canvas, cx, text_top, &clock_s, FONT_PILL, pill_fg, false);
    }

    // Suppress unused warns nos campos do snapshot (theme so usado pra debug log).
    let _ = (snap.theme, h);
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

fn read_battery() -> u8 {
    for bat in &["BAT0", "BAT1"] {
        if let Ok(s) = std::fs::read_to_string(format!("/sys/class/power_supply/{}/capacity", bat))
        {
            if let Ok(n) = s.trim().parse::<u8>() {
                return n;
            }
        }
    }
    100
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

fn drain_ipc(stream: &mut UnixStream, rx_buf: &mut Vec<u8>, active_ws: &Arc<AtomicU8>) -> bool {
    let mut tmp = [0u8; 256];
    let mut alive = true;
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
                }
            }
        }
    }
    alive
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
    wifi_on: bool,
    running: bool,
    first_configured: bool,
    pointer: Option<ThemedPointer>,
    pointer_x: f32,
    ipc_stream: Option<UnixStream>,
    ipc_rx_buf: Vec<u8>,
    theme: LumoTheme,
    palette: LumoColors,
}

impl LumoBar {
    fn refresh(&mut self) {
        self.battery_pct = read_battery();
        self.wifi_on = read_wifi();
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
            paint_frame(&mut px, &snap);
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
        self.height = BAR_HEIGHT;
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
        _qh: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for ev in events {
            if let PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } = ev.kind {
                self.pointer_x = ev.position.0 as f32;
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
    layer.set_size(0, BAR_HEIGHT);
    layer.set_exclusive_zone(BAR_HEIGHT as i32);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.commit();

    let pool = SlotPool::new(1920 * BAR_HEIGHT as usize * 4 * 2, &shm)
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
        width: 1280,
        height: BAR_HEIGHT,
        active_workspace: active_workspace.clone(),
        battery_pct: 100,
        wifi_on: true,
        running: true,
        first_configured: false,
        pointer: None,
        pointer_x: 0.0,
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
        // A19.9: dispatch_pending nao bloqueia (timers funcionam)
        queue.dispatch_pending(&mut state).expect("dispatch fail");
        // sleep curto pra nao consumir CPU 100%
        std::thread::sleep(Duration::from_millis(100));

        if last_ipc_tick.elapsed() >= Duration::from_millis(8) {
            last_ipc_tick = Instant::now();
            if let Some(mut s) = state.ipc_stream.take() {
                let alive = drain_ipc(&mut s, &mut state.ipc_rx_buf, &state.active_workspace);
                if alive {
                    state.ipc_stream = Some(s);
                } else {
                    eprintln!("[lumo-bar] IPC peer fechou; bar continua standalone");
                }
            }
        }

    }
    let _ = active_workspace;
}
