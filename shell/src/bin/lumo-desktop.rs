//! lumo-desktop - layer-shell Background pra capturar pointer events
//! na area de trabalho (estilo macOS Finder / Windows desktop).
//!
//! A21: novo binario.
//!
//! Comportamento:
//!   - Layer::Background full-screen (1920x1080 Galaxy), atras de
//!     toplevels e da bar.
//!   - Surface 100% transparente: nao desenha NADA quando menu fechado;
//!     wallpaper do compositor aparece atraves.
//!   - Click esquerdo em area vazia: envia LumoCommand::CloseDropdowns
//!     pelo socket IPC. Compositor traduz em LumoEvent::CloseDropdowns
//!     pra bar.
//!   - Click direito: abre menu contextual rrect 200x180 com 3 items
//!     (Configuracoes / Trocar wallpaper / Sobre Lumo). Outro click
//!     esquerdo fecha o menu.
//!   - Toplevels (foot/firefox) e bar (Layer::Top) vem POR CIMA: clicks
//!     neles sao roteados normal pelo compositor antes de chegar aqui.
//!
//! Memory feedback_zero_neon_glow: menu com sombra preta neutra, sem
//! glow accent. Memory feedback_design_lapidado: cada constante com
//! justificativa abaixo.

use std::io::{ErrorKind, Write};
use std::os::unix::net::UnixStream;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

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
        pointer::{
            PointerEvent, PointerEventKind, PointerHandler, ThemedPointer, BTN_LEFT, BTN_RIGHT,
        },
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
use smithay_client_toolkit::reexports::client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};
use tiny_skia::{Color, FillRule, Paint, PathBuilder, Pixmap, PixmapMut, Transform};

use lumo_ipc::{default_socket_path, LumoCommand};

// ============================================================
// Layout constants (lapidado).
// ============================================================

/// Output Galaxy nativo. Default + fallback caso configure callback
/// nao traga size cedo (DEPS.md A19.18: bar tem mesmo padrao).
const OUTPUT_W: u32 = 1920;
const OUTPUT_H: u32 = 1080;

/// Menu contextual.
/// Largura 200 = cabe os 3 textos curtos com folga (FONT 13 + pad 16).
const MENU_W: f32 = 200.0;
/// Altura 180 = 3 rows 40px + header 14 + bottom 16 + margem visual.
const MENU_H: f32 = 180.0;
/// Border-radius. 12 = consistencia com pill (14) mas mais discreto.
const MENU_RADIUS: f32 = 12.0;
/// Padding interno horizontal. 16 = respiracao Apple-grade.
const MENU_PAD_X: f32 = 16.0;
/// Padding interno vertical (topo/base). 12 = visualmente equilibrado.
const MENU_PAD_Y: f32 = 12.0;
/// Altura por row clicavel. 40 = touch-friendly + mouse-friendly.
const MENU_ROW_H: f32 = 40.0;
/// Font size dos items.
const FONT_MENU: f32 = 13.0;
/// Margem entre cursor e canto do menu (offset visual).
const MENU_OFFSET: f32 = 2.0;

// Cores (sem glow / neon - memory feedback_zero_neon_glow).
fn menu_bg() -> Color {
    Color::from_rgba(0.10, 0.10, 0.12, 0.96).unwrap()
}
fn menu_border() -> Color {
    Color::from_rgba(0.25, 0.25, 0.28, 1.0).unwrap()
}
fn menu_text() -> Color {
    Color::from_rgba(0.93, 0.93, 0.95, 1.0).unwrap()
}
fn menu_hover_bg() -> Color {
    Color::from_rgba(1.0, 1.0, 1.0, 0.06).unwrap()
}

// ============================================================
// FontSystem singleton.
// ============================================================

static FONT_SYSTEM: OnceLock<Mutex<FontSystem>> = OnceLock::new();
static SWASH_CACHE: OnceLock<Mutex<SwashCache>> = OnceLock::new();

fn font_system() -> &'static Mutex<FontSystem> {
    FONT_SYSTEM.get_or_init(|| Mutex::new(FontSystem::new()))
}

fn swash_cache() -> &'static Mutex<SwashCache> {
    SWASH_CACHE.get_or_init(|| Mutex::new(SwashCache::new()))
}

