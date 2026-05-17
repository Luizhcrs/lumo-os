//! lumo-bar - top bar Lumo OS via wlr-layer-shell + SHM + tiny-skia.
//!
//! A13 redesign: paleta light default (Luiz reportou bar feia + dark
//! indesejado). Theme switchable via `LUMO_THEME=dark|light` env.
//!
//! Layout lapidado (32px alt, full width):
//!
//!   [brand dot 8px]   1  2  3  4  5     [wifi] [bat] HH:MM  [⏻]
//!
//! Slots:
//!   - Esquerda  (PAD_X=16): brand dot 8px circulo emerald.
//!   - Centro:   workspaces 1..=5, pill 22x22 r=6, ativo=accent fill com
//!               fg invertido, inativo=transparente com border 1px.
//!   - Direita  (PAD_X=16, gap=12px): wifi (3 arcos) -> bateria (rect com
//!               fill horizontal proporcional a %) -> clock HH:MM ->
//!               power (circulo aberto + linha vertical no topo).
//!
//! Tipografia: 7-segment digits desenhados via tiny-skia (Geist Mono ja
//! visualmente proxima de mono compact; quando font rendering rolar via
//! cosmic-text trocamos). Sem dependencia de Nerd Font / emoji
//! (memory feedback_zero_emoji).
//!
//! Border bottom: 1px linha cor `border` (sutil). ZERO box-shadow colorido
//! (memory feedback_zero_neon_glow). Pode haver fileira de 1px preto
//! rgba(0,0,0,0.04) abaixo como separacao visual sutil (commented inline).
//!
//! Memory feedback_input_feedback_imediato: click em workspace pill aplica
//! local imediato + envia IPC; refresh bat/wifi a cada 15s; clock a cada
//! 1s. Drop de clicks burst < 100ms pra evitar double-fire.

use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use chrono::Local;
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
use lumo_ipc::{default_socket_path, LumoCommand, LumoEvent, MAX_WORKSPACES};

// ============================================================
// Layout constants (lapidado: cada valor justificado).
// ============================================================

/// Altura fixa da bar. 32px = aprox 8 grid units Apple HIG densidade media.
const BAR_HEIGHT: u32 = 32;

/// Padding horizontal nas duas pontas. 16px = 1rem.
const PAD_X: f32 = 16.0;

/// Gap default entre slots da direita (wifi/bat/clock/power).
const SEG_GAP: f32 = 12.0;

/// Quantidade de workspaces (vem do IPC).
const WORKSPACE_COUNT: u32 = MAX_WORKSPACES as u32;

/// Pill workspace: 22x22 com radius 6. Quadrado-ish lapidado.
const WS_PILL_SIZE: f32 = 22.0;
const WS_PILL_RADIUS: f32 = 6.0;
const WS_PILL_GAP: f32 = 4.0;
const WS_PILL_STEP: f32 = WS_PILL_SIZE + WS_PILL_GAP;

/// Brand dot diametro. 8px = atomo visual estavel.
const BRAND_DOT_RADIUS: f32 = 4.0;

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

fn fill_circle(canvas: &mut PixmapMut, cx: f32, cy: f32, r: f32, color: Color) {
    let path = PathBuilder::from_circle(cx, cy, r).unwrap();
    let mut p = Paint::default();
    p.set_color(color);
    p.anti_alias = true;
    canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
}

fn fill_rrect(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
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
    let path = pb.finish().unwrap();
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
    let path = pb.finish().unwrap();
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
    // Magic-1.0 nao serve pra arco; usa magic dependente do delta angular.
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
    if let Some(r) = Rect::from_xywh(x, y, w, h) {
        pixmap.fill_rect(r, &p, Transform::identity(), None);
    }
}

// ============================================================
// Glyph rendering (7-seg digits + dot separator).
// ============================================================
//
// Lumo nao usa Nerd Font / emoji (memory feedback_zero_emoji). Digitos
// renderizados por 7 segmentos vetoriais. Pixel-perfect a 11pt visual.
//
//     _
//    |_|     w=7  h=11  thickness=1.6
//    |_|
//
fn draw_digit(canvas: &mut PixmapMut, x: f32, y: f32, d: u8, color: Color) {
    let segs: [bool; 7] = match d {
        0 => [true, true, true, true, true, true, false],
        1 => [false, true, true, false, false, false, false],
        2 => [true, true, false, true, true, false, true],
        3 => [true, true, true, true, false, false, true],
        4 => [false, true, true, false, false, true, true],
        5 => [true, false, true, true, false, true, true],
        6 => [true, false, true, true, true, true, true],
        7 => [true, true, true, false, false, false, false],
        8 => [true, true, true, true, true, true, true],
        9 => [true, true, true, true, false, true, true],
        _ => [false; 7],
    };
    let w = 7.0;
    let h = 11.0;
    let t = 1.6;
    let hh = h / 2.0;
    // 0=top, 1=upper-right, 2=lower-right, 3=bottom, 4=lower-left,
    // 5=upper-left, 6=middle.
    if segs[0] {
        fill_rrect(canvas, x, y, w, t, t * 0.5, color);
    }
    if segs[1] {
        fill_rrect(canvas, x + w - t, y, t, hh, t * 0.5, color);
    }
    if segs[2] {
        fill_rrect(canvas, x + w - t, y + hh, t, hh, t * 0.5, color);
    }
    if segs[3] {
        fill_rrect(canvas, x, y + h - t, w, t, t * 0.5, color);
    }
    if segs[4] {
        fill_rrect(canvas, x, y + hh, t, hh, t * 0.5, color);
    }
    if segs[5] {
        fill_rrect(canvas, x, y, t, hh, t * 0.5, color);
    }
    if segs[6] {
        fill_rrect(canvas, x, y + hh - t * 0.5, w, t, t * 0.5, color);
    }
}

