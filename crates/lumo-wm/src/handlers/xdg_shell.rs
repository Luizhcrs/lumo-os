//! xdg_shell delegate - top-level windows + popups (menus, tooltips).
//!
//! MVP: aceita toplevels, faz commit inicial vazio (cliente decide
//! tamanho), mapeia em `Space` na origem. Tiling/posicionamento
//! refinado entra na Fase 5.2.

use smithay::desktop::Window;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::utils::Serial;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};

use crate::state::LumoState;

impl XdgShellHandler for LumoState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface);
        self.space
            .map_element(window, (0, 0), true);
    }

    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {
        // Popup positioning entra com PopupManager na Fase 5.2.
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: WlSeat, _serial: Serial) {
        // Popup grab handling - placeholder.
    }

    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
        // Reposition handling - placeholder.
    }
}

smithay::delegate_xdg_shell!(LumoState);
