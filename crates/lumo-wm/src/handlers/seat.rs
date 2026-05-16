//! wl_seat delegate - input devices (keyboard, pointer, touch).
//!
//! MVP: implementa SeatHandler com cursor sem renderizacao custom.

use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

use crate::state::LumoState;

impl SeatHandler for LumoState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: CursorImageStatus) {
        // Cursor rendering entra na Fase 5.3 (lumo-gfx integration).
    }

    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&WlSurface>) {
        // Foco persistido em LumoState futuramente.
    }
}

smithay::delegate_seat!(LumoState);
