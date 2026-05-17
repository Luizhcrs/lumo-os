//! lumo-bar - top bar Lumo OS via wlr-layer-shell + SHM + tiny-skia.
//!
//! A14 redesign: Apple-style top bar (menus topo + status compacto).
//! Workspaces removidos (vao pro dock futuro). Power button removido
//! (vai pro menu Lumo dropdown futuro).
//!
//! Layout (28px alt, full width):
//!
//!   [dot] Lumo  Editar  Visualizar  Ajuda          wifi 73% 13:45 sex 17 mai
//!
//! Slots:
//!   - Esquerda (PAD_X=14, gap=16px entre items): brand dot 8px circulo
//!     emerald + menus text 12px (Lumo BOLD, restantes regular).
//!   - Direita (PAD_X=14, gap=12px): wifi (3 arcos) -> bateria texto
//!     "73%" + icone -> clock HH:MM mono -> data abrev pt-br "sex 17 mai"
//!     (fg_subtle).
//!
//! Tipografia: bitmap font 5x7 pixel-perfect desenhado via tiny-skia
//! com paint anti_alias=false (texto eh pixel-art, AA borra) + coords
//! arredondadas. Glyphs definidos inline (memory feedback_zero_emoji
//! nao usa Nerd Font / emoji). Bold = double-stroke (mesmo glyph
//! desenhado duas vezes com offset 1px).
//!
//! Border bottom: 1px linha cor `border` (sutil). ZERO box-shadow colorido
//! (memory feedback_zero_neon_glow).
//!
//! IPC consumer (drain_ipc) mantido vivo pra futuro use mas state de
//! workspace nao alimenta mais render.

use std::io::{ErrorKind, Read};
use std::os::unix::net::UnixStream;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use chrono::{Datelike, Local, Timelike};
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

/// Altura fixa da bar. 28px = Apple macOS densidade (era 32 antes do A14).
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

/// Tamanho do pixel do glyph. 5x7 char * pixel_size = tamanho final.
/// `1` = 5x7 (tiny). `2` = 10x14 (legivel @ 12px equivalente).
const FONT_PX: f32 = 2.0;

/// Espacamento entre glyphs (1 col pixel @ FONT_PX).
const FONT_SPACING: f32 = 1.0;

// ============================================================
// Color helpers (hex 0xRRGGBB -> tiny_skia Color via theme).
// ============================================================

/// Hex 0xRRGGBB + alpha 0..255 -> tiny_skia Color (sRGB premultiplied).
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

// ============================================================
// Vector primitives (tiny-skia paths).
// ============================================================
//
// A14 fix serrilhado: TODOS path-based primitives usam anti_alias=true.
// Coords passadas pelos callers sao arredondadas com `.round()` antes
// de chamar para evitar sub-pixel offset acumulado nas bordas (que
// gerava "linha serrilhada" visivel nas pills).

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

/// Arco SVG-style usando tiny-skia path. cx,cy = centro do circulo
/// imaginario; arco vai de angulo `start_deg` a `end_deg` (sentido horario,
/// 0deg = leste, 90deg = sul). Quadratic bezier aproxima arco curto.
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

/// fill_rect dentro de canvas (PixmapMut). Pixel-art, AA off.
fn fill_rect_px(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, color: Color) {
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = false;
    if let Some(r) = Rect::from_xywh(x.round(), y.round(), w, h) {
        canvas.fill_rect(r, &p, Transform::identity(), None);
    }
}

// ============================================================
// Bitmap font 5x7 pixel-perfect (subset ASCII).
// ============================================================
//
// Memory feedback_zero_emoji: zero Nerd Font / emoji. Glyphs definidos
// inline via mascara de bits 5 colunas x 7 linhas. Cada `[u8; 7]` eh
// uma linha do glyph; bit `0b10000` = coluna 0 (esquerda), `0b00001` =
// coluna 4 (direita). Bit 1 = pixel ligado.
//
// Subset: a-z minusculo + 0-9 + ' ' + ':' + '%' + L V E A (maiusculos
// usados nos menus). Suficiente pra "Lumo Editar Visualizar Ajuda" +
// data abrev pt-br + clock + bateria.

type Glyph = [u8; 7];

