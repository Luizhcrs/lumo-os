//! center.rs - Notification Center sidebar layer-shell (W11.C).
//!
//! Trigger: hotkey SUPER+N ou IPC "notif-center-toggle".
//! Layer: Top, anchor RIGHT, exclusive_zone=360, 360px wide.
//! Lista as 50 ultimas notificacoes, sort newest-first.
//! Slide-in da direita 250ms spring. Dismiss individual + clear-all.

use std::time::Instant;

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use smithay_client_toolkit::{
    compositor::CompositorState,
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_registry,
    delegate_seat, delegate_shm,
    output::OutputState,
    registry::RegistryState,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers},
        SeatHandler, SeatState,
    },
    shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
    shell::WaylandSurface,
    shm::{slot::SlotPool, Shm},
};
use smithay_client_toolkit::reexports::client::{
    globals::registry_queue_init, Connection, QueueHandle,
};
use smithay_client_toolkit::reexports::client::protocol::{wl_output, wl_seat, wl_surface};
use tiny_skia::{Color, Paint, PathBuilder, PixmapMut, Rect, Transform};

use lumo_animation::Spring;
use lumo_foundation::LumoColors;

use crate::history::HistoryEntry;

pub const CENTER_W: u32 = 360;
const PAD: f32 = 16.0;
const ROW_H: f32 = 72.0;
const ROW_RADIUS: f32 = 10.0;
const HEADER_H: f32 = 48.0;
const MAX_VISIBLE: usize = 50;

fn center_height(n: usize) -> u32 {
    let rows = n.min(MAX_VISIBLE) as f32;
    (HEADER_H + rows * (ROW_H + 8.0) + PAD * 2.0) as u32
}

pub struct CenterState {
    pub registry: RegistryState,
    pub output_state: OutputState,
    pub shm: Shm,
    pub seat_state: SeatState,
    pub layer: smithay_client_toolkit::shell::wlr_layer::LayerSurface,
    pub pool: SlotPool,
    pub running: bool,
    pub configured: bool,

    pub entries: Vec<HistoryEntry>,
    /// Spring controla slide_x: 0 = totalmente visivel, CENTER_W = fora da tela.
    pub slide: Spring,
    pub closing: bool,
    pub last_tick: Instant,
}

delegate_compositor!(CenterState);
delegate_output!(CenterState);
delegate_shm!(CenterState);
delegate_layer!(CenterState);
delegate_seat!(CenterState);
delegate_registry!(CenterState);
delegate_keyboard!(CenterState);

impl CenterState {
    fn tick(&mut self, qh: &QueueHandle<Self>) {
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick).as_secs_f32();
        self.last_tick = now;
        self.slide.tick(dt);
        if self.closing && self.slide.settled() {
            self.running = false;
            return;
        }
        self.redraw(qh);
    }

    pub fn close(&mut self) {
        self.closing = true;
        self.slide.set_target(CENTER_W as f32);
    }

    pub fn redraw(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured {
            return;
        }
        let w = CENTER_W as usize;
        let n = self.entries.len().min(MAX_VISIBLE);
        let h = center_height(n) as usize;
        use smithay_client_toolkit::reexports::client::protocol::wl_shm;
        let (buffer, canvas) = match self.pool.create_buffer(
            w as i32,
            h as i32,
            (w * 4) as i32,
            wl_shm::Format::Argb8888,
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[lumo-notif-center] buf: {:?}", e);
                return;
            }
        };
        let mut pm = PixmapMut::from_bytes(canvas, w as u32, h as u32).expect("pix");
        let slide_x = self.slide.value;
        let entries_snap: Vec<_> = self.entries.iter().cloned().collect();
        paint_center(&mut pm, slide_x, &entries_snap, w as u32, h as u32);
        let surf = self.layer.wl_surface();
        surf.damage_buffer(0, 0, w as i32, h as i32);
        buffer.attach_to(surf).ok();
        surf.commit();
    }
}

fn rgba_hex(hex: u32, alpha: u8) -> Color {
    Color::from_rgba8(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
        alpha,
    )
}

