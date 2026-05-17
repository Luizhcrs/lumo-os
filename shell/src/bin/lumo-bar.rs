//! lumo-bar - top bar Lumo OS via wlr-layer-shell + SHM + tiny-skia.
//!
//! A15 redesign: cosmic-text real (Geist Mono / JetBrains Mono Nerd Font
//! fallback) substituindo bitmap font 5x7 do A14. Wifi 18px (era 14).
//! Verde solido validado (sem gradient, sem shader, blend SrcOver default).
//!
//! Layout (28px alt, full width):
//!
//!   [dot] Lumo  Editar  Visualizar  Ajuda          wifi 73% 13:45 sex 17 mai
//!
//! Slots:
//!   - Esquerda (PAD_X=14, gap=16px entre items): brand dot 8px circulo
//!     emerald + menus text 13px (Lumo BOLD, restantes regular).
//!   - Direita (PAD_X=14, gap=12px): wifi 18x18 -> bateria texto
//!     "73%" + icone 18x10 -> clock HH:MM mono -> data abrev pt-br
//!     "sex 17 mai" (fg_subtle).
//!
//! Tipografia: cosmic-text 0.12 + tiny-skia. FontSystem singleton lazy.
//! Tenta Geist Mono primeiro, depois JetBrains Mono Nerd Font, depois
//! monospace generico. Glyphs anti-aliased via SwashCache. Coords
//! arredondadas pra evitar sub-pixel shimmer.
//!
//! Border bottom: 1px linha cor `border` (sutil). ZERO box-shadow colorido
//! (memory feedback_zero_neon_glow).

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

/// Altura fixa da bar. 28px = Apple macOS densidade.
const BAR_HEIGHT: u32 = 28;

/// Padding horizontal nas duas pontas. 14px (Apple ~14).
const PAD_X: f32 = 14.0;

/// Gap entre menus do slot esquerdo (apos brand dot).
const MENU_GAP: f32 = 16.0;

/// Gap entre items do slot direito (wifi/bat/clock/data).
const SEG_GAP: f32 = 12.0;

/// Brand dot diametro. 8px = atomo visual estavel.
const BRAND_DOT_RADIUS: f32 = 4.0;

/// Espacamento entre brand dot e primeiro menu.
const BRAND_GAP: f32 = 14.0;

/// Font sizes (px). Menus 13, status 12, data 11 (Apple-grade hierarchy).
const FONT_MENU: f32 = 14.0;
const FONT_STATUS: f32 = 13.0;
const FONT_DATE: f32 = 12.0;

/// Wifi icone tamanho (A15: 18 era 14).
const WIFI_SIZE: f32 = 18.0;

/// Bateria icone tamanho (A15: 18x10 aspect Apple).
const BAT_BODY_W: f32 = 18.0;
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
//
// cosmic-text 0.12: FontSystem nao eh thread-safe internamente quando
// loadado em paralelo. Lock Mutex protege. Init lazy uma vez no primeiro
// uso. Tenta Geist Mono -> JetBrains Mono Nerd Font -> monospace.

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

/// Carrega ttf/otf extras de paths locais conhecidos (Geist se Luiz
/// instalar em ~/.local/share/fonts ou similar). Nao falha se nao achar.
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

/// Decide family pra usar com base em quais estao registradas no db.
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
    // fallback partial-match
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

/// Familia atualmente escolhida (preenchida no init do font_system).
fn current_family() -> &'static str {
    FONT_FAMILY.get().map(|s| s.as_str()).unwrap_or("monospace")
}

// ============================================================
// Vector primitives (tiny-skia paths).
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

/// Arco SVG-style usando tiny-skia path quad bezier.
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

fn fill_rect_color(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: Color) {
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = false;
    if let Some(r) = Rect::from_xywh(x.round(), y.round(), w, h) {
        pixmap.fill_rect(r, &p, Transform::identity(), None);
    }
}

// ============================================================
// Text rendering via cosmic-text + swash + tiny-skia.
// ============================================================
//
// Estrategia: shape texto com cosmic-text -> swash rasteriza glyph em
// mascara alpha 8-bit -> blita pixel a pixel no canvas usando fill_rect 1x1
// com cor pre-multiplicada pelo alpha do glyph. tiny-skia nao expose
// direct pixel API, mas fill_rect 1x1 sem AA eh equivalente.

/// Mede a largura de `text` no tamanho dado (sem desenhar).
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