/// Retorna a mascara 5x7 do char, ou None se nao suportado.
fn glyph_of(c: char) -> Option<Glyph> {
    // Formato: 5 bits por linha, 7 linhas (top->bottom).
    // 0b11111 = linha cheia, 0b10001 = bordas, etc.
    Some(match c {
        // ----- letras maiusculas (so as que aparecem em menus) -----
        'L' => [
            0b10000,
            0b10000,
            0b10000,
            0b10000,
            0b10000,
            0b10000,
            0b11111,
        ],
        'V' => [
            0b10001,
            0b10001,
            0b10001,
            0b10001,
            0b10001,
            0b01010,
            0b00100,
        ],
        'E' => [
            0b11111,
            0b10000,
            0b10000,
            0b11110,
            0b10000,
            0b10000,
            0b11111,
        ],
        'A' => [
            0b01110,
            0b10001,
            0b10001,
            0b11111,
            0b10001,
            0b10001,
            0b10001,
        ],

        // ----- letras minusculas a-z (subset usado) -----
        'a' => [
            0b00000,
            0b00000,
            0b01110,
            0b00001,
            0b01111,
            0b10001,
            0b01111,
        ],
        'b' => [
            0b10000,
            0b10000,
            0b10110,
            0b11001,
            0b10001,
            0b10001,
            0b11110,
        ],
        'c' => [
            0b00000,
            0b00000,
            0b01110,
            0b10001,
            0b10000,
            0b10001,
            0b01110,
        ],
        'd' => [
            0b00001,
            0b00001,
            0b01101,
            0b10011,
            0b10001,
            0b10001,
            0b01111,
        ],
        'e' => [
            0b00000,
            0b00000,
            0b01110,
            0b10001,
            0b11111,
            0b10000,
            0b01110,
        ],
        'f' => [
            0b00110,
            0b01001,
            0b01000,
            0b11110,
            0b01000,
            0b01000,
            0b01000,
        ],
        'g' => [
            0b00000,
            0b00000,
            0b01111,
            0b10001,
            0b01111,
            0b00001,
            0b01110,
        ],
        'h' => [
            0b10000,
            0b10000,
            0b10110,
            0b11001,
            0b10001,
            0b10001,
            0b10001,
        ],
        'i' => [
            0b00100,
            0b00000,
            0b01100,
            0b00100,
            0b00100,
            0b00100,
            0b01110,
        ],
        'j' => [
            0b00010,
            0b00000,
            0b00110,
            0b00010,
            0b00010,
            0b10010,
            0b01100,
        ],
        'k' => [
            0b10000,
            0b10000,
            0b10010,
            0b10100,
            0b11000,
            0b10100,
            0b10010,
        ],
        'l' => [
            0b01100,
            0b00100,
            0b00100,
            0b00100,
            0b00100,
            0b00100,
            0b01110,
        ],
        'm' => [
            0b00000,
            0b00000,
            0b11010,
            0b10101,
            0b10101,
            0b10001,
            0b10001,
        ],
        'n' => [
            0b00000,
            0b00000,
            0b10110,
            0b11001,
            0b10001,
            0b10001,
            0b10001,
        ],
        'o' => [
            0b00000,
            0b00000,
            0b01110,
            0b10001,
            0b10001,
            0b10001,
            0b01110,
        ],
        'p' => [
            0b00000,
            0b00000,
            0b11110,
            0b10001,
            0b11110,
            0b10000,
            0b10000,
        ],
        'q' => [
            0b00000,
            0b00000,
            0b01111,
            0b10001,
            0b01111,
            0b00001,
            0b00001,
        ],
        'r' => [
            0b00000,
            0b00000,
            0b10110,
            0b11001,
            0b10000,
            0b10000,
            0b10000,
        ],
        's' => [
            0b00000,
            0b00000,
            0b01111,
            0b10000,
            0b01110,
            0b00001,
            0b11110,
        ],
        't' => [
            0b01000,
            0b01000,
            0b11110,
            0b01000,
            0b01000,
            0b01001,
            0b00110,
        ],
        'u' => [
            0b00000,
            0b00000,
            0b10001,
            0b10001,
            0b10001,
            0b10011,
            0b01101,
        ],
        'v' => [
            0b00000,
            0b00000,
            0b10001,
            0b10001,
            0b10001,
            0b01010,
            0b00100,
        ],
        'w' => [
            0b00000,
            0b00000,
            0b10001,
            0b10001,
            0b10101,
            0b10101,
            0b01010,
        ],
        'x' => [
            0b00000,
            0b00000,
            0b10001,
            0b01010,
            0b00100,
            0b01010,
            0b10001,
        ],
        'y' => [
            0b00000,
            0b00000,
            0b10001,
            0b10001,
            0b01111,
            0b00001,
            0b01110,
        ],
        'z' => [
            0b00000,
            0b00000,
            0b11111,
            0b00010,
            0b00100,
            0b01000,
            0b11111,
        ],

        // ----- digitos 0-9 -----
        '0' => [
            0b01110,
            0b10001,
            0b10011,
            0b10101,
            0b11001,
            0b10001,
            0b01110,
        ],
        '1' => [
            0b00100,
            0b01100,
            0b00100,
            0b00100,
            0b00100,
            0b00100,
            0b01110,
        ],
        '2' => [
            0b01110,
            0b10001,
            0b00001,
            0b00010,
            0b00100,
            0b01000,
            0b11111,
        ],
        '3' => [
            0b11111,
            0b00010,
            0b00100,
            0b00010,
            0b00001,
            0b10001,
            0b01110,
        ],
        '4' => [
            0b00010,
            0b00110,
            0b01010,
            0b10010,
            0b11111,
            0b00010,
            0b00010,
        ],
        '5' => [
            0b11111,
            0b10000,
            0b11110,
            0b00001,
            0b00001,
            0b10001,
            0b01110,
        ],
        '6' => [
            0b00110,
            0b01000,
            0b10000,
            0b11110,
            0b10001,
            0b10001,
            0b01110,
        ],
        '7' => [
            0b11111,
            0b00001,
            0b00010,
            0b00100,
            0b01000,
            0b01000,
            0b01000,
        ],
        '8' => [
            0b01110,
            0b10001,
            0b10001,
            0b01110,
            0b10001,
            0b10001,
            0b01110,
        ],
        '9' => [
            0b01110,
            0b10001,
            0b10001,
            0b01111,
            0b00001,
            0b00010,
            0b01100,
        ],

        // ----- pontuacao -----
        ' ' => [0; 7],
        ':' => [
            0b00000,
            0b00100,
            0b00100,
            0b00000,
            0b00100,
            0b00100,
            0b00000,
        ],
        '%' => [
            0b11001,
            0b11010,
            0b00010,
            0b00100,
            0b01000,
            0b01011,
            0b10011,
        ],

        _ => return None,
    })
}