fn to_cosmic(c: Color) -> CosmicColor {
    let r = (c.red() * 255.0).round().clamp(0.0, 255.0) as u8;
    let g = (c.green() * 255.0).round().clamp(0.0, 255.0) as u8;
    let b = (c.blue() * 255.0).round().clamp(0.0, 255.0) as u8;
    let a = (c.alpha() * 255.0).round().clamp(0.0, 255.0) as u8;
    CosmicColor::rgba(r, g, b, a)
}

// ============================================================
// Drawing helpers.
// ============================================================

fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<tiny_skia::Path> {
    let mut pb = PathBuilder::new();
    let r = r.min(w / 2.0).min(h / 2.0);
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
    pb.finish()
}

fn fill_rrect(canvas: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    if let Some(path) = rrect_path(x, y, w, h, r) {
        let mut p = Paint::default();
        p.set_color(color);
        p.anti_alias = true;
        canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
    }
}

fn draw_text(
    canvas: &mut PixmapMut,
    x: f32,
    y: f32,
    text: &str,
    size: f32,
    color: Color,
) {
    let fs_mutex = font_system();
    let sc_mutex = swash_cache();
    let mut fs = fs_mutex.lock().unwrap();
    let mut sc = sc_mutex.lock().unwrap();
    let metrics = Metrics::new(size, size * 1.4);
    let mut buffer = CosmicBuffer::new(&mut fs, metrics);
    let attrs = Attrs::new().family(Family::Monospace);
    buffer.set_size(&mut fs, Some(f32::INFINITY), Some(size * 1.4));
    buffer.set_text(&mut fs, text, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(&mut fs, false);
    let cosmic_color = to_cosmic(color);
    let cw = canvas.width() as i32;
    let ch = canvas.height() as i32;
    buffer.draw(&mut fs, &mut sc, cosmic_color, |gx, gy, gw, gh, c| {
        let alpha = c.a();
        if alpha == 0 {
            return;
        }
        let fx = (x as i32) + gx;
        let fy = (y as i32) + gy;
        for dy in 0..gh as i32 {
            for dx in 0..gw as i32 {
                let px = fx + dx;
                let py = fy + dy;
                if px < 0 || py < 0 || px >= cw || py >= ch {
                    continue;
                }
                let idx = ((py as u32 * canvas.width() + px as u32) * 4) as usize;
                let data = canvas.data_mut();
                if idx + 3 >= data.len() {
                    continue;
                }
                let a = (alpha as f32) / 255.0;
                let r = (color.red() * 255.0 * a) as u8;
                let g = (color.green() * 255.0 * a) as u8;
                let b = (color.blue() * 255.0 * a) as u8;
                let aa = (a * 255.0) as u8;
                let inv = 1.0 - a;
                data[idx] = r.saturating_add((data[idx] as f32 * inv) as u8);
                data[idx + 1] = g.saturating_add((data[idx + 1] as f32 * inv) as u8);
                data[idx + 2] = b.saturating_add((data[idx + 2] as f32 * inv) as u8);
                data[idx + 3] = aa.saturating_add((data[idx + 3] as f32 * inv) as u8);
            }
        }
    });
}

// ============================================================
// IPC: send LumoCommand::CloseDropdowns to compositor.
// ============================================================

fn connect_ipc() -> Option<UnixStream> {
    let path = default_socket_path()?;
    match UnixStream::connect(&path) {
        Ok(s) => {
            s.set_nonblocking(true).ok()?;
            eprintln!("[lumo-desktop] IPC conectado em {}", path.display());
            Some(s)
        }
        Err(e) => {
            eprintln!("[lumo-desktop] IPC nao conectou ({}): area de trabalho passiva", e);
            None
        }
    }
}

fn send_close_dropdowns(stream: &mut Option<UnixStream>) {
    let Some(s) = stream.as_mut() else { return };
    let mut payload = match serde_json::to_string(&LumoCommand::CloseDropdowns) {
        Ok(s) => s,
        Err(_) => return,
    };
    payload.push('\n');
    if let Err(e) = s.write_all(payload.as_bytes()) {
        if e.kind() != ErrorKind::WouldBlock {
            eprintln!("[lumo-desktop] IPC write erro: {}; dropando socket", e);
            *stream = None;
        }
    }
}

// ============================================================
// LumoDesktop state + handlers.
// ============================================================

#[derive(Debug, Clone, Copy)]
struct MenuActive {
    visible: bool,
    x: f32,
    y: f32,
    hover_row: i32, // -1 = nenhuma
}

const MENU_ITEMS: [&str; 3] = ["Configuracoes", "Trocar wallpaper", "Sobre Lumo"];

struct LumoDesktop {
    registry: RegistryState,
    output_state: OutputState,
    shm: Shm,
    seat_state: SeatState,
    layer: LayerSurface,
    pool: SlotPool,
    width: u32,
    height: u32,
    running: bool,
    first_configured: bool,
    pointer: Option<ThemedPointer>,
    pointer_pos: Option<(f64, f64)>,
    menu: MenuActive,
    ipc_stream: Option<UnixStream>,
    last_click_at: Option<Instant>,
}

fn clamp_menu_origin(x: f32, y: f32, surf_w: u32, surf_h: u32) -> (f32, f32) {
    let mut mx = x + MENU_OFFSET;
    let mut my = y + MENU_OFFSET;
    if mx + MENU_W > surf_w as f32 {
        mx = (x - MENU_W - MENU_OFFSET).max(0.0);
    }
    if my + MENU_H > surf_h as f32 {
        my = (y - MENU_H - MENU_OFFSET).max(0.0);
    }
    (mx, my)
}

fn paint_menu_at(canvas: &mut PixmapMut, menu: MenuActive, surf_w: u32, surf_h: u32) {
    let (mx, my) = clamp_menu_origin(menu.x, menu.y, surf_w, surf_h);
    // Sombra: 4 rrects offset 1..4 alpha decrescente (blur fake 4px).
    // Memory feedback_zero_neon_glow: preto neutro, sem accent.
    for k in 1..=4 {
        let base = 0.55_f32;
        let alpha = (base * (1.0 - (k as f32 - 1.0) * 0.18)).max(0.0);
        let c = Color::from_rgba(0.0, 0.0, 0.0, alpha).unwrap();
        fill_rrect(
            canvas,
            mx - 1.0,
            my + k as f32,
            MENU_W + 2.0,
            MENU_H,
            MENU_RADIUS,
            c,
        );
    }
    fill_rrect(canvas, mx, my, MENU_W, MENU_H, MENU_RADIUS, menu_bg());
    if let Some(path) =
        rrect_path(mx + 0.5, my + 0.5, MENU_W - 1.0, MENU_H - 1.0, MENU_RADIUS - 0.5)
    {
        let mut p = Paint::default();
        p.set_color(menu_border());
        p.anti_alias = true;
        let stroke = tiny_skia::Stroke {
            width: 1.0,
            ..Default::default()
        };
        canvas.stroke_path(&path, &p, &stroke, Transform::identity(), None);
    }
    for (i, label) in MENU_ITEMS.iter().enumerate() {
        let row_y = my + MENU_PAD_Y + (i as f32) * MENU_ROW_H;
        if menu.hover_row == i as i32 {
            fill_rrect(
                canvas,
                mx + 4.0,
                row_y,
                MENU_W - 8.0,
                MENU_ROW_H - 4.0,
                8.0,
                menu_hover_bg(),
            );
        }
        let text_y = row_y + (MENU_ROW_H - FONT_MENU * 1.4) / 2.0;
        draw_text(canvas, mx + MENU_PAD_X, text_y, label, FONT_MENU, menu_text());
    }
}

impl LumoDesktop {
    fn redraw(&mut self, _qh: &QueueHandle<Self>) {
        let stride = self.width as i32 * 4;
        let (buffer, canvas) = match self.pool.create_buffer(
            self.width as i32,
            self.height as i32,
            stride,
            wl_shm::Format::Argb8888,
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[lumo-desktop] create_buffer falhou: {e:?}");
                return;
            }
        };

        // Copia state de menu pro stack — evita reborrow imutavel de self
        // enquanto self.pool ainda esta mut-borrowed pelo create_buffer.
        let menu_snap = self.menu;
        let surf_w = self.width;
        let surf_h = self.height;
        if let Some(mut px) = Pixmap::new(self.width, self.height) {
            // Surface 100% transparente por padrao. tiny-skia Pixmap::new
            // ja zera = ja transparente. So desenha quando menu visivel.
            if menu_snap.visible {
                let mut canvas_mut = px.as_mut();
                paint_menu_at(&mut canvas_mut, menu_snap, surf_w, surf_h);
            }
            let src = px.data();
            let dst = canvas;
            let n = (self.width * self.height) as usize;
            // tiny-skia RGBA premul -> wl_shm Argb8888 LE = BGRA na memoria.
            // Swap canais; alpha preservado (DEPS.md A15.1).
            for i in 0..n {
                let o = i * 4;
                if o + 3 < dst.len() && o + 3 < src.len() {
                    dst[o] = src[o + 2]; // B
                    dst[o + 1] = src[o + 1]; // G
                    dst[o + 2] = src[o]; // R
                    dst[o + 3] = src[o + 3]; // A
                }
            }
        }

        let surface = self.layer.wl_surface();
        surface.damage_buffer(0, 0, self.width as i32, self.height as i32);
        buffer.attach_to(surface).ok();
        surface.commit();
    }


    fn clamp_menu_origin(&self, x: f32, y: f32) -> (f32, f32) {
        clamp_menu_origin(x, y, self.width, self.height)
    }

    fn hit_test_menu(&self, px: f32, py: f32) -> Option<i32> {
        if !self.menu.visible {
            return None;
        }
        let (mx, my) = self.clamp_menu_origin(self.menu.x, self.menu.y);
        if px < mx || px > mx + MENU_W || py < my || py > my + MENU_H {
            return None;
        }
        for i in 0..MENU_ITEMS.len() {
            let row_y = my + MENU_PAD_Y + (i as f32) * MENU_ROW_H;
            if py >= row_y && py <= row_y + MENU_ROW_H {
                return Some(i as i32);
            }
        }
        Some(-1)
    }
}

