//! state.rs - event loop Wayland do lumo-notif.

use crate::dbus::NotifEvent;
use crate::history::{History, HistoryEntry};
use crate::paint::{self, ToastRender, TOAST_W};
use lumo_notif::toast_logic::{effective_timeout_ms, should_expire, slot_to_evict_for_critical};
use lumo_notif::urgency::Urgency;
use lumo_animation::Spring;
use nix::poll::{poll, PollFd, PollFlags, PollTimeout};
use smithay_client_toolkit::reexports::client::{globals::registry_queue_init, Connection};
use smithay_client_toolkit::{
    compositor::CompositorState,
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_seat,
    delegate_shm,
    output::OutputState,
    registry::RegistryState,
    seat::SeatState,
    shell::wlr_layer::{Anchor, KeyboardInteractivity, Layer, LayerShell},
    shell::WaylandSurface,
    shm::{slot::SlotPool, Shm},
};
use std::collections::VecDeque;
use std::time::Instant;
use tokio::sync::mpsc;

const MAX_TOASTS: usize = 3;
pub const SURFACE_W: u32 = 380;
pub const SURFACE_H: u32 = 296;

struct Toast {
    id: u32,
    app_name: String,
    summary: String,
    body: String,
    slide: Spring,
    created_at: Instant,
    timeout_ms: u64,
    hover: bool,
    dismissing: bool,
    urgency: Urgency,
}

impl Toast {
    fn new(
        id: u32,
        app_name: String,
        summary: String,
        body: String,
        timeout_ms: u64,
        urgency: Urgency,
    ) -> Self {
        let mut slide = Spring::snappy();
        slide.snap_to(TOAST_W);
        slide.set_target(0.0);
        Self {
            id,
            app_name,
            summary,
            body,
            slide,
            created_at: Instant::now(),
            timeout_ms,
            hover: false,
            dismissing: false,
            urgency,
        }
    }
    fn dismiss(&mut self) {
        self.dismissing = true;
        self.slide.set_target(TOAST_W);
    }
    fn should_remove(&self) -> bool {
        self.dismissing && self.slide.settled()
    }
    fn expired(&self) -> bool {
        should_expire(
            self.urgency,
            self.timeout_ms,
            self.created_at.elapsed(),
            self.hover,
            self.dismissing,
        )
    }
}

struct NotifState {
    registry: RegistryState,
    output_state: OutputState,
    shm: Shm,
    seat_state: SeatState,
    layer: smithay_client_toolkit::shell::wlr_layer::LayerSurface,
    pool: SlotPool,
    running: bool,
    configured: bool,
    toasts: VecDeque<Toast>,
    history: History,
}

delegate_compositor!(NotifState);
delegate_output!(NotifState);
delegate_shm!(NotifState);
delegate_layer!(NotifState);
delegate_seat!(NotifState);
delegate_registry!(NotifState);

impl NotifState {
    fn push_toast(
        &mut self,
        id: u32,
        app_name: String,
        summary: String,
        body: String,
        timeout_ms: i32,
        urgency: Urgency,
    ) {
        self.toasts.retain(|t| t.id != id);
        if self.toasts.len() >= MAX_TOASTS {
            // F1.5-B1 + M3 review fix:
            //   - Critical entrando: dismiss primeiro nao-critical; se fila so
            //     tem critical, NAO desloca (push alem do max — overflow critical
            //     visivel e melhor que perder critical antigo).
            //   - Nao-critical entrando: dismiss primeiro nao-critical disponivel;
            //     se so ha critical na fila, NAO desloca critical, push alem do max.
            let urgencies: Vec<_> = self.toasts.iter().map(|t| t.urgency).collect();
            if let Some(idx) = slot_to_evict_for_critical(&urgencies) {
                if let Some(t) = self.toasts.get_mut(idx) {
                    t.dismiss();
                }
            }
            // None: fila so tem critical -> nao desloca, push overflow consciente.
            let _ = urgency; // (urgency atual nao influencia: critical-fila preserva sempre)
        }
        let ms = effective_timeout_ms(timeout_ms, urgency);
        self.toasts.push_back(Toast::new(
            id,
            app_name.clone(),
            summary.clone(),
            body.clone(),
            ms,
            urgency,
        ));
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.history.push(HistoryEntry {
            id,
            app_name,
            summary,
            body,
            timestamp: ts,
            urgency,
        });
    }
    fn close_toast(&mut self, id: u32) {
        if let Some(t) = self.toasts.iter_mut().find(|t| t.id == id) {
            t.dismiss();
        }
    }
    fn tick(&mut self, dt: f32) {
        for t in &mut self.toasts {
            t.slide.tick(dt);
            if t.expired() {
                t.dismiss();
            }
        }
        self.toasts.retain(|t| !t.should_remove());
    }
    fn animating(&self) -> bool {
        self.toasts.iter().any(|t| !t.slide.settled())
    }
    fn redraw(&mut self, qh: &smithay_client_toolkit::reexports::client::QueueHandle<Self>) {
        if !self.configured {
            return;
        }
        let w = SURFACE_W as usize;
        let h = SURFACE_H as usize;
        use smithay_client_toolkit::reexports::client::protocol::wl_shm;
        let (buffer, canvas) = match self.pool.create_buffer(
            w as i32,
            h as i32,
            (w * 4) as i32,
            wl_shm::Format::Argb8888,
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[lumo-notif] buf: {:?}", e);
                return;
            }
        };
        let mut pm = tiny_skia::PixmapMut::from_bytes(canvas, w as u32, h as u32).expect("pix");
        let renders: Vec<ToastRender> = self
            .toasts
            .iter()
            .map(|t| ToastRender {
                id: t.id,
                slide_x: t.slide.value,
                summary: t.summary.clone(),
                app_name: t.app_name.clone(),
                body: t.body.clone(),
                urgency: t.urgency,
            })
            .collect();
        paint::paint_toasts(&mut pm, &renders, w as u32, h as u32);
        let surf = self.layer.wl_surface();
        surf.damage_buffer(0, 0, w as i32, h as i32);
        buffer.attach_to(surf).ok();
        surf.commit();
    }
}

