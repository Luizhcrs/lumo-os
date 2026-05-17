//! Stubs/glue pra protocolos opcionais.

use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::fractional_scale::FractionalScaleHandler;
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::selection::primary_selection::{
    PrimarySelectionHandler, PrimarySelectionState,
};
use smithay::wayland::tablet_manager::TabletSeatHandler;
use smithay::wayland::xdg_activation::{
    XdgActivationHandler, XdgActivationState, XdgActivationToken, XdgActivationTokenData,
};
use smithay::wayland::xdg_toplevel_icon::XdgToplevelIconHandler;

use crate::state::LumoState;

// --- Primary selection --------------------------------------------------

impl PrimarySelectionHandler for LumoState {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}
smithay::delegate_primary_selection!(LumoState);

// --- XDG activation ----------------------------------------------------

impl XdgActivationHandler for LumoState {
    fn activation_state(&mut self) -> &mut XdgActivationState {
        &mut self.xdg_activation_state
    }

    fn request_activation(
        &mut self,
        _token: XdgActivationToken,
        _token_data: XdgActivationTokenData,
        surface: WlSurface,
    ) {
        let target = self
            .space
            .elements()
            .find(|w| {
                w.wl_surface()
                    .map(|s| s.as_ref() == &surface)
                    .unwrap_or(false)
            })
            .cloned();
        if let Some(window) = target {
            self.space.raise_element(&window, true);
            let serial = smithay::utils::SERIAL_COUNTER.next_serial();
            let kb = self.keyboard.clone();
            kb.set_focus(self, Some(surface), serial);
        }
    }
}
smithay::delegate_xdg_activation!(LumoState);

// --- Fractional scale (stub scale=1) ------------------------------------

impl FractionalScaleHandler for LumoState {
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        smithay::wayland::compositor::with_states(&surface, |states| {
            smithay::wayland::fractional_scale::with_fractional_scale(states, |fs| {
                fs.set_preferred_scale(1.0);
            });
        });
    }
}
smithay::delegate_fractional_scale!(LumoState);

// --- XDG toplevel icon (stub) -------------------------------------------

impl XdgToplevelIconHandler for LumoState {}
smithay::delegate_xdg_toplevel_icon!(LumoState);

// --- Tablet seat (stub - precisa pra delegate_cursor_shape) -------------

impl TabletSeatHandler for LumoState {}

// --- Cursor shape (stub - anuncia global, sem renderizar) ---------------
smithay::delegate_cursor_shape!(LumoState);