/// Desenha string em (x, y_top) com cor e tamanho. Retorna largura usada.
/// y_top eh top-left do bounding box (linha de base alinhada via cosmic).
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
        // Grayscale AA: ignora RGB subpixel (rainbow artifact em painel sem LCD-RGB stripe).
        // Usa COR PASSADA + alpha do glyph (mask 8-bit).
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
        // Cada pixel do glyph eh 1x1; AA ja vem do alpha gcolor (mascara
        // 8-bit grey-on-color do swash). fill_rect 1x1 AA off pra evitar
        // duplo-AA.
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

/// Helper: altura "metrics" para alinhar vertical (cap top).
fn text_baseline_top(_size: f32, bar_h: f32, text_h: f32) -> f32 {
    ((bar_h - text_h) / 2.0).round()
}

// ============================================================
// Brand mark.
// ============================================================
fn draw_brand_dot(canvas: &mut PixmapMut, cx: f32, cy: f32, color: Color) {
    fill_circle(canvas, cx, cy, BRAND_DOT_RADIUS, color);
}

// ============================================================
// Wifi glyph (3 arcos concentricos + ponto). A15: 18x18 (era 14x14).
// ============================================================
fn draw_wifi(canvas: &mut PixmapMut, x: f32, y: f32, on: bool, palette: &LumoColors) {
    let color = if on {
        opaque(palette.fg)
    } else {
        opaque(palette.fg_subtle)
    };
    let s = WIFI_SIZE;
    let cx = x + s / 2.0;
    let cy = y + s * 0.78; // ponto fica perto da base do icone
    // Arcos: raio externo ~0.46s, medio ~0.30s, interno ~0.15s.
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
// Battery glyph (18x10 A15, ratio Apple). Fill horizontal proporcional.
// ============================================================
fn draw_battery(canvas: &mut PixmapMut, x: f32, y: f32, pct: u8, palette: &LumoColors) {
    let body_w = BAT_BODY_W;
    let body_h = BAT_BODY_H;
    let stroke = opaque(palette.fg);
    stroke_rrect(canvas, x + 0.5, y + 0.5, body_w - 1.0, body_h - 1.0, 1.8, stroke, 1.0);
    fill_rrect(canvas, x + body_w, y + body_h * 0.28, 1.6, body_h * 0.44, 0.6, stroke);
    let inner_w = body_w - 4.0;
    let fw = (pct as f32 / 100.0).clamp(0.0, 1.0) * inner_w;
    if fw > 0.2 {
        let fill_color = if pct > 20 {
            opaque(palette.accent)
        } else {
            opaque(0xEF4444)
        };
        fill_rrect(canvas, x + 2.0, y + 2.0, fw, body_h - 4.0, 0.9, fill_color);
    }
}

/// Largura total do icone bateria incluindo cap (pra layout).
fn battery_total_width() -> f32 {
    BAT_BODY_W + 1.8
}

// ============================================================
// Date abreviada pt-br.
// ============================================================
fn weekday_abbr_pt(dt: &chrono::DateTime<Local>) -> &'static str {
    match dt.weekday() {
        chrono::Weekday::Mon => "seg",
        chrono::Weekday::Tue => "ter",
        chrono::Weekday::Wed => "qua",
        chrono::Weekday::Thu => "qui",
        chrono::Weekday::Fri => "sex",
        chrono::Weekday::Sat => "sab",
        chrono::Weekday::Sun => "dom",
    }
}

fn month_abbr_pt(m: u32) -> &'static str {
    match m {
        1 => "jan",
        2 => "fev",
        3 => "mar",
        4 => "abr",
        5 => "mai",
        6 => "jun",
        7 => "jul",
        8 => "ago",
        9 => "set",
        10 => "out",
        11 => "nov",
        12 => "dez",
        _ => "???",
    }
}

fn format_date_pt(dt: &chrono::DateTime<Local>) -> String {
    format!(
        "{} {:02} {}",
        weekday_abbr_pt(dt),
        dt.day(),
        month_abbr_pt(dt.month())
    )
}

// ============================================================
// Bar snapshot.
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
    date_abbr: String,
}