/// Largura visual em pixels do char rendered com FONT_PX e spacing.
fn glyph_width_px() -> f32 {
    5.0 * FONT_PX + FONT_SPACING * FONT_PX
}

/// Altura visual em pixels do char.
fn glyph_height_px() -> f32 {
    7.0 * FONT_PX
}

/// Desenha um char numa posicao (top-left x,y). Pixels via rects sem AA
/// (pixel-art puro, AA borraria).
fn draw_glyph(canvas: &mut PixmapMut, x: f32, y: f32, c: char, color: Color, bold: bool) {
    let glyph = match glyph_of(c) {
        Some(g) => g,
        None => return,
    };
    let px = FONT_PX;
    let x = x.round();
    let y = y.round();
    for (row, mask) in glyph.iter().enumerate() {
        for col in 0..5 {
            // bit MSB (col 0) -> bit 4; col 4 -> bit 0.
            let bit = 1 << (4 - col);
            if mask & bit != 0 {
                let gx = x + (col as f32) * px;
                let gy = y + (row as f32) * px;
                fill_rect_px(canvas, gx, gy, px, px, color);
                if bold {
                    // Bold = desenha glyph deslocado +1px x (duplica trazo
                    // horizontal). Tradeoff: simples, sem font weight real.
                    fill_rect_px(canvas, gx + 1.0, gy, px, px, color);
                }
            }
        }
    }
}

