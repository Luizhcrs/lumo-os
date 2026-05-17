//! lumo-bar - top bar Lumo OS via wlr-layer-shell + SHM + tiny-skia.
//!
//! A7: SHM software-rendered, sem GPUI/wgpu (compat com lumo-wm 0.7).
//! A8 (atual): conecta no IPC do lumo-wm pra refletir workspace ativo
//! em tempo real. Click numa pill envia `Switch{to:N}`.
//!
//! Layout (32px alt, full width):
//!   [dot emerald] [Lumo] ... [1 2 3 4 5] ... [wifi] [bat] [HH:MM] [Power]
//!
//! Sem neon (memory feedback_zero_neon_glow). Memory
//! feedback_input_feedback_imediato: click em workspace pill aplica
//! no proximo frame (sem fila invisivel).

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

use lumo_ipc::{default_socket_path, LumoCommand, LumoEvent, MAX_WORKSPACES};

// ============================================================
// Tokens
// ============================================================
const C_BG_TOPBAR: u32 = 0xff0a0a0c;
const C_TEXT: u32 = 0xfff5f5f7;
const C_MUTED: u32 = 0xff9596a0;
const C_ACCENT: u32 = 0xff10b981;
const C_BG_HOVER: u32 = 0x1affffff;
const C_BORDER: u32 = 0x14ffffff;
const C_DANGER: u32 = 0xfff87171;

const BAR_HEIGHT: u32 = 32;
const PAD_X: f32 = 12.0;
const SEG_GAP: f32 = 8.0;
const WORKSPACE_COUNT: u32 = MAX_WORKSPACES as u32;

// Layout dos pills de workspace (calculado tambem no paint_frame).
// Mantido aqui pra hit-test no input handler.
const WS_PILL_SIZE: f32 = 18.0;
const WS_PILL_GAP: f32 = 4.0;
const WS_PILL_STEP: f32 = WS_PILL_SIZE + WS_PILL_GAP;

fn rgba_from_u32(c: u32) -> Color {
    let a = ((c >> 24) & 0xff) as f32 / 255.0;
    let r = ((c >> 16) & 0xff) as f32 / 255.0;
    let g = ((c >> 8) & 0xff) as f32 / 255.0;
    let b = (c & 0xff) as f32 / 255.0;
    Color::from_rgba(r, g, b, a).unwrap()
}

// ============================================================
// Vector primitives (mesmas da versao A7)
// ============================================================
fn fill_circle(canvas: &mut PixmapMut, cx: f32, cy: f32, r: f32, color: u32) {
    let path = PathBuilder::from_circle(cx, cy, r).unwrap();
    let mut p = Paint::default();
    p.set_color(rgba_from_u32(color));
    p.anti_alias = true;
    canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
}

fn fill_rrect(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, r: f32, color: u32) {
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
    p.set_color(rgba_from_u32(color));
    p.anti_alias = true;
    canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
}

fn stroke_circle(canvas: &mut PixmapMut, cx: f32, cy: f32, r: f32, color: u32, sw: f32) {
    let path = PathBuilder::from_circle(cx, cy, r).unwrap();
    let mut p = Paint::default();
    p.set_color(rgba_from_u32(color));
    p.anti_alias = true;
    let st = Stroke {
        width: sw,
        ..Default::default()
    };
    canvas.stroke_path(&path, &p, &st, Transform::identity(), None);
}

fn stroke_rrect(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, r: f32, color: u32, sw: f32) {
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
    p.set_color(rgba_from_u32(color));
    p.anti_alias = true;
    let st = Stroke {
        width: sw,
        ..Default::default()
    };
    canvas.stroke_path(&path, &p, &st, Transform::identity(), None);
}

fn fill_rect_color(pixmap: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, color: u32) {
    let mut p = Paint::default();
    p.set_color(rgba_from_u32(color));
    p.anti_alias = false;
    if let Some(r) = Rect::from_xywh(x, y, w, h) {
        pixmap.fill_rect(r, &p, Transform::identity(), None);
    }
}

