//! lumo-desktop - layer-shell Background pra capturar pointer events
//! na area de trabalho (estilo macOS Finder / Windows desktop).
//!
//! A21: novo binario.
//! A25: menu visual alinhado com pill bar.
//! A27: menu redesign Apple-style (hover pill SOLIDO accent + separators
//! entre grupos) + items MVP wallpaper/sobre/atualizar/store. Render
//! compartilhado com lumo-bar via modulo `shell/src/menu.rs`.
//!
//! Comportamento:
//!   - Layer::Background full-screen (1920x1080 Galaxy).
//!   - Surface 100% transparente quando menu fechado.
//!   - Click esquerdo em area vazia: envia LumoCommand::CloseDropdowns
//!     pelo socket IPC -> compositor traduz em LumoEvent::CloseDropdowns
//!     pra bar (A25 frente 2).
//!   - Click direito: abre menu contextual estilo pill bar.
//!
//! Memory feedback_zero_neon_glow: hover pill accent SOLIDO sem glow.
//! Memory feedback_design_lapidado: cada constante com justificativa
//! (ver `menu.rs`).
//! Memory feedback_lumo_arquitetura_clean: render compartilhado em
//! modulo `menu` (Opcao A: arquivo unico em shell/src/menu.rs).

#[path = "../menu.rs"]
mod menu;

use std::io::{ErrorKind, Read, Write};
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

use lumo_foundation::{current_colors, LumoColors};
use lumo_ipc::{default_socket_path, LumoCommand, LumoEvent};

// ============================================================
// Layout constants A27 (menu redesign Apple-style).
// ============================================================

/// Output Galaxy nativo (DEPS.md A19.18 mesmo padrao bar).
const OUTPUT_W: u32 = 1920;
const OUTPUT_H: u32 = 1080;

/// Largura do menu desktop (vem do modulo compartilhado).
const MENU_W: f32 = menu::MENU_W_DESKTOP;
/// Margem entre cursor e canto do menu. 2px = grude no cursor sem encavalar.
const MENU_OFFSET: f32 = 2.0;

/// Items do menu desktop estilo macOS Finder.
///
/// A27: items MVP (futuro: despachar comandos reais wallpaper picker / About
/// dialog / lumo-store launch via IPC).
const MENU_ITEMS: &[menu::MenuItem] = &[
    menu::MenuItem::action("Trocar wallpaper..."),
    menu::MenuItem::action("Sobre este Galaxy Book..."),
    menu::MenuItem::separator(),
    menu::MenuItem::action("Atualizar Lumo..."),
    menu::MenuItem::action("Lumo Store"),
];

// ============================================================
// FontSystem singleton (alinhado com lumo-bar: Geist/JetBrains).
// ============================================================

static FONT_SYSTEM: OnceLock<Mutex<FontSystem>> = OnceLock::new();
static SWASH_CACHE: OnceLock<Mutex<SwashCache>> = OnceLock::new();
static FONT_FAMILY: OnceLock<String> = OnceLock::new();

fn font_system() -> &'static Mutex<FontSystem> {
    FONT_SYSTEM.get_or_init(|| {
        let mut fs = FontSystem::new();
        load_extra_fonts(&mut fs);
        let family = pick_font_family(&fs);
        eprintln!("[lumo-desktop] font_family escolhida = {}", family);
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
    "monospace".to_string()
}

fn current_family() -> &'static str {
    FONT_FAMILY.get().map(|s| s.as_str()).unwrap_or("monospace")
}

// ============================================================
// Color helpers (alinhados com bar).
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
    let family_name = current_family().to_string();
    let attrs = Attrs::new().family(Family::Name(&family_name));
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

/// A26: drena eventos do compositor. Retorna (alive, close_menu_requested).
fn drain_ipc_events(stream: &mut UnixStream, rx_buf: &mut Vec<u8>) -> (bool, bool) {
    let mut tmp = [0u8; 256];
    let mut alive = true;
    let mut close_menu = false;
    loop {
        match stream.read(&mut tmp) {
            Ok(0) => { alive = false; break; }
            Ok(n) => rx_buf.extend_from_slice(&tmp[..n]),
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(_) => { alive = false; break; }
        }
    }
    while let Some(nl) = rx_buf.iter().position(|b| *b == b'\n') {
        let line: Vec<u8> = rx_buf.drain(..=nl).collect();
        if let Ok(s) = std::str::from_utf8(&line[..line.len() - 1]) {
            if let Ok(ev) = serde_json::from_str::<LumoEvent>(s.trim()) {
                if matches!(ev, LumoEvent::CloseDesktopMenu) {
                    close_menu = true;
                }
            }
        }
    }
    (alive, close_menu)
}

// ============================================================
// LumoDesktop state.
// ============================================================

#[derive(Debug, Clone, Copy)]
struct MenuActive {
    visible: bool,
    x: f32,
    y: f32,
    /// Indice do item Action/Toggle em hover. `usize::MAX` quando nenhum
    /// (sentinel; `menu::draw_menu` trata fora-de-range como sem hover).
    hover_idx: usize,
}

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
    ipc_rx_buf: Vec<u8>,
    last_click_at: Option<Instant>,
    palette: LumoColors,
    /// A26: flag setado por drain_ipc_events quando compositor pede pra
    /// fechar menu (mutex bar dropdown vs desktop menu). Loop principal
    /// consome e redesenha.
    need_redraw: bool,
}

