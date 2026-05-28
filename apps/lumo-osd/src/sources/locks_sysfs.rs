//! sysfs.rs — leitura de /sys/class/leds/input*::{capslock,numlock,scrolllock}
//!
//! Path pattern (Linux kernel):
//!   /sys/class/leds/input{N}::capslock/brightness
//!   /sys/class/leds/input{N}::numlock/brightness
//!   /sys/class/leds/input{N}::scrolllock/brightness
//!
//! brightness = "0" (off) ou "1" (on).
//!
//! Tests usam tmpdir custom em vez de /sys real.

use super::lock_state::{LockKind, LockState};
use std::fs;
use std::path::{Path, PathBuf};

pub fn default_leds_root() -> PathBuf {
    PathBuf::from("/sys/class/leds")
}

/// Le state de 1 lock dado dir do led. None se nao existe.
pub fn read_lock(dir: &Path) -> Option<bool> {
    let path = dir.join("brightness");
    let raw = fs::read_to_string(&path).ok()?;
    let trimmed = raw.trim();
    match trimmed {
        "0" => Some(false),
        _ => Some(trimmed != "0"),
    }
}

/// Varre dir root + retorna LockState agregado. Multi-keyboard: OR
/// dos estados (qualquer kb com caps on → caps state on).
pub fn read_all(root: &Path) -> LockState {
    let mut state = LockState::default();
    let Ok(entries) = fs::read_dir(root) else {
        return state;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        for kind in [LockKind::Caps, LockKind::Num, LockKind::Scroll] {
            if name.contains(kind.sysfs_pattern()) {
                if let Some(on) = read_lock(&entry.path()) {
                    // OR aggregation: qualquer led ON = state ON.
                    let cur = state.get(kind);
                    state.set(kind, cur || on);
                }
            }
        }
    }
    state
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn mk_led(root: &Path, name: &str, brightness: &str) {
        let dir = root.join(name);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("brightness"), brightness).unwrap();
    }

    #[test]
    fn read_lock_zero_returns_false() {
        let t = lumo_testkit::tempdir();
        mk_led(t.path(), "k0_capslock", "0");
        assert_eq!(read_lock(&t.path().join("k0_capslock")), Some(false));
    }

    #[test]
    fn read_lock_one_returns_true() {
        let t = lumo_testkit::tempdir();
        mk_led(t.path(), "k0_capslock", "1");
        assert_eq!(read_lock(&t.path().join("k0_capslock")), Some(true));
    }

    #[test]
    fn read_lock_nonzero_returns_true() {
        let t = lumo_testkit::tempdir();
        mk_led(t.path(), "k0_capslock", "2");
        assert_eq!(read_lock(&t.path().join("k0_capslock")), Some(true));
    }

    #[test]
    fn read_lock_missing_returns_none() {
        let t = lumo_testkit::tempdir();
        assert_eq!(read_lock(&t.path().join("does-not-exist")), None);
    }

    #[test]
    fn read_all_empty_dir_default_state() {
        let t = lumo_testkit::tempdir();
        assert_eq!(read_all(t.path()), LockState::default());
    }

    #[test]
    fn read_all_caps_on() {
        let t = lumo_testkit::tempdir();
        mk_led(t.path(), "k0_capslock", "1");
        let s = read_all(t.path());
        assert!(s.caps);
        assert!(!s.num);
        assert!(!s.scroll);
    }

    #[test]
    fn read_all_multi_keyboard_or_aggregation() {
        let t = lumo_testkit::tempdir();
        mk_led(t.path(), "k0_capslock", "0");
        mk_led(t.path(), "k1_capslock", "1");
        assert!(read_all(t.path()).caps);
    }

    #[test]
    fn read_all_all_three_locks() {
        let t = lumo_testkit::tempdir();
        mk_led(t.path(), "k0_capslock", "1");
        mk_led(t.path(), "k0_numlock", "1");
        mk_led(t.path(), "k0_scrolllock", "1");
        let s = read_all(t.path());
        assert!(s.caps && s.num && s.scroll);
    }

    #[test]
    fn read_all_ignores_unrelated_leds() {
        let t = lumo_testkit::tempdir();
        mk_led(t.path(), "k0_wlan-led", "1");
        mk_led(t.path(), "k0_backlight", "1");
        assert_eq!(read_all(t.path()), LockState::default());
    }

    #[test]
    fn read_all_missing_root_returns_default() {
        let s = read_all(&PathBuf::from("/this/does/not/exist/anywhere"));
        assert_eq!(s, LockState::default());
    }
}
