//! desktop/handlers.rs - Wayland handlers + LumoDesktop::redraw / hit_test_menu.

use smithay_client_toolkit::{
    compositor::CompositorHandler,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        pointer::ThemeSpec,
        Capability, SeatHandler, SeatState,
    },
    shell::wlr_layer::{LayerShellHandler, LayerSurface, LayerSurfaceConfigure},
    shell::WaylandSurface,
    shm::{Shm, ShmHandler},
};
use smithay_client_toolkit::reexports::client::{
    protocol::{wl_output, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};
use tiny_skia::Pixmap;

use crate::desktop::icons::{paint_icons, paint_ctx_menu};
use crate::desktop::menu_overlay::{paint_menu_at, MENU_ITEMS, MENU_W};
use crate::desktop::rubber_band::paint_rubber_band;
use crate::desktop::state::{LumoDesktop, MENU_OFFSET, OUTPUT_H, OUTPUT_W};
use crate::menu;

impl LumoDesktop {
    pub fn redraw(&mut self, _qh: &QueueHandle<Self>) {
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
        let accent_hex = palette.accent;
        let rb_snap = self.rubber_band;
        let ctx_snap = self.icons.ctx_menu;
        let ctx_hover = self.icons.ctx_hover;
        if let Some(mut px) = Pixmap::new(self.width, self.height) {
            {
                let mut canvas_mut = px.as_mut();
                // A34: rubber-band ANTES dos icons (background).
                paint_rubber_band(&mut canvas_mut, &rb_snap, accent_hex);
                // A33: icons.
                paint_icons(&mut canvas_mut, &self.icons, accent_hex);
                // A33: context menu de icon.
                if let Some((_, cx, cy)) = ctx_snap {
                    paint_ctx_menu(&mut canvas_mut, cx, cy, ctx_hover, accent_hex);
                }
                // A27: desktop menu.
                if menu_snap.visible {
                    paint_menu_at(&mut canvas_mut, menu_snap, surf_w, surf_h, &palette);
                }
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
    pub fn hit_test_menu(&self, px: f32, py: f32) -> Option<usize> {
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
                ThemeSpec::System,
            ) {
                self.pointer = Some(p);
                eprintln!("[lumo-desktop] pointer ThemedPointer adquirido");
            }
        }
    }
    fn remove_capability(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat, _: Capability) {}
    fn remove_seat(&mut self, _: &Connection, _: &QueueHandle<Self>, _: wl_seat::WlSeat) {}
}

impl ProvidesRegistryState for LumoDesktop {
    fn registry(&mut self) -> &mut RegistryState { &mut self.registry }
    registry_handlers!(OutputState, SeatState);
}
