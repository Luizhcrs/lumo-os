//! lumo-dock -- dock layer-shell Bottom com magnify spring.

mod config;
mod input;
mod paint;
mod state;

use std::time::{Duration, Instant};
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use smithay_client_toolkit::{
    compositor::CompositorState,
    delegate_compositor, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::OutputState,
    registry::RegistryState,
    seat::SeatState,
    shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
    shell::WaylandSurface,
    shm::{slot::SlotPool, Shm},
};
use smithay_client_toolkit::reexports::client::{globals::registry_queue_init, Connection};
use state::LumoDock;

delegate_compositor!(LumoDock);
delegate_output!(LumoDock);
delegate_shm!(LumoDock);
delegate_layer!(LumoDock);
delegate_seat!(LumoDock);
delegate_pointer!(LumoDock);
delegate_registry!(LumoDock);

pub const DOCK_H: u32 = 56;
pub const DOCK_W: u32 = 1920;
pub const ICON_SIZE: f32 = 36.0;
pub const ICON_MARGIN: f32 = 10.0;
pub const SLOT_W: f32 = ICON_SIZE + ICON_MARGIN * 2.0;
pub const MAGNIFY_MAX: f32 = 1.3;
pub const DOT_R: f32 = 3.0;
pub const SEPARATOR_W: f32 = 1.0;
pub const SEPARATOR_H: f32 = 28.0;
pub const DOCK_RADIUS: f32 = 16.0;

fn main() {
    let cfg = config::DockConfig::load();
    let conn = Connection::connect_to_env().expect("wayland connect");
    let (globals, mut queue) = registry_queue_init::<LumoDock>(&conn).expect("registry init");
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("wlr_layer_shell");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm");
    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(&qh, surface, Layer::Bottom, Some("lumo-dock"), None);
    layer.set_anchor(Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_size(DOCK_W, DOCK_H);
    layer.set_exclusive_zone(DOCK_H as i32);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.commit();
    let pool = SlotPool::new(DOCK_W as usize * DOCK_H as usize * 4 * 2, &shm).expect("SlotPool");
    let mut state = LumoDock::new(globals, qh.clone(), shm, pool, layer, cfg);
    let mut last_anim = Instant::now();
    let mut last_refresh = Instant::now();
    while state.running {
        let anim_dt = last_anim.elapsed().as_secs_f32();
        if state.animating() && anim_dt >= 1.0 / 62.0 {
            last_anim = Instant::now();
            state.tick_springs(anim_dt);
            state.redraw(&qh);
        }
        if last_refresh.elapsed() >= Duration::from_secs(2) {
            last_refresh = Instant::now();
            state.refresh_running();
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
        match queue.dispatch_pending(&mut state) {
            Err(e) => { let s = format!("{e:?}"); if s.contains("ConnectionReset") || s.contains("BrokenPipe") { break; } }
            Ok(_) => {}
        }
        if conn.flush().is_err() { break; }
    }
}
