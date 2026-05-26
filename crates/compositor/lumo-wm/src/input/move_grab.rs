//! MoveSurfaceGrab: pointer grab that moves a toplevel following the cursor.
//!
//! W9.B: snap edges (Aero Snap).
//! During drag, pointer near output edges triggers snap preview stored in
//! LumoState.snap_preview. On button release at edge -> apply layout.

use smithay::desktop::Window;
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, GestureHoldBeginEvent, GestureHoldEndEvent, GesturePinchBeginEvent,
    GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent, GestureSwipeEndEvent,
    GestureSwipeUpdateEvent, GrabStartData, MotionEvent, PointerGrab, PointerInnerHandle,
    RelativeMotionEvent,
};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::{Logical, Point, Size};

use crate::state::LumoState;

/// Snap zone detected during drag.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SnapZone {
    Left,
    Right,
    Maximize,
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl SnapZone {
    pub fn detect(pos: Point<f64, Logical>, out_w: i32, out_h: i32) -> Option<Self> {
        const EDGE_PX: f64 = 8.0;
        let x = pos.x;
        let y = pos.y;
        let near_left = x < EDGE_PX;
        let near_right = x > (out_w as f64 - EDGE_PX);
        let near_top = y < EDGE_PX;
        let near_bottom = y > (out_h as f64 - EDGE_PX);
        match (near_left, near_right, near_top, near_bottom) {
            (true, false, true, false) => Some(SnapZone::TopLeft),
            (false, true, true, false) => Some(SnapZone::TopRight),
            (true, false, false, true) => Some(SnapZone::BottomLeft),
            (false, true, false, true) => Some(SnapZone::BottomRight),
            (true, false, false, false) => Some(SnapZone::Left),
            (false, true, false, false) => Some(SnapZone::Right),
            (false, false, true, false) => Some(SnapZone::Maximize),
            _ => None,
        }
    }

    /// Returns (x, y, w, h) layout in logical pixels.
    /// W24: layout respect usable area (excludes bar layer-shell).
    /// usable_x/usable_y = offset, usable_w/usable_h = dims.
    pub fn layout_usable(
        self,
        usable_x: i32,
        usable_y: i32,
        usable_w: i32,
        usable_h: i32,
    ) -> (i32, i32, i32, i32) {
        let hw = usable_w / 2;
        let hh = usable_h / 2;
        match self {
            SnapZone::Left => (usable_x, usable_y, hw, usable_h),
            SnapZone::Right => (usable_x + hw, usable_y, usable_w - hw, usable_h),
            SnapZone::Maximize => (usable_x, usable_y, usable_w, usable_h),
            SnapZone::TopLeft => (usable_x, usable_y, hw, hh),
            SnapZone::TopRight => (usable_x + hw, usable_y, usable_w - hw, hh),
            SnapZone::BottomLeft => (usable_x, usable_y + hh, hw, usable_h - hh),
            SnapZone::BottomRight => (usable_x + hw, usable_y + hh, usable_w - hw, usable_h - hh),
        }
    }

    /// Legacy wrapper assuming usable = entire output (no layer shell).
    pub fn layout(self, out_w: i32, out_h: i32) -> (i32, i32, i32, i32) {
        self.layout_usable(0, 0, out_w, out_h)
    }
}

/// Grab active while user drags a window via CSD/SSD header.
pub struct MoveSurfaceGrab {
    pub start_data: GrabStartData<LumoState>,
    pub window: Window,
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
        handle.motion(data, None, event);

        let delta = event.location - self.start_data.location;
        let mut new_loc = self.initial_window_location + delta.to_i32_round();
        // W24: clamp drag dentro usable area (excludes bar Layer::Top).
        // W24.2: SSD titlebar fica acima window.loc.y. y_min += 30 pra titlebar
        // nao invadir bar Layer::Top exclusive zone.
        const SSD_TITLEBAR_H: i32 = 30;
        let usable = data.usable_geometry();
        let win_bbox = self.window.bbox();

        // W24.3: Seguranca contra panics de clamp (min > max).
        // Se a janela for maior que a area util, max_x/max_y ficariam negativos.
        // Forcamos max >= min para evitar crash total do compositor.
        let max_x = (usable.loc.x + usable.size.w - win_bbox.size.w.max(64)).max(usable.loc.x);
        let max_y = (usable.loc.y + usable.size.h - 32).max(usable.loc.y + SSD_TITLEBAR_H);

        new_loc.x = new_loc.x.clamp(usable.loc.x, max_x);
        new_loc.y = new_loc.y.clamp(usable.loc.y + SSD_TITLEBAR_H, max_y);

        data.space.map_element(self.window.clone(), new_loc, true);

        // W9.B: update snap preview.
        let (out_w, out_h) = data
            .space
            .outputs()
            .next()
            .and_then(|o| o.current_mode())
            .map(|m| (m.size.w, m.size.h))
            .unwrap_or((1920, 1080));
        data.snap_preview = SnapZone::detect(event.location, out_w, out_h);

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

