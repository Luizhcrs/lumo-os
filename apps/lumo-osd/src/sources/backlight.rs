//! backlight.rs — leitura de /sys/class/backlight/*/brightness + max_brightness.
//!
//! Path pattern Linux kernel:
//!   /sys/class/backlight/{intel_backlight,amdgpu_bl0,nvidia_0}/brightness
//!   /sys/class/backlight/{...}/max_brightness
//!
//! Galaxy Book 4: intel_backlight ou samsung-galaxybook.
//!
//! pct = brightness / max_brightness * 100.

use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BacklightState {
    pub current: u32,
    pub max: u32,
}

impl BacklightState {
    pub fn pct(&self) -> f32 {
        if self.max == 0 {
            return 0.0;
        }
        (self.current as f32 / self.max as f32 * 100.0).clamp(0.0, 100.0)
    }

    /// 0.0-1.0 normalized.
    pub fn ratio(&self) -> f32 {
        if self.max == 0 {
            return 0.0;
        }
        (self.current as f32 / self.max as f32).clamp(0.0, 1.0)
    }
}

pub fn default_root() -> PathBuf {
    PathBuf::from("/sys/class/backlight")
}

/// Le state de 1 backlight dir. None se ausente.
pub fn read_one(dir: &Path) -> Option<BacklightState> {
    let cur = fs::read_to_string(dir.join("brightness")).ok()?;
    let max = fs::read_to_string(dir.join("max_brightness")).ok()?;
    let current: u32 = cur.trim().parse().ok()?;
    let max: u32 = max.trim().parse().ok()?;
    Some(BacklightState { current, max })
}

/// Encontra primeiro backlight disponivel. Multi-monitor pega first only.
pub fn read_first(root: &Path) -> Option<BacklightState> {
    let entries = fs::read_dir(root).ok()?;
    for entry in entries.flatten() {
        if let Some(s) = read_one(&entry.path()) {
            return Some(s);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp() -> PathBuf {
        std::env::temp_dir().join(format!(
            "lumo-osd-bl-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ))
    }

    fn mk(root: &Path, name: &str, current: &str, max: &str) {
        let d = root.join(name);
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("brightness"), current).unwrap();
        fs::write(d.join("max_brightness"), max).unwrap();
    }

    #[test]
    fn pct_zero_when_current_zero() {
        let s = BacklightState { current: 0, max: 100 };
        assert_eq!(s.pct(), 0.0);
    }

    #[test]
    fn pct_full_when_current_eq_max() {
        let s = BacklightState { current: 100, max: 100 };
        assert_eq!(s.pct(), 100.0);
    }

    #[test]
    fn pct_half() {
        let s = BacklightState { current: 50, max: 100 };
        assert!((s.pct() - 50.0).abs() < 0.01);
    }

    #[test]
    fn pct_clamped_when_current_above_max() {
        let s = BacklightState { current: 150, max: 100 };
        assert_eq!(s.pct(), 100.0);
    }

    #[test]
    fn pct_zero_when_max_zero() {
        let s = BacklightState { current: 50, max: 0 };
        assert_eq!(s.pct(), 0.0);
    }

    #[test]
    fn ratio_clamp_and_match_pct() {
        let s = BacklightState { current: 75, max: 100 };
        assert!((s.ratio() - 0.75).abs() < 0.01);
        assert!((s.pct() / 100.0 - s.ratio()).abs() < 0.01);
    }

    #[test]
    fn read_one_returns_state() {
        let t = tmp();
        fs::create_dir_all(&t).unwrap();
        let d = t.join("intel_backlight");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("brightness"), "120").unwrap();
        fs::write(d.join("max_brightness"), "255").unwrap();
        let s = read_one(&d).expect("read");
        assert_eq!(s.current, 120);
        assert_eq!(s.max, 255);
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn read_one_missing_returns_none() {
        let t = tmp();
        assert!(read_one(&t.join("nope")).is_none());
    }

    #[test]
    fn read_one_invalid_number_returns_none() {
        let t = tmp();
        fs::create_dir_all(&t).unwrap();
        let d = t.join("bad");
        fs::create_dir_all(&d).unwrap();
        fs::write(d.join("brightness"), "not-a-number").unwrap();
        fs::write(d.join("max_brightness"), "255").unwrap();
        assert!(read_one(&d).is_none());
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn read_first_returns_one_when_present() {
        let t = tmp();
        fs::create_dir_all(&t).unwrap();
        mk(&t, "intel_backlight", "50", "100");
        let s = read_first(&t).expect("first");
        assert_eq!(s.current, 50);
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn read_first_missing_root_none() {
        assert!(read_first(&PathBuf::from("/this/does/not/exist/xyz")).is_none());
    }

    #[test]
    fn read_first_empty_root_none() {
        let t = tmp();
        fs::create_dir_all(&t).unwrap();
        assert!(read_first(&t).is_none());
        fs::remove_dir_all(&t).ok();
    }

    #[test]
    fn read_first_skips_invalid_picks_valid() {
        let t = tmp();
        fs::create_dir_all(&t).unwrap();
        let bad = t.join("bad");
        fs::create_dir_all(&bad).unwrap();
        fs::write(bad.join("brightness"), "garbage").unwrap();
        fs::write(bad.join("max_brightness"), "x").unwrap();
        mk(&t, "good", "200", "1000");
        let s = read_first(&t).expect("first valid");
        // Como read_dir ordem nao garantida, so verifica que pegou algum valido.
        assert!(s.current == 200 || s.max == 1000);
        fs::remove_dir_all(&t).ok();
    }
}
