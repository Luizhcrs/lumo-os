//! W9.A: Window open/close spring animation.
//!
//! Each toplevel gets a WindowAnimState tracking open/close progress.
//! Spring physics: mass=1, stiffness=170, damping=22 (LASpring preset).
//! Render: scale 0.9->1.0 + alpha 0->1 on open; reverse on close.
//! A11y: reduced_motion=true -> instant (skip spring, jump to done).

use std::collections::HashMap;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

const SPRING_MASS: f32      = 1.0;
const SPRING_STIFFNESS: f32 = 170.0;
const SPRING_DAMPING: f32   = 22.0;

/// Per-window animation state.
#[derive(Debug, Clone)]
pub enum WindowAnimState {
    Opening { progress: f32, velocity: f32 },
    Closing { progress: f32, velocity: f32 },
    Idle,
    CloseDone,
}

impl WindowAnimState {
    pub fn new_opening(reduced_motion: bool) -> Self {
        if reduced_motion { return WindowAnimState::Idle; }
        WindowAnimState::Opening { progress: 0.0, velocity: 0.0 }
    }

    pub fn new_closing(reduced_motion: bool) -> Self {
        if reduced_motion { return WindowAnimState::CloseDone; }
        WindowAnimState::Closing { progress: 1.0, velocity: 0.0 }
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        match self {
            WindowAnimState::Opening { progress, velocity } => {
                spring_step(progress, velocity, 1.0, dt);
                if *progress >= 0.998 {
                    *progress = 1.0;
                    *self = WindowAnimState::Idle;
                    return true;
                }
            }
            WindowAnimState::Closing { progress, velocity } => {
                spring_step(progress, velocity, 0.0, dt);
                if *progress <= 0.002 {
                    *progress = 0.0;
                    *self = WindowAnimState::CloseDone;
                    return true;
                }
            }
            WindowAnimState::Idle | WindowAnimState::CloseDone => return true,
        }
        false
    }

    pub fn visual_progress(&self) -> f32 {
        match self {
            WindowAnimState::Opening { progress, .. } => *progress,
            WindowAnimState::Closing { progress, .. } => *progress,
            WindowAnimState::Idle => 1.0,
            WindowAnimState::CloseDone => 0.0,
        }
    }

    /// Scale lerp 0.9..1.0
    pub fn scale(&self) -> f32 { 0.9 + 0.1 * self.visual_progress() }
    pub fn alpha(&self) -> f32 { self.visual_progress() }
    pub fn is_close_done(&self) -> bool { matches!(self, WindowAnimState::CloseDone) }
    pub fn is_animating(&self) -> bool {
        matches!(self, WindowAnimState::Opening { .. } | WindowAnimState::Closing { .. })
    }
}

fn spring_step(pos: &mut f32, vel: &mut f32, target: f32, dt: f32) {
    let dt = dt.min(0.05);
    let displacement = *pos - target;
    let accel = (-SPRING_STIFFNESS * displacement - SPRING_DAMPING * *vel) / SPRING_MASS;
    *vel += accel * dt;
    *pos += *vel * dt;
    *pos = pos.clamp(0.0, 1.2);
}

/// Registry mapping surface protocol_id -> WindowAnimState.
#[derive(Default)]
pub struct WindowAnimRegistry {
    pub states: HashMap<u32, WindowAnimState>,
}

impl WindowAnimRegistry {
    pub fn new() -> Self { Self::default() }

    fn key(surface: &WlSurface) -> u32 {
        use smithay::reexports::wayland_server::Resource;
        surface.id().protocol_id()
    }

    pub fn insert_opening(&mut self, surface: &WlSurface, reduced_motion: bool) {
        self.states.insert(Self::key(surface), WindowAnimState::new_opening(reduced_motion));
    }

    pub fn insert_closing(&mut self, surface: &WlSurface, reduced_motion: bool) {
        self.states.insert(Self::key(surface), WindowAnimState::new_closing(reduced_motion));
    }

    pub fn get(&self, surface: &WlSurface) -> Option<&WindowAnimState> {
        self.states.get(&Self::key(surface))
    }

    pub fn remove(&mut self, surface: &WlSurface) {
        self.states.remove(&Self::key(surface));
    }

    pub fn tick_all(&mut self, dt: f32) -> Vec<u32> {
        for state in self.states.values_mut() {
            state.tick(dt);
        }
        self.states.iter()
            .filter(|(_, s)| s.is_close_done())
            .map(|(k, _)| *k)
            .collect()
    }

    pub fn drain_close_done(&mut self) -> Vec<u32> {
        let done: Vec<u32> = self.states.iter()
            .filter(|(_, s)| s.is_close_done())
            .map(|(k, _)| *k)
            .collect();
        for k in &done { self.states.remove(k); }
        done
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn opening_reduced_motion_instant() {
        let s = WindowAnimState::new_opening(true);
        assert!(matches!(s, WindowAnimState::Idle));
        assert_eq!(s.visual_progress(), 1.0);
        assert_eq!(s.scale(), 1.0);
        assert_eq!(s.alpha(), 1.0);
    }

    #[test]
    fn closing_reduced_motion_instant() {
        let s = WindowAnimState::new_closing(true);
        assert!(s.is_close_done());
        assert_eq!(s.visual_progress(), 0.0);
    }

    #[test]
    fn opening_animates_to_idle() {
        let mut s = WindowAnimState::new_opening(false);
        assert!(s.is_animating());
        for _ in 0..300 { s.tick(0.016); }
        assert!(matches!(s, WindowAnimState::Idle), "expected Idle, got {:?}", s);
    }

    #[test]
    fn closing_animates_to_done() {
        let mut s = WindowAnimState::new_closing(false);
        assert!(s.is_animating());
        for _ in 0..300 { s.tick(0.016); }
        assert!(s.is_close_done());
    }

    #[test]
    fn opening_scale_range() {
        let mut s = WindowAnimState::new_opening(false);
        let init = s.scale();
        assert!(init >= 0.9 && init <= 1.0, "scale={init}");
        for _ in 0..300 { s.tick(0.016); }
        assert!((s.scale() - 1.0).abs() < 0.01);
    }

    #[test]
    fn spring_converges_to_target() {
        let mut pos = 0.0f32;
        let mut vel = 0.0f32;
        for _ in 0..500 { spring_step(&mut pos, &mut vel, 1.0, 0.016); }
        assert!((pos - 1.0).abs() < 0.01, "pos={pos}");
    }

    #[test]
    fn idle_not_animating() {
        let s = WindowAnimState::Idle;
        assert!(!s.is_animating());
        assert!(!s.is_close_done());
    }

    #[test]
    fn close_done_flags() {
        let s = WindowAnimState::CloseDone;
        assert!(!s.is_animating());
        assert!(s.is_close_done());
    }

    #[test]
    fn registry_initially_empty() {
        let reg = WindowAnimRegistry::new();
        assert!(reg.states.is_empty());
    }

    #[test]
    fn tick_all_returns_close_done_keys() {
        let mut reg = WindowAnimRegistry::new();
        reg.states.insert(99, WindowAnimState::CloseDone);
        let done = reg.tick_all(0.016);
        assert!(done.contains(&99));
    }
}
