//! xdg_shell delegate - top-level windows + popups.

use smithay::desktop::{PopupKind, Window};
use smithay::input::pointer::{Focus, GrabStartData};
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::Serial;
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};

use crate::input::move_grab::MoveSurfaceGrab;
use crate::state::LumoState;

impl XdgShellHandler for LumoState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface.clone());
        let pos = self.next_tile_position();
        self.space.map_element(window.clone(), pos, true);

        // W12.A: auto-tile on new toplevel if tiling mode is active.
        {
            let (ow, oh) = self.output_dimensions();
            crate::tiling::apply_tiling(&mut self.space, self.tiling_mode, ow, oh);
        }

        // W9.A: init opening animation (a11y guard included in new_opening).
        let a11y = lumo_foundation::A11yTokens::load_from_disk();
        self.window_anim.insert_opening(surface.wl_surface(), a11y.reduced_motion);

        // Configure inicial: cliente decide tamanho mas anunciamos
        // estado Activated.
        surface.with_pending_state(|state| {
            state.states.set(
                smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Activated,
            );
        });
        let _ = surface.send_configure();

        // L1: FocusManager gerencia foco na nova janela.
        if let Some(wl) = window.wl_surface() {
            let serial = smithay::utils::SERIAL_COUNTER.next_serial();
            let surf: WlSurface = wl.into_owned();
            let kb = self.keyboard.clone();
            let new_focus = self.focus_manager.new_toplevel(surf);
            kb.set_focus(self, new_focus, serial);
        }
    }

    fn new_popup(&mut self, surface: PopupSurface, positioner: PositionerState) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
        });
        // D1.3: send_configure obrigatorio. Sem isso o cliente aguarda configure
        // antes de commitar buffers -- popup registrado mas nunca renderizado.
        let _ = surface.send_configure();
        if let Err(err) = self.popups.track_popup(PopupKind::from(surface)) {
            tracing::warn!(?err, "Falha ao registrar popup xdg");
        }
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        // W9.A: register closing animation (delayed destroy via close_done check in render tick).
        let a11y = lumo_foundation::A11yTokens::load_from_disk();
        self.window_anim.insert_closing(surface.wl_surface(), a11y.reduced_motion);

        // M1: limpa SSD entry ao fechar toplevel.
        self.ssd_windows.remove(surface.wl_surface());
        let to_remove = self
            .space
            .elements()
            .find(|w| w.toplevel().map(|t| t == &surface).unwrap_or(false))
            .cloned();
        if let Some(window) = to_remove {
            self.space.unmap_elem(&window);
        }
        // T1.5: N5 MRU -- ao fechar toplevel, tenta focar a surface que
        // estava focada antes (prev_focus), se ainda viva no space.
        // Fallback: primeiro toplevel restante. Sem toplevels -> None.
        let prev = self.focus_manager.prev_focus.take();
        let prev_alive = prev.as_ref().and_then(|ps| {
            self.space.elements()
                .find(|w| w.wl_surface().map(|s| *s == *ps).unwrap_or(false))
                .and_then(|w| w.wl_surface())
                .map(|s| s.into_owned())
        });
        let next_surface: Option<WlSurface> = prev_alive.or_else(|| {
            self.space
                .elements()
                .next()
                .and_then(|w| w.wl_surface())
                .map(|s| s.into_owned())
        });
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        let kb = self.keyboard.clone();
        let new_focus = self.focus_manager.close_toplevel(next_surface);
        kb.set_focus(self, new_focus, serial);
    }

    fn grab(&mut self, surface: PopupSurface, seat: WlSeat, serial: Serial) {
        // Q2: aceita popup grab para xdg_popup (right-click menus GTK/Qt).
        let seat = match smithay::input::Seat::<Self>::from_resource(&seat) {
            Some(s) => s,
            None => return,
        };
        let kind = PopupKind::Xdg(surface);
        let root = match smithay::desktop::find_popup_root_surface(&kind) {
            Ok(r) => r,
            Err(_) => return,
        };
        if let Err(err) = self.popups.grab_popup(root, kind, &seat, serial) {
            tracing::warn!(?err, "Q2 grab_popup falhou");
        }
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        surface.send_repositioned(token);
    }

    fn move_request(&mut self, surface: ToplevelSurface, _seat: WlSeat, serial: Serial) {
        // Localiza Window correspondente ao toplevel no espaco.
        let window = self
            .space
            .elements()
            .find(|w| w.toplevel().map(|t| t == &surface).unwrap_or(false))
            .cloned();

        let window = match window {
            Some(w) => w,
            None => return,
        };

        let initial_window_location = self
            .space
            .element_location(&window)
            .unwrap_or_default();

        // Constroi GrabStartData com estado atual do pointer.
        let pointer = self.pointer.clone();
        let start_data = GrabStartData {
            focus: pointer.current_focus().map(|s| {
                let loc = self
                    .surface_under(self.pointer_location)
                    .map(|(_, l)| l.to_f64())
                    .unwrap_or_default();
                (s, loc)
            }),
            button: 0x110, // BTN_LEFT
            location: self.pointer_location,
        };

        let grab = MoveSurfaceGrab {
            start_data,
            window,
            initial_window_location,
        };

        pointer.set_grab(self, grab, serial, Focus::Clear);
        tracing::debug!("move_request: grab iniciado serial={:?}", serial);
    }
}

smithay::delegate_xdg_shell!(LumoState);
