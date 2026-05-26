//! state.rs - LumoDock struct + Wayland handler traits.

use crate::config::DockConfig;
use crate::paint;
use lumo_animation::Spring;
use smithay_client_toolkit::reexports::client::{
    globals::GlobalList,
    protocol::{wl_output, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};
use smithay_client_toolkit::shm::slot::SlotPool;
use smithay_client_toolkit::{
    compositor::CompositorHandler,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        pointer::{PointerEvent, PointerEventKind, PointerHandler, ThemeSpec, ThemedPointer},
        Capability, SeatHandler, SeatState,
    },
    shell::wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    shell::WaylandSurface,
    shm::{Shm, ShmHandler},
};
use std::collections::HashMap;

pub struct LumoDock {
    pub registry: RegistryState,
    pub output_state: OutputState,
    pub shm: Shm,
    pub seat_state: SeatState,
    pub layer: LayerSurface,
    pub pool: SlotPool,
    pub cfg: DockConfig,
    pub running: bool,
    pub configured: bool,
    pub width: u32,
    pub scales: Vec<Spring>,
    pub hover_idx: i32,
    pub running_procs: HashMap<String, bool>,
    pub pointer: Option<ThemedPointer>,
    pub pointer_x: f32,
    pub pointer_y: f32,
    pub slot_rects: Vec<(f32, f32)>,
    pub trash_rect: Option<(f32, f32)>,
}

impl LumoDock {
    pub fn new(
        globals: GlobalList,
        qh: QueueHandle<Self>,
        shm: Shm,
        pool: SlotPool,
        layer: LayerSurface,
        cfg: DockConfig,
    ) -> Self {
        let n = cfg.slots.len();
        let mut scales = Vec::with_capacity(n + 1);
        for _ in 0..=n {
            let mut s = Spring::snappy();
            s.snap_to(1.0);
            scales.push(s);
        }
        Self {
            registry: RegistryState::new(&globals),
            output_state: OutputState::new(&globals, &qh),
            shm,
            seat_state: SeatState::new(&globals, &qh),
            layer,
            pool,
            cfg,
            running: true,
            configured: false,
            width: crate::DOCK_W,
            scales,
            hover_idx: -1,
            running_procs: HashMap::new(),
            pointer: None,
            pointer_x: 0.0,
            pointer_y: 0.0,
            slot_rects: Vec::new(),
            trash_rect: None,
        }
    }
    pub fn animating(&self) -> bool {
        self.scales.iter().any(|s| !s.settled())
    }
    pub fn tick_springs(&mut self, dt: f32) {
        for s in &mut self.scales {
            s.tick(dt);
        }
    }
    pub fn refresh_running(&mut self) {
        for slot in &self.cfg.slots {
            if slot.process.is_empty() {
                continue;
            }
            self.running_procs
                .insert(slot.process.clone(), is_process_running(&slot.process));
        }
    }
    pub fn redraw(&mut self, qh: &QueueHandle<Self>) {
        if !self.configured {
            return;
        }
        let w = self.width as usize;
        let h = crate::DOCK_H as usize;
        // sctk 0.19: create_buffer returns (Buffer, &mut [u8])
        let (buffer, canvas) = match self.pool.create_buffer(
            w as i32,
            h as i32,
            (w * 4) as i32,
            wl_shm::Format::Argb8888,
        ) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[lumo-dock] buf: {:?}", e);
                return;
            }
        };
        let mut pm = tiny_skia::PixmapMut::from_bytes(canvas, w as u32, h as u32).expect("pixmap");
        let (sr, tr) = paint::paint_dock(
            &mut pm,
            w as u32,
            h as u32,
            &self.cfg.slots,
            &self.scales,
            self.hover_idx,
            &self.running_procs,
        );
        self.slot_rects = sr;
        self.trash_rect = tr;
        let surface = self.layer.wl_surface();
        surface.damage_buffer(0, 0, w as i32, h as i32);
        buffer.attach_to(surface).ok();
        surface.commit();
    }
}

fn is_process_running(name: &str) -> bool {
    let Ok(e) = std::fs::read_dir("/proc") else {
        return false;
    };
    for entry in e.flatten() {
        let f = entry.file_name();
        let s = f.to_string_lossy();
        if s.chars().all(|c| c.is_ascii_digit()) {
            if std::fs::read_to_string(entry.path().join("comm"))
                .unwrap_or_default()
                .trim()
                == name
            {
                return true;
            }
        }
    }
    false
}

impl CompositorHandler for LumoDock {
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
impl OutputHandler for LumoDock {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }
    fn new_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn update_output(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
    fn output_destroyed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_output::WlOutput) {}
}
impl ShmHandler for LumoDock {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}
impl LayerShellHandler for LumoDock {
    fn closed(&mut self, _: &Connection, _: &QueueHandle<Self>, _: &LayerSurface) {
        self.running = false;
    }
    fn configure(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &LayerSurface,
        c: LayerSurfaceConfigure,
        _: u32,
    ) {
        if c.new_size.0 != 0 {
            self.width = c.new_size.0;
        }
        self.configured = true;
        self.refresh_running();
        self.redraw(qh);
    }
}
impl SeatHandler for LumoDock {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }
    fn new_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
    fn new_capability(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Pointer && self.pointer.is_none() {
            // sctk 0.19: get_pointer_with_theme(qh, seat, wl_shm, surface, theme)
            if let Ok(p) = self.seat_state.get_pointer_with_theme(
                qh,
                &seat,
                self.shm.wl_shm(),
                self.layer.wl_surface().clone(),
                ThemeSpec::System,
            ) {
                self.pointer = Some(p);
            }
        }
    }
    fn remove_capability(
        &mut self,
        _: &Connection,
        _: &QueueHandle<Self>,
        _: wl_seat::WlSeat,
        cap: Capability,
    ) {
        if cap == Capability::Pointer {
            self.pointer = None;
        }
    }
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}
impl PointerHandler for LumoDock {
    fn pointer_frame(
        &mut self,
        _: &Connection,
        qh: &QueueHandle<Self>,
        _: &smithay_client_toolkit::reexports::client::protocol::wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for ev in events {
            match ev.kind {
                PointerEventKind::Motion { .. } => {
                    self.pointer_x = ev.position.0 as f32;
                    let nh = crate::input::hit_test_slot(
                        self.pointer_x,
                        &self.slot_rects,
                        self.trash_rect,
                    );
                    if nh != self.hover_idx {
                        if self.hover_idx >= 0 {
                            let i = self.hover_idx as usize;
                            if i < self.scales.len() {
                                self.scales[i].set_target(1.0);
                            }
                        }
                        self.hover_idx = nh;
                        if nh >= 0 {
                            let i = nh as usize;
                            if i < self.scales.len() {
                                self.scales[i].set_target(crate::MAGNIFY_MAX);
                            }
                        }
                    }
                    self.redraw(qh);
                }
                PointerEventKind::Leave { .. } => {
                    if self.hover_idx >= 0 {
                        let i = self.hover_idx as usize;
                        if i < self.scales.len() {
                            self.scales[i].set_target(1.0);
                        }
                    }
                    self.hover_idx = -1;
                    self.redraw(qh);
                }
                PointerEventKind::Press { button, .. } => {
                    if button == 0x110 {
                        crate::input::handle_click(self.hover_idx, &self.cfg.slots);
                    }
                }
                _ => {}
            }
        }
    }
}
impl ProvidesRegistryState for LumoDock {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry
    }
    registry_handlers![OutputState, SeatState];
}
