//! lock_state.rs — state machine pra Caps/Num/Scroll Lock.
//!
//! Logica pura: detecta transitions (ON -> OFF, OFF -> ON) e
//! decide quando spawnar OSD. Sem hardware. Testavel.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockKind {
    Caps,
    Num,
    Scroll,
}

impl LockKind {
    pub fn label(self) -> &'static str {
        match self {
            LockKind::Caps => "Caps Lock",
            LockKind::Num => "Num Lock",
            LockKind::Scroll => "Scroll Lock",
        }
    }

    pub fn sysfs_pattern(self) -> &'static str {
        match self {
            LockKind::Caps => "capslock",
            LockKind::Num => "numlock",
            LockKind::Scroll => "scrolllock",
        }
    }
}

/// State atual dos 3 locks. Apenas booleans + funcoes puras.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LockState {
    pub caps: bool,
    pub num: bool,
    pub scroll: bool,
}

impl LockState {
    pub fn get(&self, kind: LockKind) -> bool {
        match kind {
            LockKind::Caps => self.caps,
            LockKind::Num => self.num,
            LockKind::Scroll => self.scroll,
        }
    }

    pub fn set(&mut self, kind: LockKind, on: bool) {
        match kind {
            LockKind::Caps => self.caps = on,
            LockKind::Num => self.num = on,
            LockKind::Scroll => self.scroll = on,
        }
    }
}

/// Diff entre 2 estados retorna Vec<(LockKind, novo_estado)> pra cada
/// transition. Vazio se nao mudou nada.
pub fn diff(prev: &LockState, next: &LockState) -> Vec<(LockKind, bool)> {
    let mut out = Vec::new();
    if prev.caps != next.caps {
        out.push((LockKind::Caps, next.caps));
    }
    if prev.num != next.num {
        out.push((LockKind::Num, next.num));
    }
    if prev.scroll != next.scroll {
        out.push((LockKind::Scroll, next.scroll));
    }
    out
}

/// Decide se OSD deve abrir baseado em transitions.
/// Estrategia: cada transition = 1 OSD. Apenas 1 OSD ativo por vez
/// (caller mantem queue ou cancela anterior).
pub fn should_show_osd(transitions: &[(LockKind, bool)]) -> Option<(LockKind, bool)> {
    // Prioridade: caps > num > scroll (maioria user importa caps).
    for kind in [LockKind::Caps, LockKind::Num, LockKind::Scroll] {
        if let Some(&(_, on)) = transitions.iter().find(|(k, _)| *k == kind) {
            return Some((kind, on));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_state_all_off() {
        let s = LockState::default();
        assert!(!s.caps);
        assert!(!s.num);
        assert!(!s.scroll);
    }

    #[test]
    fn get_set_caps() {
        let mut s = LockState::default();
        s.set(LockKind::Caps, true);
        assert!(s.get(LockKind::Caps));
        assert!(!s.get(LockKind::Num));
    }

    #[test]
    fn get_set_num() {
        let mut s = LockState::default();
        s.set(LockKind::Num, true);
        assert!(s.get(LockKind::Num));
    }

    #[test]
    fn get_set_scroll() {
        let mut s = LockState::default();
        s.set(LockKind::Scroll, true);
        assert!(s.get(LockKind::Scroll));
    }

    #[test]
    fn diff_no_change_empty() {
        let s = LockState::default();
        assert!(diff(&s, &s).is_empty());
    }

    #[test]
    fn diff_caps_on_one_transition() {
        let prev = LockState::default();
        let next = LockState {
            caps: true,
            ..Default::default()
        };
        let d = diff(&prev, &next);
        assert_eq!(d, vec![(LockKind::Caps, true)]);
    }

    #[test]
    fn diff_multiple_changes() {
        let prev = LockState::default();
        let next = LockState {
            caps: true,
            num: true,
            scroll: true,
        };
        let d = diff(&prev, &next);
        assert_eq!(d.len(), 3);
    }

    #[test]
    fn diff_caps_off_transition() {
        let prev = LockState {
            caps: true,
            ..Default::default()
        };
        let next = LockState::default();
        let d = diff(&prev, &next);
        assert_eq!(d, vec![(LockKind::Caps, false)]);
    }

    #[test]
    fn should_show_osd_caps_priority() {
        let trans = vec![(LockKind::Num, true), (LockKind::Caps, true)];
        let osd = should_show_osd(&trans);
        assert_eq!(osd, Some((LockKind::Caps, true)));
    }

    #[test]
    fn should_show_osd_num_only() {
        let trans = vec![(LockKind::Num, false)];
        assert_eq!(should_show_osd(&trans), Some((LockKind::Num, false)));
    }

    #[test]
    fn should_show_osd_empty_none() {
        assert!(should_show_osd(&[]).is_none());
    }

    #[test]
    fn lock_kind_label_correct() {
        assert_eq!(LockKind::Caps.label(), "Caps Lock");
        assert_eq!(LockKind::Num.label(), "Num Lock");
        assert_eq!(LockKind::Scroll.label(), "Scroll Lock");
    }

    #[test]
    fn lock_kind_sysfs_pattern() {
        assert_eq!(LockKind::Caps.sysfs_pattern(), "capslock");
        assert_eq!(LockKind::Num.sysfs_pattern(), "numlock");
        assert_eq!(LockKind::Scroll.sysfs_pattern(), "scrolllock");
    }
}