// ============================================================
// Brand mark.
// ============================================================
fn draw_brand_dot(canvas: &mut PixmapMut, cx: f32, cy: f32, color: Color) {
    fill_circle(canvas, cx, cy, BRAND_DOT_RADIUS, color);
}

// ============================================================
// Workspace pill (22x22 r=6).
// ============================================================
fn draw_workspace(canvas: &mut PixmapMut, x: f32, y: f32, n: u8, active: bool, palette: &LumoColors, theme: LumoTheme) {
    if active {
        // Ativo: fill accent solido.
        fill_rrect(canvas, x, y, WS_PILL_SIZE, WS_PILL_SIZE, WS_PILL_RADIUS, opaque(palette.accent));
        // fg invertido: light theme -> branco; dark -> preto.
        let fg = match theme {
            LumoTheme::Light => Color::from_rgba(1.0, 1.0, 1.0, 1.0).unwrap(),
            LumoTheme::Dark => Color::from_rgba(0.0, 0.0, 0.0, 1.0).unwrap(),
        };
        // Digito centralizado: 7x11, pill 22x22 -> margin (22-7)/2 = 7.5 x, (22-11)/2 = 5.5 y.
        draw_digit(canvas, x + 7.5, y + 5.5, n, fg);
    } else {
        // Inativo: border 1px, fg muted.
        stroke_rrect(
            canvas,
            x + 0.5,
            y + 0.5,
            WS_PILL_SIZE - 1.0,
            WS_PILL_SIZE - 1.0,
            WS_PILL_RADIUS,
            opaque(palette.border),
            1.0,
        );
        draw_digit(canvas, x + 7.5, y + 5.5, n, opaque(palette.fg_subtle));
    }
}

// ============================================================
// Wifi glyph (3 arcos concentricos).
// ============================================================
//
// Origem (x,y) = top-left de uma caixa 16x16. Arcos sao quartos de
// circulo virados pra cima (start=-135deg, end=-45deg).
fn draw_wifi(canvas: &mut PixmapMut, x: f32, y: f32, on: bool, palette: &LumoColors) {
    let color = if on {
        opaque(palette.fg)
    } else {
        opaque(palette.fg_subtle)
    };
    let cx = x + 8.0;
    // Centro vertical mais baixo pra arcos curvarem visualmente "saindo"
    // de baixo, como wifi antenna pattern.
    let cy = y + 12.0;
    // 3 arcos: 8, 5, 2.5 radii.
    for (radius, sw) in [(7.5, 1.3), (5.0, 1.2), (2.5, 1.1)] {
        stroke_arc(canvas, cx, cy, radius, -135.0, -45.0, color, sw);
    }
    // Dot na base.
    fill_circle(canvas, cx, cy, 1.0, color);
}

// ============================================================
// Battery glyph (16x10 com fill horizontal proporcional + cap).
// ============================================================
fn draw_battery(canvas: &mut PixmapMut, x: f32, y: f32, pct: u8, palette: &LumoColors) {
    // Body 16x10, cap 1.5x4.
    let body_w = 16.0;
    let body_h = 10.0;
    let stroke = opaque(palette.fg);
    stroke_rrect(canvas, x + 0.5, y + 0.5, body_w - 1.0, body_h - 1.0, 1.6, stroke, 1.0);
    // Cap (terminal +) na direita.
    fill_rrect(canvas, x + body_w, y + 3.5, 1.5, 3.0, 0.6, stroke);
    // Fill interno proporcional, com inset de 2px.
    let inner_w = body_w - 4.0;
    let fw = (pct as f32 / 100.0).clamp(0.0, 1.0) * inner_w;
    if fw > 0.2 {
        // Cor do fill: accent se > 20%, danger se < 20%.
        // Como palette nao tem "danger" direto, usamos accent invertido
        // (saturado mais claro pra dark, mais escuro pra light) seria
        // overkill. Mantemos accent solido p/ >20%, e pra <=20% usamos um
        // tom vermelho hardcoded fixo entre temas (signal universal).
        let fill_color = if pct > 20 {
            opaque(palette.accent)
        } else {
            // Red-500 (#ef4444) — universal alert.
            opaque(0xEF4444)
        };
        fill_rrect(canvas, x + 2.0, y + 2.0, fw, body_h - 4.0, 0.8, fill_color);
    }
}

