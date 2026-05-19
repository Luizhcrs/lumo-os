//! lumo-lock W10.A fullscreen lock screen via wlr-layer-shell Overlay.
//!
//! Layer: Overlay, KeyboardInteractivity=Exclusive, anchor=ALL, fullscreen.
//! Covers all outputs. Clock + date secondary. Masked password input.
//! Auth: PAM crate (feature pam-auth) or su fallback.
//! Esc is intentionally ignored. Enter triggers auth. Fail: shake animation.
//! Success: process exits 0.

use std::time::{Duration, Instant};

use smithay_client_toolkit::{
    compositor::CompositorState,
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{Capability, SeatHandler, SeatState},
    shell::wlr_layer::{
        Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
        LayerSurfaceConfigure,
    },
    shell::WaylandSurface,
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use smithay_client_toolkit::reexports::client::{
    globals::registry_queue_init,
    protocol::{wl_output, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};
use tiny_skia::PixmapMut;

mod auth;
mod render;
#[cfg(test)]
mod tests;
pub use render::paint_lock;

fn main() {
    let conn = Connection::connect_to_env().expect("lumo-lock: no Wayland display");
    let (globals, mut queue) = registry_queue_init::<LumoLock>(&conn).expect("registry");
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("zwlr_layer_shell_v1");
    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, Some("lumo-lock"), None);
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    layer.set_size(0, 0);
    layer.commit();
    let pool = SlotPool::new(1920 * 1080 * 4, &shm).expect("shm pool");
    let mut state = LumoLock {
        registry: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        compositor_state: compositor,
        seat_state: SeatState::new(&globals, &qh),
        keyboard: None,
        layer,
        pool,
        width: 1920,
        height: 1080,
        running: true,
        first_configured: false,
        password: String::new(),
        shake_count: 0,
        shake_start: None,
        last_fail_msg: String::new(),
        auth_pending: false,
    };
    loop {
        queue.blocking_dispatch(&mut state).expect("dispatch");
        if !state.running { break; }
    }
}

pub struct LumoLock {
    pub registry: RegistryState,
    pub output_state: OutputState,
    pub shm: Shm,
    pub compositor_state: CompositorState,
    pub seat_state: SeatState,
    pub keyboard: Option<smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard>,
    pub layer: LayerSurface,
    pub pool: SlotPool,
    pub width: u32,
    pub height: u32,
    pub running: bool,
    pub first_configured: bool,
    pub password: String,
    pub shake_count: u32,
    pub shake_start: Option<Instant>,
    pub last_fail_msg: String,
    pub auth_pending: bool,
}

impl LumoLock {
    pub fn redraw(&mut self, qh: &QueueHandle<Self>) {
        if self.width == 0 || self.height == 0 { return; }
        let stride = self.width * 4;
        let shake_offset = self.shake_offset();
        let (buffer, canvas) = self.pool
            .create_buffer(self.width as i32, self.height as i32, stride as i32, wl_shm::Format::Argb8888)
            .expect("create buffer");
        let mut pixmap = PixmapMut::from_bytes(canvas, self.width, self.height).expect("pixmap");
        paint_lock(&mut pixmap, self.width, self.height, &self.password, &self.last_fail_msg, shake_offset);
        self.layer.wl_surface().attach(Some(buffer.wl_buffer()), 0, 0);
        self.layer.wl_surface().damage_buffer(0, 0, self.width as i32, self.height as i32);
        self.layer.wl_surface().commit();
    }

    pub fn shake_offset(&self) -> f32 {
        let Some(start) = self.shake_start else { return 0.0; };
        let elapsed = start.elapsed().as_secs_f32();
        if elapsed > 0.5 { return 0.0; }
        8.0 * (-10.0 * elapsed).exp() * (40.0 * elapsed * std::f32::consts::TAU).sin()
    }

    pub fn submit_password(&mut self, qh: &QueueHandle<Self>) {
        if self.auth_pending { return; }
        let pwd = std::mem::take(&mut self.password);
        match auth::authenticate(&pwd) {
            Ok(()) => { self.running = false; }
            Err(msg) => {
                self.shake_count += 1;
                self.shake_start = Some(Instant::now());
                self.last_fail_msg = msg;
                self.redraw(qh);
            }
        }
    }
}

impl ProvidesRegistryState for LumoLock {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry }
    registry_handlers![OutputState, SeatState];
}
impl ShmHandler for LumoLock {
    fn shm_state(&mut self) -> &mut Shm { &mut self.shm }
}
impl smithay_client_toolkit::compositor::CompositorHandler for LumoLock {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        if self.shake_start.map(|s| s.elapsed() < Duration::from_millis(500)).unwrap_or(false) {
            self.redraw(qh);
        }
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}
impl OutputHandler for LumoLock {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}
impl LayerShellHandler for LumoLock {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) { self.running = false; }
    fn configure(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &LayerSurface, configure: LayerSurfaceConfigure, _: u32) {
        let (w, h) = configure.new_size;
        if w > 0 { self.width = w; }
        if h > 0 { self.height = h; }
        if !self.first_configured {
            self.first_configured = true;
            let needed = (self.width * self.height * 4) as usize;
            if needed > self.pool.len() { self.pool.resize(needed).ok(); }
        }
        self.redraw(qh);
    }
}
impl SeatHandler for LumoLock {
    fn seat_state(&mut self) -> &mut SeatState { &mut self.seat_state }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, capability: Capability) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            match self.seat_state.get_keyboard::<_, LumoLock>(qh, &seat, None) {
                Ok(kb) => self.keyboard = Some(kb),
                Err(e) => eprintln!("[lumo-lock] keyboard init failed: {e:?}"),
            }
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, capability: Capability) {
        if capability == Capability::Keyboard {
            if let Some(kb) = self.keyboard.take() { kb.release(); }
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}
type WlKeyboard = smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard;
type WlPointer = smithay_client_toolkit::reexports::client::protocol::wl_pointer::WlPointer;
impl smithay_client_toolkit::seat::keyboard::KeyboardHandler for LumoLock {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, _: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[smithay_client_toolkit::seat::keyboard::Keysym]) {}
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, _: &wl_surface::WlSurface, _: u32) {}
    fn press_key(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &WlKeyboard, _: u32, event: smithay_client_toolkit::seat::keyboard::KeyEvent) {
        use smithay_client_toolkit::seat::keyboard::Keysym;
        match event.keysym {
            Keysym::Return | Keysym::KP_Enter => self.submit_password(qh),
            Keysym::Escape => {}
            Keysym::BackSpace => { self.password.pop(); self.redraw(qh); }
            _ => {
                if let Some(s) = event.utf8 {
                    for ch in s.chars() { if !ch.is_control() { self.password.push(ch); } }
                    self.redraw(qh);
                }
            }
        }
    }
    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, _: u32, _: smithay_client_toolkit::seat::keyboard::KeyEvent) {}
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, _: u32, _: smithay_client_toolkit::seat::keyboard::Modifiers, _: u32) {}
    fn update_keymap(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, _: smithay_client_toolkit::seat::keyboard::Keymap<'_>) {}
    fn update_repeat_info(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlKeyboard, _: smithay_client_toolkit::seat::keyboard::RepeatInfo) {}
}
impl smithay_client_toolkit::seat::pointer::PointerHandler for LumoLock {
    fn pointer_frame(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &WlPointer, _: &[smithay_client_toolkit::seat::pointer::PointerEvent]) {}
}
delegate_compositor!(LumoLock);
delegate_output!(LumoLock);
delegate_shm!(LumoLock);
delegate_layer!(LumoLock);
delegate_seat!(LumoLock);
delegate_keyboard!(LumoLock);
delegate_pointer!(LumoLock);
delegate_registry!(LumoLock);