fn draw_digit(canvas: &mut PixmapMut, x: f32, y: f32, d: u8, color: u32) {
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

fn draw_brand_dot(canvas: &mut PixmapMut, cx: f32, cy: f32) {
    fill_circle(canvas, cx, cy, 4.0, C_ACCENT);
}

fn draw_workspace(canvas: &mut PixmapMut, x: f32, y: f32, n: u8, active: bool) {
    let bg = if active { C_ACCENT } else { C_BG_HOVER };
    let fg = if active { 0xff0a0a0c } else { C_TEXT };
    fill_rrect(canvas, x, y, WS_PILL_SIZE, WS_PILL_SIZE, 9.0, bg);
    draw_digit(canvas, x + 5.5, y + 3.5, n, fg);
}

fn draw_wifi(canvas: &mut PixmapMut, x: f32, y: f32, on: bool) {
    let c = if on { C_TEXT } else { C_MUTED };
    fill_rrect(canvas, x + 5.0, y + 9.0, 4.0, 2.0, 1.0, c);
    fill_rrect(canvas, x + 3.0, y + 5.5, 8.0, 2.0, 1.0, c);
    fill_rrect(canvas, x + 1.0, y + 2.0, 12.0, 2.0, 1.0, c);
}

fn draw_battery(canvas: &mut PixmapMut, x: f32, y: f32, pct: u8) {
    let c = if pct <= 20 { C_DANGER } else { C_TEXT };
    stroke_rrect(canvas, x, y + 1.0, 20.0, 10.0, 2.0, c, 1.2);
    fill_rrect(canvas, x + 20.5, y + 4.0, 1.5, 4.0, 0.7, c);
    let fw = (pct as f32 / 100.0) * 16.0;
    if fw > 0.0 {
        fill_rrect(canvas, x + 2.0, y + 3.0, fw, 6.0, 1.0, c);
    }
}

fn draw_clock(canvas: &mut PixmapMut, x: f32, y: f32, hh: u8, mm: u8, color: u32) {
    let dx = 9.0;
    draw_digit(canvas, x + dx * 0.0, y, hh / 10, color);
    draw_digit(canvas, x + dx * 1.0, y, hh % 10, color);
    let cx = x + dx * 2.0 + 1.5;
    fill_rrect(canvas, cx, y + 3.0, 1.8, 1.8, 0.9, color);
    fill_rrect(canvas, cx, y + 6.8, 1.8, 1.8, 0.9, color);
    draw_digit(canvas, x + dx * 2.0 + 5.0, y, mm / 10, color);
    draw_digit(canvas, x + dx * 3.0 + 5.0, y, mm % 10, color);
}

fn draw_power(canvas: &mut PixmapMut, x: f32, y: f32) {
    let cx = x + 10.0;
    let cy = y + 10.0;
    stroke_circle(canvas, cx, cy, 7.0, C_TEXT, 1.3);
    fill_rrect(canvas, cx - 1.2, cy - 9.0, 2.4, 5.0, 0.0, C_BG_TOPBAR);
    fill_rrect(canvas, cx - 0.7, cy - 8.5, 1.4, 6.0, 0.7, C_TEXT);
}

struct BarSnapshot {
    width: u32,
    height: u32,
    active_workspace: u32,
    battery_pct: u8,
    wifi_on: bool,
}

/// Calcula x inicial dos pills. Reusado por hit-test (input)
/// e paint_frame (render). Verdade unica = sem drift visual.
fn workspace_layout_origin_x(bar_width: u32) -> f32 {
    let ws_total =
        (WORKSPACE_COUNT as f32) * WS_PILL_SIZE + (WORKSPACE_COUNT as f32 - 1.0) * WS_PILL_GAP;
    (bar_width as f32 - ws_total) / 2.0
}

fn paint_frame(pixmap: &mut Pixmap, snap: &BarSnapshot) {
    pixmap.fill(rgba_from_u32(C_BG_TOPBAR));
    let h = snap.height as f32;
    let cy = h / 2.0;

    {
        let mut canvas = pixmap.as_mut();
        draw_brand_dot(&mut canvas, PAD_X + 4.0, cy);
    }

    let mut wx = workspace_layout_origin_x(snap.width);
    let wy = cy - WS_PILL_SIZE / 2.0;
    {
        let mut canvas = pixmap.as_mut();
        for i in 1..=WORKSPACE_COUNT {
            draw_workspace(&mut canvas, wx, wy, i as u8, i == snap.active_workspace);
            wx += WS_PILL_STEP;
        }
    }

    let mut rx = snap.width as f32 - PAD_X;
    rx -= 20.0;
    {
        let mut canvas = pixmap.as_mut();
        draw_power(&mut canvas, rx, cy - 10.0);
    }
    rx -= SEG_GAP + 4.0;

    let now = Local::now();
    let hh = now.format("%H").to_string().parse::<u8>().unwrap_or(0);
    let mm = now.format("%M").to_string().parse::<u8>().unwrap_or(0);
    rx -= 41.0;
    {
        let mut canvas = pixmap.as_mut();
        draw_clock(&mut canvas, rx, cy - 5.5, hh, mm, C_TEXT);
    }
    rx -= SEG_GAP;

    rx -= 22.0;
    {
        let mut canvas = pixmap.as_mut();
        draw_battery(&mut canvas, rx, cy - 6.0, snap.battery_pct);
    }
    rx -= SEG_GAP;

    rx -= 14.0;
    {
        let mut canvas = pixmap.as_mut();
        draw_wifi(&mut canvas, rx, cy - 6.0, snap.wifi_on);
    }

    fill_rect_color(pixmap, 0.0, h - 1.0, snap.width as f32, 1.0, C_BORDER);
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
// IPC client - lumo-wm.sock
// ============================================================

/// Tenta conectar. Falha silenciosa = bar funciona em standalone.
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

/// Le linhas disponiveis. Aplica em `active_ws` (Arc<AtomicU8>).
/// Memory feedback_input_feedback_imediato: leitura nao-bloqueante,
/// nao acumula; ultimo valor vale.
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

/// Envia LumoCommand::Switch. Failsafe: erro = drop stream.
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
    // Cache pra hit-test - ultimo x do mouse.
    pointer_x: f32,
    ipc_stream: Option<UnixStream>,
    ipc_rx_buf: Vec<u8>,
    last_drawn_ws: u8,
    /// Anti-spam: ultimo click. Memory feedback_input_feedback_imediato
    /// pede drop quando lag > 100ms - aqui anti-doubleclick para
    /// evitar burst de events espuriais (acumular nunca; sempre dropar
    /// se chegou tarde).
    last_click_instant: Option<Instant>,
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
        // Snapshot do estado ANTES de pegar borrow mut do pool.
        // Evita E0502: pool.create_buffer mantem mut borrow ate
        // o final do bloco; chamadas a self.* depois quebram.
        let active = self.current_active().clamp(1, MAX_WORKSPACES) as u32;
        self.last_drawn_ws = active as u8;
        let battery_pct = self.battery_pct;
        let wifi_on = self.wifi_on;
        let bar_w = self.width;
        let bar_h = self.height;

        let stride = bar_w as i32 * 4;
        let (buffer, canvas) = match self.pool.create_buffer(
            bar_w as i32,
            bar_h as i32,
            stride,
            wl_shm::Format::Argb8888,
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[lumo-bar] create_buffer falhou: {e:?}");
                return;
            }
        };

        if let Some(mut px) = Pixmap::new(bar_w, bar_h) {
            let snap = BarSnapshot {
                width: bar_w,
                height: bar_h,
                active_workspace: active,
                battery_pct,
                wifi_on,
            };
            paint_frame(&mut px, &snap);
            let src = px.data();
            let dst = canvas;
            let n = (bar_w * bar_h) as usize;
            for i in 0..n {
                let o = i * 4;
                if o + 3 < dst.len() && o + 3 < src.len() {
                    dst[o] = src[o + 2];
                    dst[o + 1] = src[o + 1];
                    dst[o + 2] = src[o];
                    dst[o + 3] = src[o + 3];
                }
            }
        }

        let surface = self.layer.wl_surface();
        surface.damage_buffer(0, 0, bar_w as i32, bar_h as i32);
        buffer.attach_to(surface).ok();
        surface.commit();
    }

    /// Hit-test: x em pixel logico -> 1..=5 ou None. Mesma origem
    /// usada no paint_frame.
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
        let py = self.height as f32 / 2.0; // hit test linha media
        let now = Instant::now();
        // Drop se < 100ms desde ultimo click (anti-burst espurio).
        if let Some(last) = self.last_click_instant {
            if now.duration_since(last) < Duration::from_millis(100) {
                return;
            }
        }
        self.last_click_instant = Some(now);
        if let Some(target) = self.hit_workspace(self.pointer_x, py) {
            // Local feedback imediato + envia comando.
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
                    // BTN_LEFT = 0x110
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
    // Bar precisa receber pointer events pra responder clicks em workspace.
    // Keyboard nao precisa (sem text input na bar).
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.commit();

    let pool = SlotPool::new(1920 * BAR_HEIGHT as usize * 4 * 2, &shm)
        .expect("SlotPool init");

    let active_workspace = Arc::new(AtomicU8::new(1));

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
    };

    let mut last_tick = Instant::now();
    let mut last_ipc_tick = Instant::now();
    while state.running {
        conn.flush().ok();
        // dispatch non-blocking pra dar room aos ticks IPC + refresh.
        // 8ms timeout = ~125Hz polling, latencia subjetiva imediata.
        queue
            .blocking_dispatch(&mut state)
            .expect("dispatch fail");

        // IPC tick: drain de eventos do compositor.
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
            // Re-render se workspace mudou via IPC.
            let now_ws = state.current_active();
            if now_ws != state.last_drawn_ws {
                state.redraw(&qh);
            }
        }

        if last_tick.elapsed() >= Duration::from_secs(15) {
            state.refresh();
            state.redraw(&qh);
            last_tick = Instant::now();
        }
    }
    let _ = active_workspace;
}
