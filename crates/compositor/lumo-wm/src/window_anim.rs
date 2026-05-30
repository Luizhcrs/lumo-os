//! W9.A: Window open/close spring animation.
//!
//! Each toplevel gets a WindowAnimState tracking open/close progress.
//! Spring physics: mass=1, stiffness=170, damping=22 (LASpring preset).
//! Render: scale 0.9->1.0 + alpha 0->1 on open; reverse on close.
//! A11y: reduced_motion=true -> instant (skip spring, jump to done).

use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use std::collections::HashMap;

const SPRING_MASS: f32 = 1.0;
// W38: curva RAPIDA (anti "demora" do feedback W32.4). Stiffness alto + bem
// amortecido = pop vivo que assenta em ~150ms. Hard-cap de 180ms garante que
// nunca arrasta mesmo com dt irregular.
const SPRING_STIFFNESS: f32 = 550.0;
const SPRING_DAMPING: f32 = 42.0;
/// Duracao maxima absoluta (s). Passou disso -> forca done (sem "demora").
const MAX_ANIM_S: f32 = 0.18;

/// Per-window animation state.
#[derive(Debug, Clone)]
pub enum WindowAnimState {
    Opening { progress: f32, velocity: f32, elapsed: f32 },
    Closing { progress: f32, velocity: f32, elapsed: f32 },
    /// W38: minimizar -- encolhe+fade in-place (progress 1->0). A janela fica
    /// mapeada/viva durante a anim; ao terminar (MinimizeDone) e desmapeada +
    /// guardada em minimized_windows pra restaurar (sem use-after-free).
    Minimizing { progress: f32, velocity: f32, elapsed: f32 },
    Idle,
    CloseDone,
    MinimizeDone,
}

impl WindowAnimState {
    pub fn new_opening(reduced_motion: bool) -> Self {
        // W38: reativada (curva rapida + hard-cap). reduced_motion = instant.
        if reduced_motion {
            WindowAnimState::Idle
        } else {
            WindowAnimState::Opening { progress: 0.0, velocity: 0.0, elapsed: 0.0 }
        }
    }

    pub fn new_closing(reduced_motion: bool) -> Self {
        if reduced_motion {
            WindowAnimState::CloseDone
        } else {
            WindowAnimState::Closing { progress: 1.0, velocity: 0.0, elapsed: 0.0 }
        }
    }

    pub fn new_minimizing(reduced_motion: bool) -> Self {
        if reduced_motion {
            WindowAnimState::MinimizeDone
        } else {
            WindowAnimState::Minimizing { progress: 1.0, velocity: 0.0, elapsed: 0.0 }
        }
    }

    pub fn tick(&mut self, dt: f32) -> bool {
        match self {
            WindowAnimState::Opening { progress, velocity, elapsed } => {
                spring_step(progress, velocity, 1.0, dt);
                *elapsed += dt;
                if *progress >= 0.97 || *elapsed >= MAX_ANIM_S {
                    *self = WindowAnimState::Idle;
                    return true;
                }
            }
            WindowAnimState::Closing { progress, velocity, elapsed } => {
                spring_step(progress, velocity, 0.0, dt);
                *elapsed += dt;
                if *progress <= 0.03 || *elapsed >= MAX_ANIM_S {
                    *progress = 0.0;
                    *self = WindowAnimState::CloseDone;
                    return true;
                }
            }
            WindowAnimState::Minimizing { progress, velocity, elapsed } => {
                spring_step(progress, velocity, 0.0, dt);
                *elapsed += dt;
                if *progress <= 0.05 || *elapsed >= MAX_ANIM_S {
                    *progress = 0.0;
                    *self = WindowAnimState::MinimizeDone;
                    return true;
                }
            }
            WindowAnimState::Idle
            | WindowAnimState::CloseDone
            | WindowAnimState::MinimizeDone => return true,
        }
        false
    }

    pub fn visual_progress(&self) -> f32 {
        match self {
            WindowAnimState::Opening { progress, .. } => *progress,
            WindowAnimState::Closing { progress, .. } => *progress,
            WindowAnimState::Minimizing { progress, .. } => *progress,
            WindowAnimState::Idle => 1.0,
            WindowAnimState::CloseDone | WindowAnimState::MinimizeDone => 0.0,
        }
    }