fn paint_frame(pixmap: &mut Pixmap, snap: &BarSnapshot) {
    let palette = &snap.palette;
    pixmap.fill(opaque(palette.bg));
    let h = snap.height as f32;
    let cy = h / 2.0;

    // Top y para textos (centralizado vertical). cosmic-text baseline-top
    // alinha pelo line-height; usamos size*1.2 entao text_h ~ size*1.2.
    let menu_top = text_baseline_top(FONT_MENU, h, FONT_MENU * 1.2);
    let status_top = text_baseline_top(FONT_STATUS, h, FONT_STATUS * 1.2);
    let date_top = text_baseline_top(FONT_DATE, h, FONT_DATE * 1.2);

    // ===== Esquerda: brand dot + menus =====
    {
        let mut canvas = pixmap.as_mut();
        let mut lx = PAD_X;
        draw_brand_dot(&mut canvas, lx + BRAND_DOT_RADIUS, cy, opaque(palette.accent));
        lx += BRAND_DOT_RADIUS * 2.0 + BRAND_GAP;

        let menus: &[(&str, bool)] = &[
            ("Lumo", true),
            ("Editar", false),
            ("Visualizar", false),
            ("Ajuda", false),
        ];
        let menu_color = opaque(palette.fg);
        for (text, bold) in menus {
            draw_text(&mut canvas, lx, menu_top, text, FONT_MENU, menu_color, *bold);
            let w = measure_text(text, FONT_MENU, *bold);
            lx += w + MENU_GAP;
        }
    }

    // ===== Direita =====
    let mut rx = snap.width as f32 - PAD_X;

    // Data
    let date_w = measure_text(&snap.date_abbr, FONT_DATE, false);
    rx -= date_w;
    {
        let mut canvas = pixmap.as_mut();
        draw_text(
            &mut canvas,
            rx,
            date_top,
            &snap.date_abbr,
            FONT_DATE,
            opaque(palette.fg_subtle),
            false,
        );
    }
    rx -= SEG_GAP;

    // Clock
    let clock_s = format!("{:02}:{:02}", snap.clock_hh, snap.clock_mm);
    let clock_w = measure_text(&clock_s, FONT_STATUS, false);
    rx -= clock_w;
    {
        let mut canvas = pixmap.as_mut();
        draw_text(
            &mut canvas,
            rx,
            status_top,
            &clock_s,
            FONT_STATUS,
            opaque(palette.fg),
            false,
        );
    }
    rx -= SEG_GAP;

    // Bateria: texto + icone
    let bat_text = format!("{}%", snap.battery_pct);
    let bat_text_w = measure_text(&bat_text, FONT_STATUS, false);
    let bat_icon_w = battery_total_width();
    let bat_gap = 4.0;
    rx -= bat_text_w + bat_gap + bat_icon_w;
    {
        let mut canvas = pixmap.as_mut();
        let bat_color = if snap.battery_pct > 20 {
            opaque(palette.accent)
        } else {
            opaque(palette.fg)
        };
        draw_text(
            &mut canvas,
            rx,
            status_top,
            &bat_text,
            FONT_STATUS,
            bat_color,
            false,
        );
        draw_battery(
            &mut canvas,
            rx + bat_text_w + bat_gap,
            cy - BAT_BODY_H / 2.0,
            snap.battery_pct,
            palette,
        );
    }
    rx -= SEG_GAP;

    // Wifi 18x18
    rx -= WIFI_SIZE;
    {
        let mut canvas = pixmap.as_mut();
        draw_wifi(&mut canvas, rx, cy - WIFI_SIZE / 2.0, snap.wifi_on, palette);
    }

    let _ = snap.theme;

    // Border-bottom 1px
    fill_rect_color(pixmap, 0.0, h - 1.0, snap.width as f32, 1.0, opaque(palette.border));
}

// ============================================================
// Sensors.
// ============================================================
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
            date_abbr: format_date_pt(&now),
        };

        let stride = self.width as i32 * 4;
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
            // tiny-skia Pixmap.data() JA premultiplied (docs.rs/tiny-skia
            // Pixmap: "premultiplied RGBA pixels"). So precisa swap RGBA->BGRA
            // pra wl_shm Argb8888 little-endian. Premultiply manual = duplo = bug.
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
        self.width = if w == 0 { 1280 } else { w };
        self.height = if h == 0 { BAR_HEIGHT } else { h };
        self.first_configured = true;
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
    // Warm up font_system (gera log de qual familia foi escolhida).
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
        "[lumo-bar] A15 cosmic-text; tema = {:?}, accent = #{:06X}, bg = #{:06X}",
        theme, palette.accent, palette.bg
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
        conn.flush().ok();
        queue
            .blocking_dispatch(&mut state)
            .expect("dispatch fail");

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

        if last_clock_tick.elapsed() >= Duration::from_secs(1) {
            last_clock_tick = Instant::now();
            state.redraw(&qh);
        }

        if last_tick.elapsed() >= Duration::from_secs(30) {
            state.refresh();
            state.redraw(&qh);
            last_tick = Instant::now();
        }
    }
    let _ = active_workspace;
}