/// Largura total em pixels de uma string com font e spacing atual.
fn text_width_px(s: &str, bold: bool) -> f32 {
    let mut w = 0.0;
    for (i, c) in s.chars().enumerate() {
        if glyph_of(c).is_some() {
            w += 5.0 * FONT_PX;
            if i + 1 < s.chars().count() {
                w += FONT_SPACING * FONT_PX;
            }
        }
    }
    if bold {
        w += 1.0;
    }
    w
}

/// Desenha string em x,y (top-left), avancando glyph_width_px por char.
fn draw_text(canvas: &mut PixmapMut, x: f32, y: f32, s: &str, color: Color, bold: bool) {
    let mut cx = x;
    let advance = glyph_width_px();
    for c in s.chars() {
        if glyph_of(c).is_some() {
            draw_glyph(canvas, cx, y, c, color, bold);
            cx += advance;
        }
    }
}

// ============================================================
// Brand mark.
// ============================================================
fn draw_brand_dot(canvas: &mut PixmapMut, cx: f32, cy: f32, color: Color) {
    fill_circle(canvas, cx, cy, BRAND_DOT_RADIUS, color);
}

// ============================================================
// Wifi glyph (3 arcos concentricos).
// ============================================================
fn draw_wifi(canvas: &mut PixmapMut, x: f32, y: f32, on: bool, palette: &LumoColors) {
    let color = if on {
        opaque(palette.fg)
    } else {
        opaque(palette.fg_subtle)
    };
    let cx = x + 7.0;
    let cy = y + 11.0;
    for (radius, sw) in [(6.5, 1.2), (4.3, 1.1), (2.2, 1.0)] {
        stroke_arc(canvas, cx, cy, radius, -135.0, -45.0, color, sw);
    }
    fill_circle(canvas, cx, cy, 0.9, color);
}

// ============================================================
// Battery glyph (14x10 com fill horizontal proporcional + cap).
// ============================================================
fn draw_battery(canvas: &mut PixmapMut, x: f32, y: f32, pct: u8, palette: &LumoColors) {
    let body_w = 14.0;
    let body_h = 8.0;
    let stroke = opaque(palette.fg);
    stroke_rrect(canvas, x + 0.5, y + 0.5, body_w - 1.0, body_h - 1.0, 1.4, stroke, 1.0);
    fill_rrect(canvas, x + body_w, y + 2.5, 1.3, 3.0, 0.5, stroke);
    let inner_w = body_w - 4.0;
    let fw = (pct as f32 / 100.0).clamp(0.0, 1.0) * inner_w;
    if fw > 0.2 {
        // A14: accent se >20%, red signal universal se <=20%.
        let fill_color = if pct > 20 {
            opaque(palette.accent)
        } else {
            opaque(0xEF4444)
        };
        fill_rrect(canvas, x + 2.0, y + 2.0, fw, body_h - 4.0, 0.7, fill_color);
    }
}

