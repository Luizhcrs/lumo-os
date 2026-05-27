//! lumo-launcher - overlay fullscreen fuzzy XDG app launcher.

mod desktop;
mod fuzzy;
mod math;
mod paint;
mod recent;
mod state;

use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use smithay_client_toolkit::reexports::client::{globals::registry_queue_init, Connection};
use smithay_client_toolkit::{
    compositor::CompositorState,
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_registry,
    delegate_seat, delegate_shm,
    output::OutputState,
    registry::RegistryState,
    seat::SeatState,
    shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
    shell::WaylandSurface,
    shm::{slot::SlotPool, Shm},
};
use state::LumoLauncher;
use std::time::Instant;

delegate_compositor!(LumoLauncher);
delegate_output!(LumoLauncher);
delegate_shm!(LumoLauncher);
delegate_layer!(LumoLauncher);
delegate_seat!(LumoLauncher);
delegate_keyboard!(LumoLauncher);
delegate_registry!(LumoLauncher);

pub const SCREEN_W: u32 = 1920;
pub const SCREEN_H: u32 = 1080;
pub const PANEL_W: f32 = 640.0;
pub const PANEL_H_BASE: f32 = 72.0;
pub const ROW_H: f32 = 48.0;
pub const MAX_RESULTS: usize = 8;
pub const PANEL_RADIUS: f32 = 16.0;
pub const SEARCH_BOX_H: f32 = 48.0;

fn main() {
    lumo_error::hook::install_panic_hook("lumo-launcher", lumo_error::Domain::App);
    let entries = desktop::load_desktop_entries();
    let recent = recent::RecentApps::load();
    let conn = Connection::connect_to_env().expect("wayland connect");
    let (globals, mut queue) = registry_queue_init::<LumoLauncher>(&conn).expect("registry init");
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("wlr_layer_shell");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm");
    let surface = compositor.create_surface(&qh);
    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, Some("lumo-launcher"), None);
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_size(SCREEN_W, SCREEN_H);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    layer.commit();
    let pool = SlotPool::new(SCREEN_W as usize * SCREEN_H as usize * 4, &shm).expect("SlotPool");
    let mut state = LumoLauncher::new(globals, qh.clone(), shm, pool, layer, entries, recent);
    let mut last_redraw = Instant::now();
    while state.running {
        if state.needs_redraw && last_redraw.elapsed().as_millis() >= 8 {
            last_redraw = Instant::now();
            state.needs_redraw = false;
            state.redraw(&qh);
        }
        conn.flush().ok();
        if let Some(guard) = queue.prepare_read() {
            use std::os::fd::AsFd;
            let fd = conn.as_fd();
            let mut pfd = [PollFd::new(fd, PollFlags::POLLIN)];
            let _ = poll(&mut pfd, PollTimeout::try_from(16i32).unwrap());
            let _ = guard.read();
        }
        if let Err(e) = queue.dispatch_pending(&mut state) {
            let s = format!("{e:?}");
            if s.contains("ConnectionReset") || s.contains("BrokenPipe") {
                break;
            }
        }
        if conn.flush().is_err() {
            break;
        }
    }
}