fn paint_menu_at(
    canvas: &mut PixmapMut,
    menu_active: MenuActive,
    surf_w: u32,
    surf_h: u32,
    palette: &LumoColors,
) {
    let (mx, my) = menu::clamp_menu_origin(
        MENU_ITEMS,
        menu_active.x,
        menu_active.y,
        MENU_W,
        surf_w,
        surf_h,
        MENU_OFFSET,
    );

    menu::draw_menu(
        canvas,
        mx,
        my,
        MENU_W,
        MENU_ITEMS,
        menu_active.hover_idx,
        palette,
        |c, x, y, w, h, r, color| fill_rrect(c, x, y, w, h, r, color),
        |c, x, y, label, size, color| draw_text(c, x, y, label, size, color),
    );
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

        let menu_snap = self.menu;
        let surf_w = self.width;
        let surf_h = self.height;
        let palette = self.palette;
        if let Some(mut px) = Pixmap::new(self.width, self.height) {
            if menu_snap.visible {
                let mut canvas_mut = px.as_mut();
                paint_menu_at(&mut canvas_mut, menu_snap, surf_w, surf_h, &palette);
            }
            let src = px.data();
            let dst = canvas;
            let n = (self.width * self.height) as usize;
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
        surface.damage_buffer(0, 0, self.width as i32, self.height as i32);
        buffer.attach_to(surface).ok();
        surface.commit();
    }

    /// Hit-test absoluto: retorna Some(idx) se cursor sobre item clicavel.
    /// None = fora do menu OU sobre separator.
    fn hit_test_menu(&self, px: f32, py: f32) -> Option<usize> {
        if !self.menu.visible {
            return None;
        }
        let (mx, my) = menu::clamp_menu_origin(
            MENU_ITEMS,
            self.menu.x,
            self.menu.y,
            MENU_W,
            self.width,
            self.height,
            MENU_OFFSET,
        );
        menu::hit_test(MENU_ITEMS, mx, my, MENU_W, px, py)
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
                        let new_idx = self
                            .hit_test_menu(ev.position.0 as f32, ev.position.1 as f32)
                            .unwrap_or(usize::MAX);
                        if new_idx != self.menu.hover_idx {
                            self.menu.hover_idx = new_idx;
                            need_redraw = true;
                        }
                    }
                }
                PointerEventKind::Leave { .. } => {
                    self.pointer_pos = None;
                    if self.menu.visible && self.menu.hover_idx != usize::MAX {
                        self.menu.hover_idx = usize::MAX;
                        need_redraw = true;
                    }
                }
                PointerEventKind::Press { button, .. } => {
                    let (px, py) = (ev.position.0 as f32, ev.position.1 as f32);
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
                            hover_idx: usize::MAX,
                        };
                        need_redraw = true;
                        eprintln!("[lumo-desktop] right-click ({}, {}) -> menu open", px, py);
                    } else if button == BTN_LEFT {
                        if self.menu.visible {
                            if let Some(idx) = self.hit_test_menu(px, py) {
                                eprintln!(
                                    "[lumo-desktop] menu item: '{}' (stub)",
                                    MENU_ITEMS[idx].label
                                );
                            }
                            self.menu.visible = false;
                            self.menu.hover_idx = usize::MAX;
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
    let layer = layer_shell.create_layer_surface(
        &qh,
        surface,
        Layer::Background,
        Some("lumo-desktop"),
        None,
    );
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_size(0, 0);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.commit();

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
        menu: MenuActive { visible: false, x: 0.0, y: 0.0, hover_idx: usize::MAX },
        ipc_stream: connect_ipc(),
        ipc_rx_buf: Vec::with_capacity(256),
        last_click_at: None,
        palette: current_colors(),
        need_redraw: false,
    };

    eprintln!("[lumo-desktop] A27: menu Apple-style + CloseDropdowns IPC");

    let mut last_ipc_tick = Instant::now();
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

        // A26: drena IPC events do compositor. Tick 8ms = ~120Hz pra reagir
        // imediato (memory feedback_input_feedback_imediato).
        if last_ipc_tick.elapsed() >= Duration::from_millis(8) {
            last_ipc_tick = Instant::now();
            if let Some(mut s) = state.ipc_stream.take() {
                let (alive, close_menu) = drain_ipc_events(&mut s, &mut state.ipc_rx_buf);
                if alive {
                    state.ipc_stream = Some(s);
                } else {
                    eprintln!("[lumo-desktop] IPC peer fechou; desktop continua passivo");
                }
                if close_menu && state.menu.visible {
                    state.menu.visible = false;
                    state.menu.hover_idx = usize::MAX;
                    state.need_redraw = true;
                    eprintln!("[lumo-desktop] CloseDesktopMenu IPC -> menu fechado");
                }
            }
        }
        if state.need_redraw {
            state.need_redraw = false;
            state.redraw(&qh);
        }
    }
}