impl CompositorHandler for LumoDesktop {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        self.redraw(qh);
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}

impl OutputHandler for LumoDesktop {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}

impl LayerShellHandler for LumoDesktop {
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
        // A21: forca OUTPUT default se compositor mandar 0 (size=(0,0) +
        // Anchor full = compositor pode mandar zero antes do output state estabilizar).
        self.width = if w > 0 { w } else { OUTPUT_W };
        self.height = if h > 0 { h } else { OUTPUT_H };
        self.first_configured = true;
        eprintln!("[lumo-desktop] configured cfg_size=({},{}) using=({},{})", w, h, self.width, self.height);
        self.redraw(qh);
    }
}

impl ShmHandler for LumoDesktop {
    fn shm_state(&mut self) -> &mut Shm { &mut self.shm }
}

impl SeatHandler for LumoDesktop {
    fn seat_state(&mut self) -> &mut SeatState { &mut self.seat_state }
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
                eprintln!("[lumo-desktop] pointer ThemedPointer adquirido");
            }
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, _: Capability) {}
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl PointerHandler for LumoDesktop {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        let mut need_redraw = false;
        for ev in events {
            match ev.kind {
                PointerEventKind::Enter { .. } | PointerEventKind::Motion { .. } => {
                    self.pointer_pos = Some(ev.position);
                    if self.menu.visible {
                        let row = self
                            .hit_test_menu(ev.position.0 as f32, ev.position.1 as f32)
                            .unwrap_or(-1);
                        if row != self.menu.hover_row {
                            self.menu.hover_row = row;
                            need_redraw = true;
                        }
                    }
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer_pos = None;
                    if self.menu.visible && self.menu.hover_row != -1 {
                        self.menu.hover_row = -1;
                        need_redraw = true;
                    }
                }
                PointerEventKind::Press { button, .. } => {
                    let (px, py) = (ev.position.0 as f32, ev.position.1 as f32);
                    // Debounce 150ms (espelha pattern bar A20.10).
                    let now = Instant::now();
                    if let Some(last) = self.last_click_at {
                        if now.duration_since(last) < Duration::from_millis(150) {
                            continue;
                        }
                    }
                    self.last_click_at = Some(now);

                    if button == BTN_RIGHT {
                        self.menu = MenuActive {
                            visible: true,
                            x: px,
                            y: py,
                            hover_row: -1,
                        };
                        need_redraw = true;
                        eprintln!("[lumo-desktop] right-click ({}, {}) -> menu open", px, py);
                    } else if button == BTN_LEFT {
                        if self.menu.visible {
                            let row = self.hit_test_menu(px, py).unwrap_or(-1);
                            if row >= 0 {
                                eprintln!(
                                    "[lumo-desktop] menu item={} '{}' (stub)",
                                    row, MENU_ITEMS[row as usize]
                                );
                            }
                            self.menu.visible = false;
                            self.menu.hover_row = -1;
                            need_redraw = true;
                        } else {
                            send_close_dropdowns(&mut self.ipc_stream);
                            eprintln!("[lumo-desktop] left-click empty -> CloseDropdowns IPC");
                        }
                    }
                }
                _ => {}
            }
        }
        if need_redraw {
            self.redraw(qh);
        }
    }
}

