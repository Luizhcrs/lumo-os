//! dispatch.rs — A4: prioridade entre eventos OSD multi-source.
//!
//! Quando varios sources detectam mudanca no mesmo tick (raro mas possivel —
//! ex: bind do compositor sobe brilho + da unmute simultaneo), so 1 OSD aparece.
//! Prioridade: Lock > Brightness > Volume (matches Mac UX: locks sao raros
//! e usuario espera feedback imediato; brightness e volume tem ja-saw-it).

use crate::sources::backlight::BacklightState;
use crate::sources::lock_state::LockKind;
use crate::sources::pactl_parse::VolumeState;

/// Evento OSD pronto pra render.
#[derive(Debug, Clone, PartialEq)]
pub enum OsdEvent {
    Lock { kind: LockKind, on: bool },
    Brightness(BacklightState),
    Volume(VolumeState),
}

impl OsdEvent {
    pub fn priority(&self) -> u8 {
        match self {
            OsdEvent::Lock { .. } => 0,        // mais alta
            OsdEvent::Brightness(_) => 1,
            OsdEvent::Volume(_) => 2,
        }
    }
}

/// Escolhe o evento de maior prioridade entre os candidates (menor numero = maior).
pub fn pick_event(candidates: Vec<OsdEvent>) -> Option<OsdEvent> {
    candidates.into_iter().min_by_key(|e| e.priority())
}

/// Diff threshold pra brilho (1%).
pub const BRIGHTNESS_THRESHOLD_PCT: f32 = 1.0;
/// Diff threshold pra volume (1 ponto pct).
pub const VOLUME_THRESHOLD_PCT: u32 = 1;

pub fn brightness_should_show(prev: Option<BacklightState>, next: Option<BacklightState>) -> bool {
    match (prev, next) {
        (None, Some(_)) => true,
        (Some(p), Some(n)) => (p.pct() - n.pct()).abs() > BRIGHTNESS_THRESHOLD_PCT,
        _ => false,
    }
}

pub fn volume_should_show(prev: Option<VolumeState>, next: Option<VolumeState>) -> bool {
    match (prev, next) {
        (None, Some(_)) => true,
        (Some(p), Some(n)) => p.muted != n.muted || diff_u32(p.pct, n.pct) > VOLUME_THRESHOLD_PCT,
        _ => false,
    }
}

pub fn diff_u32(a: u32, b: u32) -> u32 {
    if a > b {
        a - b
    } else {
        b - a
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b(c: u32, m: u32) -> BacklightState {
        BacklightState { current: c, max: m }
    }
    fn v(pct: u32, muted: bool) -> VolumeState {
        VolumeState { pct, muted }
    }

    // pick_event priority
    #[test]
    fn pick_event_returns_none_when_empty() {
        assert!(pick_event(vec![]).is_none());
    }

    #[test]
    fn pick_event_single() {
        let e = OsdEvent::Volume(v(50, false));
        assert_eq!(pick_event(vec![e.clone()]), Some(e));
    }

    #[test]
    fn pick_event_lock_over_brightness() {
        let lock = OsdEvent::Lock {
            kind: LockKind::Caps,
            on: true,
        };
        let bri = OsdEvent::Brightness(b(50, 100));
        let r = pick_event(vec![bri, lock.clone()]);
        assert_eq!(r, Some(lock));
    }

    #[test]
    fn pick_event_lock_over_volume() {
        let lock = OsdEvent::Lock {
            kind: LockKind::Num,
            on: false,
        };
        let vol = OsdEvent::Volume(v(30, true));
        let r = pick_event(vec![vol, lock.clone()]);
        assert_eq!(r, Some(lock));
    }

    #[test]
    fn pick_event_brightness_over_volume() {
        let bri = OsdEvent::Brightness(b(50, 100));
        let vol = OsdEvent::Volume(v(30, true));
        let r = pick_event(vec![vol, bri.clone()]);
        assert_eq!(r, Some(bri));
    }

    // brightness_should_show
    #[test]
    fn brightness_first_read_shows() {
        assert!(brightness_should_show(None, Some(b(50, 100))));
    }

    #[test]
    fn brightness_significant_change_shows() {
        assert!(brightness_should_show(Some(b(50, 100)), Some(b(70, 100))));
    }

    #[test]
    fn brightness_tiny_change_hides() {
        assert!(!brightness_should_show(
            Some(b(500, 1000)),
            Some(b(505, 1000))
        ));
    }

    #[test]
    fn brightness_unchanged_hides() {
        assert!(!brightness_should_show(Some(b(50, 100)), Some(b(50, 100))));
    }

    #[test]
    fn brightness_disappear_hides() {
        assert!(!brightness_should_show(Some(b(50, 100)), None));
    }

    #[test]
    fn brightness_decrease_shows() {
        assert!(brightness_should_show(Some(b(80, 100)), Some(b(60, 100))));
    }

    // volume_should_show
    #[test]
    fn volume_first_read_shows() {
        assert!(volume_should_show(None, Some(v(50, false))));
    }

    #[test]
    fn volume_mute_toggle_shows() {
        assert!(volume_should_show(Some(v(50, false)), Some(v(50, true))));
    }

    #[test]
    fn volume_increase_shows() {
        assert!(volume_should_show(Some(v(50, false)), Some(v(60, false))));
    }

    #[test]
    fn volume_unchanged_hides() {
        assert!(!volume_should_show(
            Some(v(50, false)),
            Some(v(50, false))
        ));
    }

    #[test]
    fn volume_tiny_change_hides() {
        assert!(!volume_should_show(Some(v(50, false)), Some(v(51, false))));
    }

    #[test]
    fn diff_u32_basic() {
        assert_eq!(diff_u32(10, 5), 5);
        assert_eq!(diff_u32(5, 10), 5);
        assert_eq!(diff_u32(0, 0), 0);
    }

    #[test]
    fn osd_event_priority_values() {
        assert_eq!(
            OsdEvent::Lock {
                kind: LockKind::Caps,
                on: true
            }
            .priority(),
            0
        );
        assert_eq!(OsdEvent::Brightness(b(0, 100)).priority(), 1);
        assert_eq!(OsdEvent::Volume(v(0, false)).priority(), 2);
    }
}
