//! picker.rs - Overlay layer-shell para selecao de clipboard (W11.B).
//!
//! Lista vertical de max 10 entradas visiveis, filtro de busca.
//! Enter = pasta entrada selecionada, Esc = fechar.

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
use smithay_client_toolkit::reexports::client::{globals::registry_queue_init, Connection, QueueHandle};
use smithay_client_toolkit::reexports::client::protocol::{wl_output, wl_seat, wl_surface};
use tiny_skia::{Color, Paint, PixmapMut, Rect, Transform};

use lumo_foundation::LumoColors;
use crate::history::ClipEntry;

pub const PICKER_W: u32 = 480;
pub const PICKER_H: u32 = 400;
const ROW_H: f32 = 36.0;
const PAD: f32 = 12.0;
const SEARCH_H: f32 = 40.0;
pub const MAX_VISIBLE: usize = 10;

pub struct PickerState {
    pub registry: RegistryState,
    pub output_state: OutputState,
    pub shm: Shm,
    pub seat_state: SeatState,
    pub layer: smithay_client_toolkit::shell::wlr_layer::LayerSurface,
    pub pool: SlotPool,
    pub running: bool,
    pub configured: bool,
    pub entries: Vec<ClipEntry>,
    pub filtered: Vec<usize>,
    pub search: String,
    pub hover_idx: usize,
    pub scroll_off: usize,
    pub result: Option<usize>,
}

delegate_compositor!(PickerState);
delegate_output!(PickerState);
delegate_shm!(PickerState);
delegate_layer!(PickerState);
delegate_seat!(PickerState);
delegate_registry!(PickerState);
delegate_keyboard!(PickerState);

impl PickerState {
    pub fn filter(&mut self) {
        let q = self.search.to_lowercase();
        self.filtered = (0..self.entries.len())
            .filter(|&i| {
                if q.is_empty() {
                    return true;
                }
                self.entries[i].preview(200).to_lowercase().contains(&q)
            })
            .collect();
        self.scroll_off = 0;
        if self.hover_idx >= self.filtered.len() && !self.filtered.is_empty() {
            self.hover_idx = 0;
        }
    }

    pub fn redraw(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured {
            return;
        }
        let w = PICKER_W as usize;
        let h = PICKER_H as usize;
        use smithay_client_toolkit::reexports::client::protocol::wl_shm;
        let (buffer, canvas) = match self.pool.create_buffer(
            w as i32,
            h as i32,
            (w * 4) as i32,
            wl_shm::Format::Argb8888,
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[lumo-clip] buf: {:?}", e);
                return;
            }
        };
        let mut pm = PixmapMut::from_bytes(canvas, w as u32, h as u32).expect("pix");
        let hover = self.hover_idx;
        let scroll = self.scroll_off;
        let filtered = self.filtered.clone();
        paint_picker(&mut pm, hover, scroll, &filtered);
        let surf = self.layer.wl_surface();
        surf.damage_buffer(0, 0, w as i32, h as i32);
        buffer.attach_to(surf).ok();
        surf.commit();
    }
}

fn paint_picker(pm: &mut PixmapMut, hover_idx: usize, scroll_off: usize, filtered: &[usize]) {
    let palette = LumoColors::dark();
    let bg = hex_color(palette.bg, 0xFF);
    let row_bg = hex_color(palette.bg_subtle, 0xFF);
    let accent = hex_color(palette.accent, 0xFF);
    pm.fill(bg);
    let mut p = Paint::default();
    p.set_color(row_bg);
    if let Some(r) = Rect::from_xywh(PAD, PAD, PICKER_W as f32 - PAD * 2.0, SEARCH_H) {
        pm.fill_rect(r, &p, Transform::identity(), None);
    }
    let start = scroll_off;
    let end = (start + MAX_VISIBLE).min(filtered.len());
    for (vi, _fi) in (start..end).enumerate() {
        let y = PAD + SEARCH_H + 4.0 + vi as f32 * ROW_H;
        let is_hover = (scroll_off + vi) == hover_idx;
        let row_color = if is_hover { accent } else { row_bg };
        let mut rp = Paint::default();
        rp.set_color(row_color);
        if let Some(r) = Rect::from_xywh(PAD, y, PICKER_W as f32 - PAD * 2.0, ROW_H - 2.0) {
            pm.fill_rect(r, &rp, Transform::identity(), None);
        }
    }
}

fn hex_color(hex: u32, alpha: u8) -> Color {
    Color::from_rgba8(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
        alpha,
    )
}