        if event.button == self.start_data.button
            && event.state == smithay::backend::input::ButtonState::Released
        {
            if let Some(zone) = data.snap_preview.take() {
                let usable = data.usable_geometry();
                let (sx, sy, sw, sh) =
                    zone.layout_usable(usable.loc.x, usable.loc.y, usable.size.w, usable.size.h);
                if let Some(tl) = self.window.toplevel() {
                    tl.with_pending_state(|state| {
                        state.size = Some(Size::from((sw, sh)));
                    });
                    let _ = tl.send_configure();
                }
                data.space.map_element(
                    self.window.clone(),
                    smithay::utils::Point::<i32, smithay::utils::Logical>::from((sx, sy)),
                    true,
                );
                tracing::info!(?zone, sx, sy, sw, sh, "W9.B: snap applied");
            } else {
                data.snap_preview = None;
            }
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

    fn frame(&mut self, data: &mut LumoState, handle: &mut PointerInnerHandle<'_, LumoState>) {
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

    fn unset(&mut self, data: &mut LumoState) {
        data.snap_preview = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smithay::utils::Point;

    fn pt(x: f64, y: f64) -> Point<f64, Logical> {
        Point::from((x, y))
    }

    #[test]
    fn snap_left_edge() {
        assert_eq!(
            SnapZone::detect(pt(4.0, 540.0), 1920, 1080),
            Some(SnapZone::Left)
        );
    }

    #[test]
    fn snap_right_edge() {
        assert_eq!(
            SnapZone::detect(pt(1916.0, 540.0), 1920, 1080),
            Some(SnapZone::Right)
        );
    }

    #[test]
    fn snap_top_maximize() {
        assert_eq!(
            SnapZone::detect(pt(960.0, 4.0), 1920, 1080),
            Some(SnapZone::Maximize)
        );
    }

    #[test]
    fn snap_top_left_corner() {
        assert_eq!(
            SnapZone::detect(pt(4.0, 4.0), 1920, 1080),
            Some(SnapZone::TopLeft)
        );
    }

    #[test]
    fn snap_top_right_corner() {
        assert_eq!(
            SnapZone::detect(pt(1916.0, 4.0), 1920, 1080),
            Some(SnapZone::TopRight)
        );
    }

    #[test]
    fn snap_bottom_left_corner() {
        assert_eq!(
            SnapZone::detect(pt(4.0, 1076.0), 1920, 1080),
            Some(SnapZone::BottomLeft)
        );
    }

    #[test]
    fn snap_bottom_right_corner() {
        assert_eq!(
            SnapZone::detect(pt(1916.0, 1076.0), 1920, 1080),
            Some(SnapZone::BottomRight)
        );
    }

    #[test]
    fn snap_none_center() {
        assert_eq!(SnapZone::detect(pt(960.0, 540.0), 1920, 1080), None);
    }

    #[test]
    fn snap_left_layout() {
        assert_eq!(SnapZone::Left.layout(1920, 1080), (0, 0, 960, 1080));
    }

    #[test]
    fn snap_maximize_layout() {
        assert_eq!(SnapZone::Maximize.layout(1920, 1080), (0, 0, 1920, 1080));
    }

    #[test]
    fn snap_right_x_and_width() {
        let (x, _, w, _) = SnapZone::Right.layout(1920, 1080);
        assert_eq!(x, 960);
        assert_eq!(w, 960);
    }

    #[test]
    fn snap_top_left_quarter() {
        assert_eq!(SnapZone::TopLeft.layout(1920, 1080), (0, 0, 960, 540));
    }

    #[test]
    fn quarter_areas_fill_screen() {
        let (_, _, w1, h1) = SnapZone::TopLeft.layout(1920, 1080);
        let (_, _, w2, _) = SnapZone::TopRight.layout(1920, 1080);
        let (_, _, _, h3) = SnapZone::BottomLeft.layout(1920, 1080);
        assert_eq!(w1 + w2, 1920);
        assert_eq!(h1 + h3, 1080);
    }

    #[test]
    fn regression_clamp_logic_safety() {
        // Mock data
        let usable_x = 0;
        let usable_w = 1000;
        let win_w = 1052; // Caso do Mousepad bizarro
        
        // A logica que causava panic:
        // let max_x = usable_x + usable_w - win_w; // -> -52
        // target.clamp(usable_x, max_x); // -> panic: 0 > -52
        
        // A logica corrigida:
        let max_x = (usable_x + usable_w - win_w).max(usable_x);
        assert_eq!(max_x, 0); // max deve ser no minimo o proprio min
        
        let target = 500;
        let clamped = target.clamp(usable_x, max_x);
        assert_eq!(clamped, 0); // move para o limite seguro
    }
}
