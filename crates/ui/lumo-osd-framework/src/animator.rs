//! animator.rs — state machine OSD fade-in / hold / fade-out.
//!
//! Phases:
//! 1. FadeIn (0..FADE_IN_MS): alpha 0 -> 1
//! 2. Hold (FADE_IN_MS..FADE_IN_MS+HOLD_MS): alpha = 1
//! 3. FadeOut (..total): alpha 1 -> 0
//! 4. Done: invisible, pode unmap
//!
//! Trigger: caller chama `bump()` cada vez que user mexe (ex: brightness key
//! pressionada multiplas vezes em 1s). Reset Hold phase pra estender visibilidade.
//!
//! Funcao pura testavel sem Wayland.

use std::time::{Duration, Instant};

use crate::tokens;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OsdPhase {
    FadeIn,
    Hold,
    FadeOut,
    Done,
}

#[derive(Debug, Clone)]
pub struct OsdAnimator {
    started_at: Instant,
    fade_in_ms: u32,
    hold_ms: u32,
    fade_out_ms: u32,
}

impl OsdAnimator {
    pub fn new() -> Self {
        Self::with_hold(tokens::HOLD_MS_DEFAULT)
    }

    pub fn with_hold(hold_ms: u32) -> Self {
        Self {
            started_at: Instant::now(),
            fade_in_ms: tokens::FADE_IN_MS,
            hold_ms,
            fade_out_ms: tokens::FADE_OUT_MS,
        }
    }

    /// Bump = user re-disparou OSD. Reset to FadeIn fim + Hold restart.
    /// Garante OSD permanece visivel enquanto user spam tecla brightness etc.
    pub fn bump(&mut self) {
        let now = Instant::now();
        // Se ja passou FadeIn, mantem alpha em Hold. Se ainda FadeIn, mantem.
        // Estrategia simples: reset clock pro inicio do Hold (pula fade-in
        // se ja estava visivel).
        let phase = self.phase_at(now);
        match phase {
            OsdPhase::FadeIn | OsdPhase::Hold => {
                // Recoloca clock no inicio do Hold pra dar full hold de novo.
                self.started_at = now
                    .checked_sub(Duration::from_millis(self.fade_in_ms as u64))
                    .unwrap_or(now);
            }
            OsdPhase::FadeOut | OsdPhase::Done => {
                // Reinicia do zero.
                self.started_at = now;
            }
        }
    }

    pub fn phase(&self) -> OsdPhase {
        self.phase_at(Instant::now())
    }

    pub fn phase_at(&self, now: Instant) -> OsdPhase {
        let elapsed_ms = now.duration_since(self.started_at).as_millis() as u64;
        let fade_in_end = self.fade_in_ms as u64;
        let hold_end = fade_in_end + self.hold_ms as u64;
        let total = hold_end + self.fade_out_ms as u64;
        if elapsed_ms < fade_in_end {
            OsdPhase::FadeIn
        } else if elapsed_ms < hold_end {
            OsdPhase::Hold
        } else if elapsed_ms < total {
            OsdPhase::FadeOut
        } else {
            OsdPhase::Done
        }
    }

    /// Retorna alpha 0.0-1.0 baseado em phase atual.
    pub fn alpha(&self) -> f32 {
        self.alpha_at(Instant::now())
    }

    pub fn alpha_at(&self, now: Instant) -> f32 {
        let elapsed_ms = now.duration_since(self.started_at).as_millis() as u64;
        let fade_in_end = self.fade_in_ms as u64;
        let hold_end = fade_in_end + self.hold_ms as u64;
        let total = hold_end + self.fade_out_ms as u64;
        if elapsed_ms < fade_in_end {
            (elapsed_ms as f32 / self.fade_in_ms as f32).clamp(0.0, 1.0)
        } else if elapsed_ms < hold_end {
            1.0
        } else if elapsed_ms < total {
            let fade_progress = (elapsed_ms - hold_end) as f32 / self.fade_out_ms as f32;
            (1.0 - fade_progress).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }

    pub fn is_done(&self) -> bool {
        self.phase() == OsdPhase::Done
    }

    pub fn next_repaint(&self) -> Option<Duration> {
        if self.is_done() {
            return None;
        }
        Some(Duration::from_millis(16)) // ~60fps
    }
}

impl Default for OsdAnimator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anim_at(animator: &OsdAnimator, offset_ms: u64) -> (OsdPhase, f32) {
        let probe = animator.started_at + Duration::from_millis(offset_ms);
        (animator.phase_at(probe), animator.alpha_at(probe))
    }