    /// Scale: open 0.9..1.0; minimize encolhe mais (1.0..0.25) pra leitura de
    /// "sumindo". Demais = 0.9+0.1*progress.
    pub fn scale(&self) -> f32 {
        match self {
            WindowAnimState::Minimizing { progress, .. } => 0.25 + 0.75 * progress,
            _ => 0.9 + 0.1 * self.visual_progress(),
        }
    }
    pub fn alpha(&self) -> f32 {
        self.visual_progress()
    }
    pub fn is_close_done(&self) -> bool {
        matches!(self, WindowAnimState::CloseDone)
    }
    pub fn is_minimize_done(&self) -> bool {
        matches!(self, WindowAnimState::MinimizeDone)
    }
    pub fn is_animating(&self) -> bool {
        matches!(
            self,
            WindowAnimState::Opening { .. }
                | WindowAnimState::Closing { .. }
                | WindowAnimState::Minimizing { .. }
        )
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
    pub fn new() -> Self {
        Self::default()
    }

    fn key(surface: &WlSurface) -> u32 {
        use smithay::reexports::wayland_server::Resource;
        surface.id().protocol_id()
    }

    pub fn insert_opening(&mut self, surface: &WlSurface, reduced_motion: bool) {
        self.states.insert(
            Self::key(surface),
            WindowAnimState::new_opening(reduced_motion),
        );
    }

    pub fn insert_closing(&mut self, surface: &WlSurface, reduced_motion: bool) {
        self.states.insert(
            Self::key(surface),
            WindowAnimState::new_closing(reduced_motion),
        );
    }

    pub fn insert_minimizing(&mut self, surface: &WlSurface, reduced_motion: bool) {
        self.states.insert(
            Self::key(surface),
            WindowAnimState::new_minimizing(reduced_motion),
        );
    }

    /// W38: drena (remove + retorna) as janelas que terminaram de minimizar.
    /// O caller faz o unmap + guarda em minimized_windows pra cada id.
    pub fn drain_minimize_done(&mut self) -> Vec<u32> {
        let done: Vec<u32> = self
            .states
            .iter()
            .filter(|(_, s)| s.is_minimize_done())
            .map(|(k, _)| *k)
            .collect();
        for k in &done {
            self.states.remove(k);
        }
        done
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
        self.states
            .iter()
            .filter(|(_, s)| s.is_close_done())
            .map(|(k, _)| *k)
            .collect()
    }

    /// W38: true se alguma janela esta animando (Opening/Closing). Usado pra
    /// (a) decidir tickar + forcar repaint, (b) manter o adaptive timer rapido.
    pub fn is_active(&self) -> bool {
        self.states.values().any(|s| s.is_animating())
    }

    /// W38: remove estados ja assentados (Idle/CloseDone) -- evita acumular
    /// entradas mortas no registry. Chamado apos tick_all.
    pub fn prune_settled(&mut self) {
        self.states
            .retain(|_, s| matches!(s, WindowAnimState::Opening { .. } | WindowAnimState::Closing { .. }));
    }

    pub fn drain_close_done(&mut self) -> Vec<u32> {
        let done: Vec<u32> = self
            .states
            .iter()
            .filter(|(_, s)| s.is_close_done())
            .map(|(k, _)| *k)
            .collect();
        for k in &done {
            self.states.remove(k);
        }
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

    // W38: animacao REATIVADA (curva rapida + hard-cap). Sem reduced_motion,
    // open comeca animando em progress 0; close em progress 1.
    #[test]
    fn opening_starts_animating() {
        let s = WindowAnimState::new_opening(false);
        assert!(matches!(s, WindowAnimState::Opening { .. }));
        assert!(s.is_animating());
        assert!(s.visual_progress() < 0.1);
    }

    #[test]
    fn closing_starts_animating() {
        let s = WindowAnimState::new_closing(false);
        assert!(matches!(s, WindowAnimState::Closing { .. }));
        assert!(s.is_animating());
        assert!(s.visual_progress() > 0.9);
    }

    // W38: hard-cap 180ms -- nunca arrasta (anti "demora").
    #[test]
    fn anim_never_exceeds_180ms() {
        let mut s = WindowAnimState::new_opening(false);
        let mut t = 0.0f32;
        let mut done = false;
        // dt irregular 16ms; deve terminar em <= ~180ms (12 ticks).
        for _ in 0..20 {
            if s.tick(0.016) {
                done = true;
                break;
            }
            t += 0.016;
        }
        assert!(done, "anim nao terminou");
        assert!(t <= 0.18 + 0.016, "anim passou de 180ms: {t}");
    }

    #[test]
    fn opening_scale_range() {
        let mut s = WindowAnimState::new_opening(false);
        let init = s.scale();
        assert!(init >= 0.9 && init <= 1.0, "scale={init}");
        for _ in 0..300 {
            s.tick(0.016);
        }
        assert!((s.scale() - 1.0).abs() < 0.01);
    }

    #[test]
    fn spring_converges_to_target() {
        let mut pos = 0.0f32;
        let mut vel = 0.0f32;
        for _ in 0..500 {
            spring_step(&mut pos, &mut vel, 1.0, 0.016);
        }
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