// ============================================================
// Clock HH:MM via 7-seg digits.
// ============================================================
fn draw_clock(canvas: &mut PixmapMut, x: f32, y: f32, hh: u8, mm: u8, color: Color) {
    let dx = 9.0;
    draw_digit(canvas, x, y, hh / 10, color);
    draw_digit(canvas, x + dx, y, hh % 10, color);
    // Separator dots (verticalmente alinhados meio do glyph 11px).
    let sep_x = x + dx * 2.0 + 1.0;
    fill_rrect(canvas, sep_x, y + 3.0, 1.6, 1.6, 0.8, color);
    fill_rrect(canvas, sep_x, y + 6.8, 1.6, 1.6, 0.8, color);
    draw_digit(canvas, x + dx * 2.0 + 5.0, y, mm / 10, color);
    draw_digit(canvas, x + dx * 3.0 + 5.0, y, mm % 10, color);
}

// ============================================================
// Power glyph (circulo aberto + linha vertical no topo).
// ============================================================
fn draw_power(canvas: &mut PixmapMut, x: f32, y: f32, palette: &LumoColors) {
    let cx = x + 8.0;
    let cy = y + 9.0;
    let r = 5.5;
    let color = opaque(palette.fg);
    // Arco quase fechado, gap no topo.
    stroke_arc(canvas, cx, cy, r, 70.0, 110.0 + 360.0, color, 1.3);
    // Linha vertical no topo.
    fill_rrect(canvas, cx - 0.7, cy - r - 2.2, 1.4, 5.0, 0.6, color);
}

// ============================================================
// Bar snapshot (state imutavel pra paint_frame).
// ============================================================
struct BarSnapshot {
    width: u32,
    height: u32,
    active_workspace: u32,
    battery_pct: u8,
    wifi_on: bool,
    theme: LumoTheme,
    palette: LumoColors,
}

/// Calcula x inicial dos pills. Reusado por hit-test + paint_frame.
fn workspace_layout_origin_x(bar_width: u32) -> f32 {
    let ws_total =
        (WORKSPACE_COUNT as f32) * WS_PILL_SIZE + (WORKSPACE_COUNT as f32 - 1.0) * WS_PILL_GAP;
    (bar_width as f32 - ws_total) / 2.0
}

