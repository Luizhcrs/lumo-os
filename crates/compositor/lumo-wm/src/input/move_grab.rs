//! MoveSurfaceGrab — pointer grab que move toplevel seguindo cursor.
//!
//! Ativado quando cliente emite xdg_toplevel.move (CSD header drag).
//! Implementa PointerGrab: em cada MotionEvent, calcula delta desde
//! o inicio do grab e reposiciona o Window no Space.
//! Libera o grab quando o botao que iniciou eh solto.

use smithay::desktop::{Space, Window};
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent,
    GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent,
    GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent,
    GrabStartData, MotionEvent, PointerGrab, PointerInnerHandle, RelativeMotionEvent,
};
use smithay::input::{pointer::Focus, SeatHandler};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point};

use crate::state::LumoState;

/// Grab ativo enquanto usuario arrasta janela pelo CSD header.
pub struct MoveSurfaceGrab {
    /// Dados do click que iniciou o grab.
    pub start_data: GrabStartData<LumoState>,
    /// Janela que esta sendo movida.
    pub window: Window,
    /// Posicao inicial da janela no espaco (no inicio do grab).
    pub initial_window_location: Point<i32, Logical>,
}

impl PointerGrab<LumoState> for MoveSurfaceGrab {
    fn motion(
        &mut self,
        data: &mut LumoState,
        handle: &mut PointerInnerHandle<'_, LumoState>,
        _focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &MotionEvent,
    ) {
        // Sem foco durante drag (cursor navega livre).
        handle.motion(data, None, event);

        let delta = event.location - self.start_data.location;
        let new_loc = self.initial_window_location + delta.to_i32_round();

        data.space.map_element(self.window.clone(), new_loc, true);

        #[cfg(feature = "drm-backend")]
        {
            data.drm_force_repaint = true;
        }
    }

    fn relative_motion(
        &mut self,
        data: &mut LumoState,
        handle: &mut PointerInnerHandle<'_, LumoState>,
        focus: Option<(WlSurface, Point<f64, Logical>)>,
        event: &RelativeMotionEvent,
    ) {
        handle.relative_motion(data, focus, event);
    }

    fn button(
        &mut self,
        data: &mut LumoState,
        handle: &mut PointerInnerHandle<'_, LumoState>,
        event: &ButtonEvent,
    ) {
        handle.button(data, event);

        // Soltar grab quando o botao que iniciou for liberado.
        if event.button == self.start_data.button
            && event.state == smithay::backend::input::ButtonState::Released
        {
            handle.unset_grab(self, data, event.serial, event.time, true);
        }
    }

    fn axis(
        &mut self,
        data: &mut LumoState,
        handle: &mut PointerInnerHandle<'_, LumoState>,
        details: AxisFrame,
    ) {
        handle.axis(data, details);
    }

    fn frame(
        &mut self,
        data: &mut LumoState,
        handle: &mut PointerInnerHandle<'_, LumoState>,
    ) {
        handle.frame(data);
    }

    fn gesture_swipe_begin(
        &mut self,
        data: &mut LumoState,
        handle: &mut PointerInnerHandle<'_, LumoState>,
        event: &GestureSwipeBeginEvent,
    ) {
        handle.gesture_swipe_begin(data, event);
    }

    fn gesture_swipe_update(
        &mut self,
        data: &mut LumoState,
        handle: &mut PointerInnerHandle<'_, LumoState>,
        event: &GestureSwipeUpdateEvent,
    ) {
        handle.gesture_swipe_update(data, event);
    }

    fn gesture_swipe_end(
        &mut self,
        data: &mut LumoState,
        handle: &mut PointerInnerHandle<'_, LumoState>,
        event: &GestureSwipeEndEvent,
    ) {
        handle.gesture_swipe_end(data, event);
    }

    fn gesture_pinch_begin(
        &mut self,
        data: &mut LumoState,
        handle: &mut PointerInnerHandle<'_, LumoState>,
        event: &GesturePinchBeginEvent,
    ) {
        handle.gesture_pinch_begin(data, event);
    }

    fn gesture_pinch_update(
        &mut self,
        data: &mut LumoState,
        handle: &mut PointerInnerHandle<'_, LumoState>,
        event: &GesturePinchUpdateEvent,
    ) {
        handle.gesture_pinch_update(data, event);
    }

    fn gesture_pinch_end(
        &mut self,
        data: &mut LumoState,
        handle: &mut PointerInnerHandle<'_, LumoState>,
        event: &GesturePinchEndEvent,
    ) {
        handle.gesture_pinch_end(data, event);
    }

    fn gesture_hold_begin(
        &mut self,
        data: &mut LumoState,
        handle: &mut PointerInnerHandle<'_, LumoState>,
        event: &GestureHoldBeginEvent,
    ) {
        handle.gesture_hold_begin(data, event);
    }

    fn gesture_hold_end(
        &mut self,
        data: &mut LumoState,
        handle: &mut PointerInnerHandle<'_, LumoState>,
        event: &GestureHoldEndEvent,
    ) {
        handle.gesture_hold_end(data, event);
    }

    fn start_data(&self) -> &GrabStartData<LumoState> {
        &self.start_data
    }
}