use smithay_client_toolkit::reexports::client::{
    protocol::{wl_output, wl_seat, wl_surface},
    QueueHandle,
};
use smithay_client_toolkit::{
    compositor::CompositorHandler,
    output::OutputHandler,
    registry::ProvidesRegistryState,
    registry_handlers,
    seat::SeatHandler,
    shell::wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    shm::ShmHandler,
};

impl CompositorHandler for NotifState {
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
    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
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
impl OutputHandler for NotifState {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}
impl ShmHandler for NotifState {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}
impl LayerShellHandler for NotifState {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.running = false;
    }
    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &LayerSurface,
        _: LayerSurfaceConfigure,
        _: u32,
    ) {
        self.configured = true;
        self.redraw(qh);
    }
}
impl SeatHandler for NotifState {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        _: smithay_client_toolkit::seat::Capability,
    ) {
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        _: smithay_client_toolkit::seat::Capability,
    ) {
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}
impl ProvidesRegistryState for NotifState {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}

pub async fn run(mut rx: mpsc::Receiver<NotifEvent>) {
    let conn = Connection::connect_to_env().expect("wayland connect");
    let (globals, mut queue) = registry_queue_init::<NotifState>(&conn).expect("registry init");
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh).expect("wl_compositor");
    let layer_shell = LayerShell::bind(&globals, &qh).expect("wlr_layer_shell");
    let shm = Shm::bind(&globals, &qh).expect("wl_shm");
    let surface = compositor.create_surface(&qh);
    let layer =
        layer_shell.create_layer_surface(&qh, surface, Layer::Overlay, Some("lumo-notif"), None);
    layer.set_anchor(Anchor::TOP | Anchor::RIGHT);
    layer.set_size(SURFACE_W, SURFACE_H);
    layer.set_margin(16, 16, 0, 0);
    layer.set_exclusive_zone(-1);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.commit();
    let pool =
        SlotPool::new(SURFACE_W as usize * SURFACE_H as usize * 4 * 2, &shm).expect("SlotPool");
    let mut state = NotifState {
        registry: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        seat_state: SeatState::new(&globals, &qh),
        layer,
        pool,
        running: true,
        configured: false,
        toasts: VecDeque::new(),
        history: History::load(),
    };
    let mut last_tick = Instant::now();
    while state.running {
        while let Ok(ev) = rx.try_recv() {
            match ev {
                NotifEvent::Notify {
                    id,
                    app_name,
                    summary,
                    body,
                    timeout_ms,
                    urgency,
                } => {
                    state.push_toast(id, app_name, summary, body, timeout_ms, urgency);
                    state.redraw(&qh);
                }
                NotifEvent::CloseNotification { id } => {
                    state.close_toast(id);
                    state.redraw(&qh);
                }
            }
        }
        let now = Instant::now();
        let dt = now.duration_since(last_tick).as_secs_f32();
        if dt >= 0.016 {
            last_tick = now;
            state.tick(dt);
            if state.animating() || !state.toasts.is_empty() {
                state.redraw(&qh);
            }
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