fn paint_frame(pixmap: &mut Pixmap, snap: &BarSnapshot) {
    let palette = &snap.palette;
    pixmap.fill(opaque(palette.bg));
    let h = snap.height as f32;
    let cy = h / 2.0;

    // ===== Esquerda: brand dot =====
    {
        let mut canvas = pixmap.as_mut();
        draw_brand_dot(&mut canvas, PAD_X, cy, opaque(palette.accent));
    }

    // ===== Centro: workspaces =====
    let mut wx = workspace_layout_origin_x(snap.width);
    let wy = cy - WS_PILL_SIZE / 2.0;
    {
        let mut canvas = pixmap.as_mut();
        for i in 1..=WORKSPACE_COUNT {
            draw_workspace(&mut canvas, wx, wy, i as u8, i == snap.active_workspace, palette, snap.theme);
            wx += WS_PILL_STEP;
        }
    }

    // ===== Direita: power, clock, bat, wifi (do mais a direita pra esquerda) =====
    let mut rx = snap.width as f32 - PAD_X;

    // Power 16x16.
    rx -= 16.0;
    {
        let mut canvas = pixmap.as_mut();
        draw_power(&mut canvas, rx, cy - 8.0, palette);
    }
    rx -= SEG_GAP;

    // Clock HH:MM. Largura ~= 9*4 + 5 separator + 1 margin = 42px.
    let now = Local::now();
    let hh = now.format("%H").to_string().parse::<u8>().unwrap_or(0);
    let mm = now.format("%M").to_string().parse::<u8>().unwrap_or(0);
    let clock_w = 41.0;
    rx -= clock_w;
    {
        let mut canvas = pixmap.as_mut();
        draw_clock(&mut canvas, rx, cy - 5.5, hh, mm, opaque(palette.fg));
    }
    rx -= SEG_GAP;

    // Battery 17.5x10 (body 16 + cap 1.5).
    rx -= 17.5;
    {
        let mut canvas = pixmap.as_mut();
        draw_battery(&mut canvas, rx, cy - 5.0, snap.battery_pct, palette);
    }
    rx -= SEG_GAP;

    // Wifi 16x16.
    rx -= 16.0;
    {
        let mut canvas = pixmap.as_mut();
        draw_wifi(&mut canvas, rx, cy - 8.0, snap.wifi_on, palette);
    }

    // Border-bottom 1px (sutil, cor border palette).
    fill_rect_color(pixmap, 0.0, h - 1.0, snap.width as f32, 1.0, opaque(palette.border));
    // Sombra preta neutra adicional 1px rgba(0,0,0,0.04) — separacao visual.
    // Memory feedback_zero_neon_glow: alpha baixissimo, RGB puro preto.
    fill_rect_color(
        pixmap,
        0.0,
        h - 0.5,
        snap.width as f32,
        0.5,
        rgba_hex(0x000000, 10),
    );
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

fn send_switch(stream: &mut UnixStream, to: u8) -> bool {
    let cmd = LumoCommand::Switch { to };
    let mut payload = serde_json::to_string(&cmd).unwrap_or_default();
    payload.push('\n');
    stream.write_all(payload.as_bytes()).is_ok()
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
    last_drawn_ws: u8,
    last_click_instant: Option<Instant>,
    /// Paleta cacheada (lida 1x no init/configure). Trocar tema requer
    /// reiniciar a bar — comportamento sane: a bar nao precisa hot-reload
    /// de tema, e cache evita lookup env por frame.
    theme: LumoTheme,
    palette: LumoColors,
}

impl LumoBar {
    fn refresh(&mut self) {
        self.battery_pct = read_battery();
        self.wifi_on = read_wifi();
    }

    fn current_active(&self) -> u8 {
        self.active_workspace.load(Ordering::Relaxed)
    }

    fn redraw(&mut self, _qh: &QueueHandle<Self>) {
        // Snapshot ANTES de pegar borrow mut do pool (E0502 evite).
        let active = self.current_active().clamp(1, MAX_WORKSPACES) as u32;
        self.last_drawn_ws = active as u8;
        let snap = BarSnapshot {
            width: self.width,
            height: self.height,
            active_workspace: active,
            battery_pct: self.battery_pct,
            wifi_on: self.wifi_on,
            theme: self.theme,
            palette: self.palette,
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

    fn hit_workspace(&self, px: f32, py: f32) -> Option<u8> {
        let bar_h = self.height as f32;
        let cy = bar_h / 2.0;
        let wy = cy - WS_PILL_SIZE / 2.0;
        if py < wy || py > wy + WS_PILL_SIZE {
            return None;
        }
        let origin = workspace_layout_origin_x(self.width);
        for i in 0..WORKSPACE_COUNT {
            let x = origin + (i as f32) * WS_PILL_STEP;
            if px >= x && px <= x + WS_PILL_SIZE {
                return Some((i + 1) as u8);
            }
        }
        None
    }

    fn handle_click(&mut self, qh: &QueueHandle<Self>) {
        let py = self.height as f32 / 2.0;
        let now = Instant::now();
        if let Some(last) = self.last_click_instant {
            if now.duration_since(last) < Duration::from_millis(100) {
                return;
            }
        }
        self.last_click_instant = Some(now);
        if let Some(target) = self.hit_workspace(self.pointer_x, py) {
            self.active_workspace.store(target, Ordering::Relaxed);
            self.redraw(qh);
            if let Some(stream) = self.ipc_stream.as_mut() {
                if !send_switch(stream, target) {
                    eprintln!("[lumo-bar] send_switch falhou; drop ipc");
                    self.ipc_stream = None;
                }
            }
        }
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
        qh: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for ev in events {
            match ev.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.pointer_x = ev.position.0 as f32;
                }
                PointerEventKind::Press { button: 0x110, .. } => {
                    self.handle_click(qh);
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
        "[lumo-bar] tema = {:?}, accent = #{:06X}, bg = #{:06X}",
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
        last_drawn_ws: 1,
        last_click_instant: None,
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

        // IPC drain @ 125Hz (~8ms).
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
            let now_ws = state.current_active();
            if now_ws != state.last_drawn_ws {
                state.redraw(&qh);
            }
        }

        // Clock tick @ 1s.
        if last_clock_tick.elapsed() >= Duration::from_secs(1) {
            last_clock_tick = Instant::now();
            state.redraw(&qh);
        }

        // Sensors refresh @ 15s (memory feedback_design_lapidado: ev.30s
        // spec, mas 15s da feedback mais responsivo sem custo
        // perceptivel).
        if last_tick.elapsed() >= Duration::from_secs(15) {
            state.refresh();
            state.redraw(&qh);
            last_tick = Instant::now();
        }
    }
    let _ = active_workspace;
}