// ============================================================
// Date abreviada pt-br (sex 17 mai).
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
// Bar snapshot (state imutavel pra paint_frame).
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
    // Texto 7px glyph @ FONT_PX=2 = 14px alt. Top y = cy - 7.
    let text_top = (cy - glyph_height_px() / 2.0).round();

    // ===== Esquerda: brand dot + menus =====
    {
        let mut canvas = pixmap.as_mut();
        let mut lx = PAD_X;
        draw_brand_dot(&mut canvas, lx + BRAND_DOT_RADIUS, cy, opaque(palette.accent));
        lx += BRAND_DOT_RADIUS * 2.0 + BRAND_GAP;

        // Menus: Lumo bold, restantes regular. fg cor padrao.
        let menus: &[(&str, bool)] = &[
            ("Lumo", true),
            ("Editar", false),
            ("Visualizar", false),
            ("Ajuda", false),
        ];
        let menu_color = opaque(palette.fg);
        for (text, bold) in menus {
            draw_text(&mut canvas, lx, text_top, text, menu_color, *bold);
            let w = text_width_px(text, *bold);
            lx += w + MENU_GAP;
        }
    }

    // ===== Direita: data, clock, bat (texto + icone), wifi =====
    // Ordem render: da direita pra esquerda (subtraindo de rx).
    let mut rx = snap.width as f32 - PAD_X;

    // -- Data abrev (rightmost). cor fg_subtle.
    let date_w = text_width_px(&snap.date_abbr, false);
    rx -= date_w;
    {
        let mut canvas = pixmap.as_mut();
        draw_text(&mut canvas, rx, text_top, &snap.date_abbr, opaque(palette.fg_subtle), false);
    }
    rx -= SEG_GAP;

    // -- Clock HH:MM (Geist Mono equivalent: usa mesma font 5x7).
    let clock_s = format!("{:02}:{:02}", snap.clock_hh, snap.clock_mm);
    let clock_w = text_width_px(&clock_s, false);
    rx -= clock_w;
    {
        let mut canvas = pixmap.as_mut();
        draw_text(&mut canvas, rx, text_top, &clock_s, opaque(palette.fg), false);
    }
    rx -= SEG_GAP;

    // -- Bateria: "73%" texto + icone 14x8.
    let bat_text = format!("{}%", snap.battery_pct);
    let bat_text_w = text_width_px(&bat_text, false);
    let bat_icon_w = 15.3; // body 14 + cap 1.3.
    let bat_gap = 4.0;
    rx -= bat_text_w + bat_gap + bat_icon_w;
    {
        let mut canvas = pixmap.as_mut();
        let bat_color = if snap.battery_pct > 20 {
            opaque(palette.accent)
        } else {
            opaque(palette.fg)
        };
        draw_text(&mut canvas, rx, text_top, &bat_text, bat_color, false);
        draw_battery(&mut canvas, rx + bat_text_w + bat_gap, cy - 4.0, snap.battery_pct, palette);
    }
    rx -= SEG_GAP;

    // -- Wifi icone 14x14.
    rx -= 14.0;
    {
        let mut canvas = pixmap.as_mut();
        draw_wifi(&mut canvas, rx, cy - 7.0, snap.wifi_on, palette);
    }

    let _ = snap.theme; // theme nao usado pra render (so palette).

    // Border-bottom 1px (sutil, cor border palette).
    fill_rect_color(pixmap, 0.0, h - 1.0, snap.width as f32, 1.0, opaque(palette.border));
}

// ============================================================
// Sensors (battery + wifi).
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
// IPC client - lumo-wm.sock
// ============================================================
//
// A14: mantido conectado pra futuro use (dock vai consumir workspaces),
// mas state NAO alimenta mais render da bar (workspaces removidos).

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
                        // A14: ainda tracked pra futuro use (dock), mas nao
                        // alimenta render.
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
    /// Tracked pra futuro use no dock; nao alimenta render A14.
    active_workspace: Arc<AtomicU8>,
    battery_pct: u8,
    wifi_on: bool,
    running: bool,
    first_configured: bool,
    pointer: Option<ThemedPointer>,
    pointer_x: f32,
    ipc_stream: Option<UnixStream>,
    ipc_rx_buf: Vec<u8>,
    /// Paleta cacheada (lida 1x no init). Trocar tema requer reiniciar bar.
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
            for i in 0..n {
                let o = i * 4;
                if o + 3 < dst.len() && o + 3 < src.len() {
                    // tiny-skia entrega RGBA; SHM Argb8888 wayland espera
                    // BGRA em little-endian (B,G,R,A em memoria).
                    dst[o] = src[o + 2];
                    dst[o + 1] = src[o + 1];
                    dst[o + 2] = src[o];
                    dst[o + 3] = src[o + 3];
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
        // A14: sem workspaces, sem power -> sem click handler ativo.
        // Tracking pointer_x mantido pra hover futuro (menus dropdown).
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
        "[lumo-bar] A14 Apple-style; tema = {:?}, accent = #{:06X}, bg = #{:06X}",
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

        // IPC drain @ 125Hz (~8ms). State nao alimenta render mas drena
        // socket pra evitar buffer fill.
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

        // Clock tick @ 1s (minuto pode rolar).
        if last_clock_tick.elapsed() >= Duration::from_secs(1) {
            last_clock_tick = Instant::now();
            state.redraw(&qh);
        }

        // Sensors refresh @ 30s (memory feedback_design_lapidado).
        if last_tick.elapsed() >= Duration::from_secs(30) {
            state.refresh();
            state.redraw(&qh);
            last_tick = Instant::now();
        }
    }
    let _ = active_workspace;
}