impl ProvidesRegistryState for LumoDesktop {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry }
    registry_handlers!(OutputState, SeatState);
}

delegate_compositor!(LumoDesktop);
delegate_output!(LumoDesktop);
delegate_shm!(LumoDesktop);
delegate_layer!(LumoDesktop);
delegate_seat!(LumoDesktop);
delegate_pointer!(LumoDesktop);
delegate_registry!(LumoDesktop);

fn main() {
    let _ = font_system();
    let _ = swash_cache();

    let conn = Connection::connect_to_env().expect("conectar wayland");
    let (globals, mut queue) =
        registry_queue_init::<LumoDesktop>(&conn).expect("registry init");
    let qh = queue.handle();

    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor nao disponivel");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("wlr_layer_shell nao disponivel");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm nao disponivel");

    let surface = compositor.create_surface(&qh);
    // Background layer = atras de tudo. Namespace 'lumo-desktop' pra log
    // no compositor (state.rs trace 'namespace layer encontrado').
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface,
        Layer::Background,
        Some("lumo-desktop"),
        None,
    );
    // Full-screen via Anchor 4 lados + size (0,0) auto (compositor preenche).
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_size(0, 0);
    // exclusive_zone -1: NAO reserva area E NAO eh afetado por outros
    // layers (bar exclusive_zone fica intocada).
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.commit();

    // Pool dimensionado pra full-screen 1920x1080 Argb8888 + double buffer.
    let pool = SlotPool::new(OUTPUT_W as usize * OUTPUT_H as usize * 4 * 2, &shm)
        .expect("SlotPool init");

    let mut state = LumoDesktop {
        registry: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        seat_state: SeatState::new(&globals, &qh),
        layer,
        pool,
        width: OUTPUT_W,
        height: OUTPUT_H,
        running: true,
        first_configured: false,
        pointer: None,
        pointer_pos: None,
        menu: MenuActive { visible: false, x: 0.0, y: 0.0, hover_row: -1 },
        ipc_stream: connect_ipc(),
        last_click_at: None,
    };

    eprintln!("[lumo-desktop] A21: layer-shell Background + menu contextual + CloseDropdowns IPC");

    // Loop principal: padrao DEPS.md A20.9 (prepare_read + poll + dispatch).
    while state.running {
        conn.flush().ok();
        if let Some(guard) = queue.prepare_read() {
            use std::os::fd::AsFd;
            let fd = conn.as_fd();
            let mut pfd = [nix::poll::PollFd::new(fd, nix::poll::PollFlags::POLLIN)];
            let _ = nix::poll::poll(&mut pfd, nix::poll::PollTimeout::try_from(50i32).unwrap());
            let _ = guard.read();
        }
        if let Err(e) = queue.dispatch_pending(&mut state) {
            let msg = format!("{e:?}");
            if msg.contains("ConnectionReset")
                || msg.contains("BrokenPipe")
                || msg.contains("InvalidObject")
            {
                eprintln!("[lumo-desktop] compositor desconectou ({e:?}), saindo");
                break;
            }
            eprintln!("[lumo-desktop] dispatch_pending warn: {e:?}");
        }
        if conn.flush().is_err() {
            eprintln!("[lumo-desktop] flush falhou, saindo");
            break;
        }
    }
}
