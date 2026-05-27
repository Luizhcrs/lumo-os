//! osd.rs - lumo-osd: OSD overlay visual pra Caps Lock, Volume, Brightness.
use crate::bar::fonts::{draw_text, font_system, rgba_hex, swash_cache};
use crate::bar::icons::{fill_rrect, stroke_rrect};
use lumo_animation::{AnimCurve, LAAnimator, LACurve};
use lumo_ipc::{default_socket_path, LumoEvent, OsdIcon};
use smithay_client_toolkit::reexports::client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_shm, wl_surface},
    Connection, QueueHandle,
};
use smithay_client_toolkit::{
    compositor::CompositorState,
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::OutputState,
    registry::RegistryState,
    shell::wlr_layer::{
        Anchor, KeyboardInteractivity, Layer, LayerShell, LayerSurface, LayerSurfaceConfigure,
    },
    shell::WaylandSurface,
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use std::io::{ErrorKind, Read};
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};
const OSD_W: u32 = 280;
const OSD_H: u32 = 56;
const OSD_RADIUS: f32 = 14.0;
const OSD_MARGIN_TOP: i32 = 80;
const OSD_BG_HEX: u32 = 0x1A1A1C;
const OSD_BG_ALPHA: u8 = 0xCC;
const OSD_TEXT_HEX: u32 = 0xF5F5F7;
const OSD_ICON_SIZE: f32 = 22.0;
const OSD_ICON_X: f32 = 16.0;
const OSD_TEXT_SIZE: f32 = 14.0;
const FADE_IN_DUR: f32 = 0.15;
const HOLD_DUR: f32 = 1.70;
const FADE_OUT_DUR: f32 = 0.15;
const TOTAL_DUR: f32 = FADE_IN_DUR + HOLD_DUR + FADE_OUT_DUR;
#[derive(Debug, Clone)]
struct OsdRequest {
    text: String,
    icon: OsdIcon,
}
struct LumoOsd {
    registry: RegistryState,
    output_state: OutputState,
    shm: Shm,
    layer: LayerSurface,
    pool: SlotPool,
    width: u32,
    height: u32,
    running: bool,
    first_configured: bool,
    ipc_stream: Option<UnixStream>,
    ipc_rx_buf: Vec<u8>,
    current: Option<OsdRequest>,
    phase_elapsed: f32,
    alpha_anim: LAAnimator<f32>,
    visible: bool,
    last_frame: Instant,
}
impl LumoOsd {
    fn redraw(&mut self, qh: &QueueHandle<LumoOsd>) {
        let (buffer, canvas) = match self.pool.create_buffer(
            self.width as i32,
            self.height as i32,
            self.width as i32 * 4,
            wl_shm::Format::Argb8888,
        ) {
            Ok(x) => x,
            Err(e) => {
                eprintln!("[lumo-osd] create_buffer err: {e}");
                return;
            }
        };
        let mut pixmap =
            tiny_skia::PixmapMut::from_bytes(canvas, self.width, self.height).expect("PixmapMut");
        pixmap.fill(tiny_skia::Color::TRANSPARENT);
        if let Some(req) = self.current.clone() {
            let alpha_f = self.alpha_anim.tick(0.0).clamp(0.0, 1.0);
            let alpha = (alpha_f * 255.0).round() as u8;
            if alpha > 0 {
                draw_osd_frame(&mut pixmap, &req, alpha);
            }
        }
        self.layer
            .wl_surface()
            .attach(Some(buffer.wl_buffer()), 0, 0);
        self.layer
            .wl_surface()
            .damage_buffer(0, 0, self.width as i32, self.height as i32);
        self.layer.wl_surface().commit();
    }
    fn show_osd(&mut self, text: String, icon: OsdIcon) {
        self.current = Some(OsdRequest { text, icon });
        self.phase_elapsed = 0.0;
        let ca = self.alpha_anim.tick(0.0);
        self.alpha_anim = LAAnimator::new(
            ca,
            1.0,
            AnimCurve::Bezier {
                curve: LACurve::ease_out_cubic(),
                duration: FADE_IN_DUR,
            },
        );
        self.visible = true;
        self.last_frame = Instant::now();
    }
    fn tick(&mut self, dt: f32, qh: &QueueHandle<LumoOsd>) {
        if self.current.is_none() {
            return;
        }
        self.phase_elapsed += dt;
        if self.phase_elapsed <= FADE_IN_DUR {
            let _ = self.alpha_anim.tick(dt);
        } else if self.phase_elapsed <= FADE_IN_DUR + HOLD_DUR {
            if !self.alpha_anim.is_done() {
                let _ = self.alpha_anim.tick(dt);
            }
        } else {
            let lt = self.phase_elapsed - (FADE_IN_DUR + HOLD_DUR);
            if lt <= dt * 2.0 {
                self.alpha_anim = LAAnimator::new(
                    1.0,
                    0.0,
                    AnimCurve::Bezier {
                        curve: LACurve::ease_out_cubic(),
                        duration: FADE_OUT_DUR,
                    },
                );
            }
            let _ = self.alpha_anim.tick(dt);
        }
        if self.phase_elapsed > TOTAL_DUR {
            self.current = None;
            self.visible = false;
            self.redraw(qh);
            return;
        }
        self.redraw(qh);
    }
}
fn draw_osd_frame(pixmap: &mut tiny_skia::PixmapMut, req: &OsdRequest, alpha: u8) {
    let w = pixmap.width() as f32;
    let h = pixmap.height() as f32;
    let bga = ((OSD_BG_ALPHA as u32 * alpha as u32) / 255) as u8;
    fill_rrect(
        pixmap,
        0.0,
        0.0,
        w,
        h,
        OSD_RADIUS,
        rgba_hex(OSD_BG_HEX, bga),
    );
    let ic = rgba_hex(OSD_TEXT_HEX, alpha);
    let cy = h / 2.0;
    match req.icon {
        OsdIcon::Keyboard => draw_keyboard_icon(pixmap, OSD_ICON_X, cy, OSD_ICON_SIZE, ic),
        OsdIcon::Volume => draw_volume_icon(pixmap, OSD_ICON_X, cy, OSD_ICON_SIZE, ic),
        OsdIcon::Brightness => draw_brightness_icon(pixmap, OSD_ICON_X, cy, OSD_ICON_SIZE, ic),
        OsdIcon::None => {}
    }
    draw_text(
        pixmap,
        OSD_ICON_X + OSD_ICON_SIZE + 10.0,
        (h - OSD_TEXT_SIZE * 1.2) / 2.0,
        &req.text,
        OSD_TEXT_SIZE,
        rgba_hex(OSD_TEXT_HEX, alpha),
        false,
    );
}
fn draw_keyboard_icon(
    c: &mut tiny_skia::PixmapMut,
    cx: f32,
    cy: f32,
    sz: f32,
    col: tiny_skia::Color,
) {
    let y = cy - sz * 0.4;
    let w = sz;
    let h = sz * 0.65;
    stroke_rrect(c, cx, y, w, h, 2.5, col, 1.5);
    let kw = sz * 0.22;
    let kh = sz * 0.18;
    let kr = 1.5;
    let ry = y + h * 0.2;
    for i in 0..3i32 {
        fill_rrect(
            c,
            cx + sz * 0.08 + i as f32 * (kw + sz * 0.09),
            ry,
            kw,
            kh,
            kr,
            col,
        );
    }
    let sw = sz * 0.55;
    fill_rrect(
        c,
        cx + (w - sw) / 2.0,
        ry + kh + sz * 0.1,
        sw,
        kh * 0.85,
        kr,
        col,
    );
}
fn draw_volume_icon(
    c: &mut tiny_skia::PixmapMut,
    cx: f32,
    cy: f32,
    sz: f32,
    col: tiny_skia::Color,
) {
    let hh = sz * 0.35;
    fill_rrect(c, cx, cy - hh, sz * 0.4, hh * 2.0, 1.0, col);
    for i in 1..=2u32 {
        let r = sz * 0.25 * i as f32;
        stroke_rrect(
            c,
            cx + sz * 0.35,
            cy - r,
            r * 0.4,
            r * 2.0,
            r * 0.2,
            col,
            1.5,
        );
    }
}
fn draw_brightness_icon(
    c: &mut tiny_skia::PixmapMut,
    cx: f32,
    cy: f32,
    sz: f32,
    col: tiny_skia::Color,
) {
    use crate::bar::icons::fill_circle;
    fill_circle(c, cx + sz / 2.0, cy, sz * 0.22, col);
    let rl = sz * 0.18;
    let rw = sz * 0.07;
    let d = sz * 0.32;
    for (dx, dy) in [(0.0f32, -d), (0.0, d), (-d, 0.0), (d, 0.0)] {
        fill_rrect(
            c,
            cx + sz / 2.0 + dx - rw / 2.0,
            cy + dy - rl / 2.0,
            rw,
            rl,
            1.0,
            col,
        );
    }
}
fn connect_ipc_osd() -> Option<UnixStream> {
    let path = default_socket_path()?;
    match UnixStream::connect(&path) {
        Ok(s) => {
            s.set_nonblocking(true).ok()?;
            eprintln!("[lumo-osd] IPC conectado em {}", path.display());
            Some(s)
        }
        Err(e) => {
            eprintln!("[lumo-osd] IPC nao conectou ({})", e);
            None
        }
    }
}
fn drain_ipc_osd(s: &mut UnixStream, buf: &mut Vec<u8>) -> (bool, Option<(String, OsdIcon)>) {
    let mut tmp = [0u8; 256];
    let mut alive = true;
    let mut pending = None;
    loop {
        match s.read(&mut tmp) {
            Ok(0) => {
                alive = false;
                break;
            }
            Ok(n) => buf.extend_from_slice(&tmp[..n]),
            Err(e) if e.kind() == ErrorKind::WouldBlock => break,
            Err(_) => {
                alive = false;
                break;
            }
        }
    }
    while let Some(nl) = buf.iter().position(|b| *b == b'\n') {
        let line: Vec<u8> = buf.drain(..=nl).collect();
        let len = line.len();
        if let Ok(s2) = std::str::from_utf8(&line[..len.saturating_sub(1)]) {
            if let Ok(ev) = serde_json::from_str::<LumoEvent>(s2.trim()) {
                if let LumoEvent::ShowOsd { text, icon, .. } = ev {
                    pending = Some((text, icon));
                }
            }
        }
    }
    (alive, pending)
}
use smithay_client_toolkit::compositor::CompositorHandler;
use smithay_client_toolkit::output::OutputHandler;
use smithay_client_toolkit::registry::ProvidesRegistryState;
use smithay_client_toolkit::registry_handlers;
use smithay_client_toolkit::shell::wlr_layer::LayerShellHandler;
impl CompositorHandler for LumoOsd {
    fn scale_factor_changed(
        &mut self,
        _c: &Connection,
        _q: &QueueHandle<Self>,
        _s: &wl_surface::WlSurface,
        _f: i32,
    ) {
    }
    fn transform_changed(
        &mut self,
        _c: &Connection,
        _q: &QueueHandle<Self>,
        _s: &wl_surface::WlSurface,
        _t: wl_output::Transform,
    ) {
    }
    fn frame(
        &mut self,
        _c: &Connection,
        _q: &QueueHandle<Self>,
        _s: &wl_surface::WlSurface,
        _t: u32,
    ) {
    }
    fn surface_enter(
        &mut self,
        _c: &Connection,
        _q: &QueueHandle<Self>,
        _s: &wl_surface::WlSurface,
        _o: &wl_output::WlOutput,
    ) {
    }
    fn surface_leave(
        &mut self,
        _c: &Connection,
        _q: &QueueHandle<Self>,
        _s: &wl_surface::WlSurface,
        _o: &wl_output::WlOutput,
    ) {
    }
}
impl OutputHandler for LumoOsd {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _c: &Connection, _q: &QueueHandle<Self>, _o: wl_output::WlOutput) {}
    fn update_output(&mut self, _c: &Connection, _q: &QueueHandle<Self>, _o: wl_output::WlOutput) {}
    fn output_destroyed(
        &mut self,
        _c: &Connection,
        _q: &QueueHandle<Self>,
        _o: wl_output::WlOutput,
    ) {
    }
}
impl ShmHandler for LumoOsd {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}
impl LayerShellHandler for LumoOsd {
    fn closed(&mut self, _c: &Connection, _q: &QueueHandle<Self>, _l: &LayerSurface) {
        self.running = false;
    }
    fn configure(
        &mut self,
        _c: &Connection,
        qh: &QueueHandle<Self>,
        _l: &LayerSurface,
        cfg: LayerSurfaceConfigure,
        _s: u32,
    ) {
        self.width = if cfg.new_size.0 == 0 {
            OSD_W
        } else {
            cfg.new_size.0
        };
        self.height = if cfg.new_size.1 == 0 {
            OSD_H
        } else {
            cfg.new_size.1
        };
        self.first_configured = true;
        self.redraw(qh);
    }
}
impl ProvidesRegistryState for LumoOsd {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers!(OutputState);
}
delegate_compositor!(LumoOsd);
delegate_output!(LumoOsd);
delegate_shm!(LumoOsd);
delegate_layer!(LumoOsd);
delegate_registry!(LumoOsd);
/// Entry point do binario `lumo-osd`.
pub fn run() {
    let _ = font_system();
    let _ = swash_cache();
    let conn = Connection::connect_to_env().expect("[SHELL-INIT-001] conectar wayland");
    let (globals, mut queue) = registry_queue_init::<LumoOsd>(&conn).expect("[SHELL-INIT-002] registry init");
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh).expect("[SHELL-INIT-003] wl_compositor missing");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("[SHELL-INIT-004] wlr_layer_shell missing");
    let shm = Shm::bind(&globals, &qh).expect("[SHELL-INIT-005] wl_shm missing");
    let surface = compositor.create_surface(&qh);
    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, Some("lumo-osd"), None);
    layer.set_anchor(Anchor::TOP);
    layer.set_margin(OSD_MARGIN_TOP, 0, 0, 0);
    layer.set_size(OSD_W, OSD_H);
    layer.set_exclusive_zone(0);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.commit();
    let pool = SlotPool::new((OSD_W * OSD_H * 4 * 2) as usize, &shm).expect("[SHELL-INIT-006] SlotPool alloc");
    let ai = LAAnimator::new(
        0.0f32,
        0.0f32,
        AnimCurve::Bezier {
            curve: LACurve::ease_out_cubic(),
            duration: FADE_IN_DUR,
        },
    );
    let mut state = LumoOsd {
        registry: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        layer,
        pool,
        width: OSD_W,
        height: OSD_H,
        running: true,
        first_configured: false,
        ipc_stream: connect_ipc_osd(),
        ipc_rx_buf: Vec::with_capacity(256),
        current: None,
        phase_elapsed: 0.0,
        alpha_anim: ai,
        visible: false,
        last_frame: Instant::now(),
    };
    let mut last_ipc = Instant::now();
    // T1.10: timeout de inatividade 60s para evitar processo zombie.
    let mut idle_since = Instant::now();
    const INACTIVITY_TIMEOUT: Duration = Duration::from_secs(60);
    while state.running {
        let now = Instant::now();
        let dt = now.duration_since(state.last_frame).as_secs_f32().min(0.05);
        state.last_frame = now;
        if state.current.is_some() {
            state.tick(dt, &qh);
        }
        conn.flush().ok();
        if let Some(guard) = queue.prepare_read() {
            use std::os::fd::AsFd;
            let fd = conn.as_fd();
            let tms: i32 = if state.current.is_some() { 16 } else { 50 };
            let mut pfd = [nix::poll::PollFd::new(fd, nix::poll::PollFlags::POLLIN)];
            let _ = nix::poll::poll(
                &mut pfd,
                nix::poll::PollTimeout::try_from(tms).expect("tms"),
            );
            let _ = guard.read();
        }
        if let Err(e) = queue.dispatch_pending(&mut state) {
            let m = format!("{e:?}");
            if m.contains("ConnectionReset")
                || m.contains("BrokenPipe")
                || m.contains("InvalidObject")
            {
                eprintln!("[lumo-osd] compositor desconectou, saindo");
                break;
            }
        }
        if conn.flush().is_err() {
            break;
        }
        // T1.10: rastreia ultima atividade; exit apos 60s idle.
        if state.current.is_some() || state.ipc_stream.is_some() {
            idle_since = Instant::now();
        }
        if idle_since.elapsed() >= INACTIVITY_TIMEOUT {
            eprintln!("[lumo-osd] inatividade 60s, saindo");
            break;
        }
        if last_ipc.elapsed() >= Duration::from_millis(16) {
            last_ipc = Instant::now();
            if let Some(mut s) = state.ipc_stream.take() {
                let (alive, pending) = drain_ipc_osd(&mut s, &mut state.ipc_rx_buf);
                if alive {
                    state.ipc_stream = Some(s);
                } else {
                    eprintln!("[lumo-osd] IPC peer fechou");
                }
                if let Some((text, icon)) = pending {
                    state.show_osd(text, icon);
                }
            }
        }
    }
}