    #[test]
    fn phase_starts_fade_in() {
        let a = OsdAnimator::new();
        let (phase, alpha) = anim_at(&a, 0);
        assert_eq!(phase, OsdPhase::FadeIn);
        assert!(alpha < 0.01);
    }

    #[test]
    fn phase_at_fade_in_mid() {
        let a = OsdAnimator::new();
        let (phase, alpha) = anim_at(&a, 75); // metade de 150ms
        assert_eq!(phase, OsdPhase::FadeIn);
        assert!((alpha - 0.5).abs() < 0.1);
    }

    #[test]
    fn phase_hold_alpha_one() {
        let a = OsdAnimator::new();
        let (phase, alpha) = anim_at(&a, 500); // 500ms = dentro Hold
        assert_eq!(phase, OsdPhase::Hold);
        assert!((alpha - 1.0).abs() < 0.001);
    }

    #[test]
    fn phase_fade_out_alpha_decreases() {
        let a = OsdAnimator::new();
        let total_pre_fade = (tokens::FADE_IN_MS + tokens::HOLD_MS_DEFAULT) as u64;
        let (phase, alpha) = anim_at(&a, total_pre_fade + 100);
        assert_eq!(phase, OsdPhase::FadeOut);
        assert!(alpha < 0.6 && alpha > 0.3);
    }

    #[test]
    fn phase_done_after_total() {
        let a = OsdAnimator::new();
        let total = (tokens::FADE_IN_MS + tokens::HOLD_MS_DEFAULT + tokens::FADE_OUT_MS) as u64;
        let (phase, alpha) = anim_at(&a, total + 100);
        assert_eq!(phase, OsdPhase::Done);
        assert_eq!(alpha, 0.0);
    }

    #[test]
    fn bump_during_hold_extends_visibility() {
        let mut a = OsdAnimator::new();
        // Avanca clock pra meio do Hold via simular passagem de tempo.
        a.started_at = Instant::now() - Duration::from_millis(500);
        // Bump = restart hold.
        a.bump();
        assert_eq!(a.phase(), OsdPhase::Hold);
    }

    #[test]
    fn bump_after_done_restarts_fade_in() {
        let mut a = OsdAnimator::new();
        a.started_at = Instant::now()
            - Duration::from_millis(
                (tokens::FADE_IN_MS + tokens::HOLD_MS_DEFAULT + tokens::FADE_OUT_MS + 100) as u64,
            );
        assert_eq!(a.phase(), OsdPhase::Done);
        a.bump();
        assert_eq!(a.phase(), OsdPhase::FadeIn);
    }

    #[test]
    fn alpha_clamped_zero_to_one() {
        let a = OsdAnimator::new();
        for ms in [0, 50, 150, 500, 1900, 2050, 5000] {
            let (_, alpha) = anim_at(&a, ms);
            assert!(alpha >= 0.0 && alpha <= 1.0, "alpha={} fora de [0,1]", alpha);
        }
    }

    #[test]
    fn custom_hold_duration() {
        let a = OsdAnimator::with_hold(500);
        let total_pre_fade = (tokens::FADE_IN_MS + 500) as u64;
        let (phase, _) = anim_at(&a, total_pre_fade - 50);
        assert_eq!(phase, OsdPhase::Hold);
        let (phase, _) = anim_at(&a, total_pre_fade + 10);
        assert_eq!(phase, OsdPhase::FadeOut);
    }

    #[test]
    fn next_repaint_none_when_done() {
        let mut a = OsdAnimator::new();
        a.started_at = Instant::now() - Duration::from_secs(10);
        assert!(a.next_repaint().is_none());
    }

    #[test]
    fn next_repaint_some_when_active() {
        let a = OsdAnimator::new();
        assert!(a.next_repaint().is_some());
    }
}