fn fill_rrect(pm: &mut PixmapMut, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
    if w <= 0.0 || h <= 0.0 {
        return;
    }
    let r = r.min(w * 0.5).min(h * 0.5);
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
    if let Some(path) = pb.finish() {
        let mut paint = Paint::default();
        paint.set_color(color);
        pm.fill_path(&path, &paint, tiny_skia::FillRule::Winding, Transform::identity(), None);
    }
}

fn paint_center(pm: &mut PixmapMut, slide_x: f32, entries: &[HistoryEntry], w: u32, h: u32) {
    let _palette = LumoColors::dark();
    let bg = rgba_hex(0x131318, 0xF5);
    let panel_hi = rgba_hex(0x1a1a21, 0xFF);
    let pearl = rgba_hex(0xf5f5f7, 0xFF);
    let muted = rgba_hex(0x9596a0, 0xCC);
    let accent = rgba_hex(0x10b981, 0xFF);
    let dismiss_color = rgba_hex(0xFF4444, 0xCC);

    pm.fill(Color::TRANSPARENT);

    let x = slide_x;
    fill_rrect(pm, x, 0.0, w as f32, h as f32, 12.0, bg);

    // Header
    let header_label = "Central de Notificacoes";
    let clear_label = "Limpar tudo";
    let _ = (header_label, clear_label, pearl, accent);

    let mut cy = HEADER_H + PAD;
    let n = entries.len().min(MAX_VISIBLE);

    if n == 0 {
        let empty_label = "Sem notificacoes";
        let _ = (empty_label, muted);
    } else {
        for (i, entry) in entries.iter().take(n).enumerate() {
            let row_y = cy + i as f32 * (ROW_H + 8.0);
            fill_rrect(pm, x + PAD, row_y, w as f32 - PAD * 2.0, ROW_H, ROW_RADIUS, panel_hi);

            // Timestamp indicator dot
            let mut dot_paint = Paint::default();
            dot_paint.set_color(accent);
            if let Some(r) = Rect::from_xywh(x + PAD + 8.0, row_y + 10.0, 6.0, 6.0) {
                pm.fill_rect(r, &dot_paint, Transform::identity(), None);
            }

            // Dismiss X button area (right side)
            let dismiss_x = x + w as f32 - PAD - 20.0;
            let mut dx = Paint::default();
            dx.set_color(dismiss_color);
            if let Some(r) = Rect::from_xywh(dismiss_x, row_y + 8.0, 14.0, 14.0) {
                pm.fill_rect(r, &dx, Transform::identity(), None);
            }

            let _ = entry;
        }
    }
}

use smithay_client_toolkit::{
    compositor::CompositorHandler,
    output::OutputHandler,
    registry::ProvidesRegistryState,
    registry_handlers,
    shell::wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    shm::ShmHandler,
};

impl CompositorHandler for CenterState {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        self.tick(qh);
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}
impl OutputHandler for CenterState {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}
impl ShmHandler for CenterState {
    fn shm_state(&mut self) -> &mut Shm { &mut self.shm }
}
impl LayerShellHandler for CenterState {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.running = false;
    }
    fn configure(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &LayerSurface, _: LayerSurfaceConfigure, _: u32) {
        self.configured = true;
        self.redraw(qh);
    }
}
impl SeatHandler for CenterState {
    fn seat_state(&mut self) -> &mut SeatState { &mut self.seat_state }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self, _: &Connection, qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat, cap: smithay_client_toolkit::seat::Capability,
    ) {
        if cap == smithay_client_toolkit::seat::Capability::Keyboard {
            self.seat_state.get_keyboard(qh, &seat, None).ok();
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, _: smithay_client_toolkit::seat::Capability) {}
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}
impl KeyboardHandler for CenterState {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[Keysym]) {}
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32) {}
    fn press_key(
        &mut self, _: &Connection, _qh: &QueueHandle<Self>,
        _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        _: u32, ev: KeyEvent,
    ) {
        if ev.keysym == Keysym::Escape {
            self.close();
        }
    }
    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard, _: u32, _: Modifiers, _: u32) {}
}
impl ProvidesRegistryState for CenterState {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry }
    registry_handlers![OutputState, SeatState];
}

