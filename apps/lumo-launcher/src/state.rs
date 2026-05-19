//! state.rs - LumoLauncher struct + Wayland handler traits.

use smithay_client_toolkit::{
    compositor::CompositorHandler,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyboardHandler, KeyEvent, Keysym, Modifiers},
        Capability, SeatHandler, SeatState,
    },
    shell::wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    shell::WaylandSurface,
    shm::{Shm, ShmHandler},
};
use smithay_client_toolkit::reexports::client::{
    globals::GlobalList,
    protocol::{wl_output, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};
use smithay_client_toolkit::shm::slot::SlotPool;
use crate::desktop::DesktopEntry;
use crate::fuzzy;
use crate::math;
use crate::paint;
use crate::recent::RecentApps;

pub struct LumoLauncher {
    pub registry: RegistryState,
    pub output_state: OutputState,
    pub shm: Shm,
    pub seat_state: SeatState,
    pub layer: LayerSurface,
    pub pool: SlotPool,
    pub running: bool,
    pub configured: bool,
    pub width: u32,
    pub height: u32,
    pub needs_redraw: bool,
    pub entries: Vec<DesktopEntry>,
    pub recent: RecentApps,
    pub query: String,
    pub results: Vec<DesktopEntry>,
    pub selected_idx: usize,
    pub math_result: Option<String>,
    pub result_rects: Vec<(f32, f32)>,
    pub keyboard: Option<smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard>,
}

impl LumoLauncher {
    pub fn new(globals: GlobalList, qh: QueueHandle<Self>, shm: Shm, pool: SlotPool, layer: LayerSurface, entries: Vec<DesktopEntry>, recent: RecentApps) -> Self {
        Self {
            registry: RegistryState::new(&globals),
            output_state: OutputState::new(&globals, &qh),
            shm, seat_state: SeatState::new(&globals, &qh),
            layer, pool, running: true, configured: false,
            width: crate::SCREEN_W, height: crate::SCREEN_H,
            needs_redraw: false, entries, recent,
            query: String::new(), results: Vec::new(), selected_idx: 0,
            math_result: None, result_rects: Vec::new(), keyboard: None,
        }
    }
    pub fn update_results(&mut self) {
        if self.query.is_empty() {
            self.results = self.recent.entries.iter()
                .filter_map(|name| self.entries.iter().find(|e| &e.name == name).cloned())
                .take(crate::MAX_RESULTS).collect();
            self.math_result = None;
        } else {
            self.math_result = math::try_eval(&self.query);
            self.results = fuzzy::search(&self.query, &self.entries).into_iter().map(|r| r.entry).collect();
        }
        self.selected_idx = 0;
    }
    pub fn launch_selected(&mut self) {
        if let Some(entry) = self.results.get(self.selected_idx) {
            let cmd = entry.clean_exec();
            self.recent.push(&entry.name);
            spawn_app(&cmd);
        }
        self.running = false;
    }
    pub fn redraw(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured { return; }
        let w = self.width as usize; let h = self.height as usize;
        let (buffer, canvas) = match self.pool.create_buffer(w as i32, h as i32, (w*4) as i32, wl_shm::Format::Argb8888) {
            Ok(v) => v, Err(e) => { eprintln!("[lumo-launcher] buf: {:?}", e); return; }
        };
        let mut pm = tiny_skia::PixmapMut::from_bytes(canvas, w as u32, h as u32).expect("pix");
        let inp = paint::PaintInput {
            query: &self.query, results: &self.results, selected_idx: self.selected_idx,
            math_result: self.math_result.as_deref(), width: self.width, height: self.height,
        };
        self.result_rects = paint::paint_launcher(&mut pm, &inp);
        let surf = self.layer.wl_surface();
        surf.damage_buffer(0, 0, w as i32, h as i32);
        buffer.attach_to(surf).ok();
        surf.commit();
    }
}

fn spawn_app(cmd: &str) {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let Some(prog) = parts.first() else { return };
    match std::process::Command::new(prog).args(&parts[1..]).spawn() {
        Ok(child) => eprintln!("[lumo-launcher] spawn {prog} pid={}", child.id()),
        Err(e) => eprintln!("[lumo-launcher] spawn {prog} falhou: {e}"),
    }
}

impl CompositorHandler for LumoLauncher {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) { self.redraw(qh); }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}
impl OutputHandler for LumoLauncher {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}
impl ShmHandler for LumoLauncher { fn shm_state(&mut self) -> &mut Shm { &mut self.shm } }
impl LayerShellHandler for LumoLauncher {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) { self.running = false; }
    fn configure(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &LayerSurface, c: LayerSurfaceConfigure, _: u32) {
        if c.new_size.0 != 0 { self.width = c.new_size.0; }
        if c.new_size.1 != 0 { self.height = c.new_size.1; }
        self.configured = true; self.update_results(); self.redraw(qh);
    }
}
impl SeatHandler for LumoLauncher {
    fn seat_state(&mut self) -> &mut SeatState { &mut self.seat_state }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(&mut self, _: &Connection, qh: &QueueHandle<Self>, seat: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Keyboard && self.keyboard.is_none() {
            self.keyboard = Some(self.seat_state.get_keyboard(qh, &seat, None).expect("kb"));
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, cap: Capability) {
        if cap == Capability::Keyboard { self.keyboard = None; }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}
impl KeyboardHandler for LumoLauncher {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[Keysym]) {}
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32) {}
    fn press_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard, _: u32, event: KeyEvent) {
        match event.keysym {
            Keysym::Escape => { self.running = false; }
            Keysym::Return | Keysym::KP_Enter => { self.launch_selected(); }
            Keysym::Up => { if self.selected_idx > 0 { self.selected_idx -= 1; self.needs_redraw = true; } }
            Keysym::Down => { if self.selected_idx + 1 < self.results.len() { self.selected_idx += 1; self.needs_redraw = true; } }
            Keysym::BackSpace => { self.query.pop(); self.update_results(); self.needs_redraw = true; }
            sym => {
                if let Some(ch) = keysym_to_char(sym) { self.query.push(ch); self.update_results(); self.needs_redraw = true; }
            }
        }
    }
    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard, _: u32, _: Modifiers, _: u32) {}
}
impl ProvidesRegistryState for LumoLauncher {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry }
    registry_handlers![OutputState, SeatState];
}

fn keysym_to_char(sym: Keysym) -> Option<char> {
    let raw = sym.raw();
    if raw >= 0x20 && raw <= 0x7E { char::from_u32(raw) } else { None }
}