use smithay_client_toolkit::{
    compositor::CompositorHandler,
    output::OutputHandler,
    registry::ProvidesRegistryState,
    registry_handlers,
    shell::wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    shm::ShmHandler,
};

impl CompositorHandler for PickerState {
    fn scale_factor_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: i32) {}
    fn transform_changed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: wl_output::Transform) {}
    fn frame(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: u32) {
        self.redraw(qh);
    }
    fn surface_enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
    fn surface_leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &wl_surface::WlSurface, _: &wl_output::WlOutput) {}
}
impl OutputHandler for PickerState {
    fn output_state(&mut self) -> &mut OutputState { &mut self.output_state }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}
impl ShmHandler for PickerState {
    fn shm_state(&mut self) -> &mut Shm { &mut self.shm }
}
impl LayerShellHandler for PickerState {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.running = false;
    }
    fn configure(&mut self, _: &Connection, qh: &QueueHandle<Self>, _: &LayerSurface, _: LayerSurfaceConfigure, _: u32) {
        self.configured = true;
        self.redraw(qh);
    }
}
impl SeatHandler for PickerState {
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
impl KeyboardHandler for PickerState {
    fn enter(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32, _: &[u32], _: &[Keysym]) {}
    fn leave(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard, _: &wl_surface::WlSurface, _: u32) {}
    fn press_key(
        &mut self, _: &Connection, qh: &QueueHandle<Self>,
        _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard,
        _: u32, ev: KeyEvent,
    ) {
        match ev.keysym {
            Keysym::Escape => {
                self.running = false;
            }
            Keysym::Return | Keysym::KP_Enter => {
                if !self.filtered.is_empty() {
                    self.result = Some(self.filtered[self.hover_idx.min(self.filtered.len() - 1)]);
                    self.running = false;
                }
            }
            Keysym::Up => {
                if self.hover_idx > 0 {
                    self.hover_idx -= 1;
                }
                if self.hover_idx < self.scroll_off {
                    self.scroll_off = self.hover_idx;
                }
                self.redraw(qh);
            }
            Keysym::Down => {
                if self.hover_idx + 1 < self.filtered.len() {
                    self.hover_idx += 1;
                }
                if self.hover_idx >= self.scroll_off + MAX_VISIBLE {
                    self.scroll_off = self.hover_idx + 1 - MAX_VISIBLE;
                }
                self.redraw(qh);
            }
            Keysym::BackSpace => {
                self.search.pop();
                self.filter();
                self.redraw(qh);
            }
            _ => {
                if let Some(ch) = ev.keysym.key_char() {
                    self.search.push(ch);
                    self.filter();
                    self.redraw(qh);
                }
            }
        }
    }
    fn release_key(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard, _: u32, _: KeyEvent) {}
    fn update_modifiers(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &smithay_client_toolkit::reexports::client::protocol::wl_keyboard::WlKeyboard, _: u32, _: Modifiers, _: u32) {}
}
impl ProvidesRegistryState for PickerState {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry }
    registry_handlers![OutputState, SeatState];
}

pub fn run_picker(entries: Vec<ClipEntry>) -> Option<ClipEntry> {
    if entries.is_empty() {
        return None;
    }
    let conn = Connection::connect_to_env().ok()?;
    let (globals, mut queue) = registry_queue_init::<PickerState>(&conn).ok()?;
    let qh = queue.handle();
    let compositor = CompositorState::bind(&globals, &qh).ok()?;
    let layer_shell = LayerShell::bind(&globals, &qh).ok()?;
    let shm = Shm::bind(&globals, &qh).ok()?;
    let surface = compositor.create_surface(&qh);
    let layer = layer_shell.create_layer_surface(
        &qh, surface, Layer::Overlay, Some("lumo-clip-picker"), None,
    );
    layer.set_anchor(Anchor::empty());
    layer.set_size(PICKER_W, PICKER_H);
    layer.set_keyboard_interactivity(KeyboardInteractivity::Exclusive);
    layer.commit();
    let pool = SlotPool::new(PICKER_W as usize * PICKER_H as usize * 4 * 2, &shm).ok()?;
    let n = entries.len();
    let mut state = PickerState {
        registry: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &qh),
        shm,
        seat_state: SeatState::new(&globals, &qh),
        layer,
        pool,
        running: true,
        configured: false,
        entries,
        filtered: (0..n).collect(),
        search: String::new(),
        hover_idx: 0,
        scroll_off: 0,
        result: None,
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
    state.result.map(|i| state.entries.swap_remove(i))
}