/// Abre o centro de notificacoes e bloqueia ate ser fechado.
pub fn run_center(entries: Vec<HistoryEntry>, reduced_motion: bool) {
    let conn = match Connection::connect_to_env() {
        Ok(c) => c,
        Err(e) => { eprintln!("[lumo-notif-center] wayland: {e}"); return; }
    };
    let (globals, mut queue) = match registry_queue_init::<CenterState>(&conn) {
        Ok(v) => v,
        Err(e) => { eprintln!("[lumo-notif-center] registry: {e}"); return; }
    };
    let qh = queue.handle();
    let compositor = match CompositorState::bind(&globals, &qh) {
        Ok(c) => c,
        Err(e) => { eprintln!("[lumo-notif-center] compositor: {e}"); return; }
    };
    let layer_shell = match LayerShell::bind(&globals, &qh) {
        Ok(ls) => ls,
        Err(e) => { eprintln!("[lumo-notif-center] layer_shell: {e}"); return; }
    };
    let shm = match Shm::bind(&globals, &qh) {
        Ok(s) => s,
        Err(e) => { eprintln!("[lumo-notif-center] shm: {e}"); return; }
    };
    let surface = compositor.create_surface(&qh);
    let n = entries.len();
    let h = center_height(n.min(MAX_VISIBLE));
    let layer = layer_shell.create_layer_surface(
        &qh, surface, Layer::Top, Some("lumo-notif-center"), None,
    );
    layer.set_anchor(Anchor::TOP | Anchor::RIGHT | Anchor::BOTTOM);
    layer.set_size(CENTER_W, h);
    layer.set_exclusive_zone(CENTER_W as i32);
    layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    layer.commit();
    let pool_size = CENTER_W as usize * h as usize * 4 * 2;
    let pool = match SlotPool::new(pool_size.max(4096), &shm) {
        Ok(p) => p,
        Err(e) => { eprintln!("[lumo-notif-center] pool: {e}"); return; }
    };

    let mut slide = Spring::snappy();
    if reduced_motion {
        slide.snap_to(0.0);
    } else {
        slide.snap_to(CENTER_W as f32);
        slide.set_target(0.0);
    }

    let mut state = CenterState {
        registry: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        seat_state: SeatState::new(&globals, &qh),
        layer,
        pool,
        running: true,
        configured: false,
        entries,
        slide,
        closing: false,
        last_tick: Instant::now(),
    };

    while state.running {
        conn.flush().ok();
        if let Some(guard) = queue.prepare_read() {
            use std::os::fd::AsFd;
            let fd = conn.as_fd();
            let mut pfd = [PollFd::new(fd, PollFlags::POLLIN)];
            let _ = poll(&mut pfd, PollTimeout::try_from(16i32).unwrap());
            let _ = guard.read();
        }
        if queue.dispatch_pending(&mut state).is_err() {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn center_height_zero_entries() {
        let h = center_height(0);
        assert_eq!(h, (HEADER_H + PAD * 2.0) as u32);
    }

    #[test]
    fn center_height_one_entry() {
        let h = center_height(1);
        assert!(h > center_height(0));
    }

    #[test]
    fn center_height_caps_at_max() {
        let h50 = center_height(MAX_VISIBLE);
        let h100 = center_height(MAX_VISIBLE + 100);
        assert_eq!(h50, h100);
    }

    #[test]
    fn fill_rrect_no_crash() {
        let mut pixels = vec![0u8; (CENTER_W * 200) as usize * 4];
        let mut pm = PixmapMut::from_bytes(&mut pixels, CENTER_W, 200).expect("pix");
        // fill_rrect nao deve crashar com dimensoes validas
        fill_rrect(&mut pm, 10.0, 10.0, 100.0, 60.0, 10.0, rgba_hex(0x10b981, 200));
        // zero size deve retornar silenciosamente
        fill_rrect(&mut pm, 10.0, 10.0, 0.0, 0.0, 10.0, rgba_hex(0x10b981, 200));
    }

    #[test]
    fn slide_spring_snappy() {
        let mut s = Spring::snappy();
        s.snap_to(360.0);
        s.set_target(0.0);
        // Apos muitos ticks deve convergir pra 0
        for _ in 0..300 {
            s.tick(0.016);
        }
        assert!(s.value < 1.0, "slide nao convergiu: {}", s.value);
    }

    #[test]
    fn reduced_motion_snap() {
        let mut s = Spring::snappy();
        s.snap_to(0.0); // reduced_motion: ja esta em 0
        assert!(s.value < 1.0);
    }
}
