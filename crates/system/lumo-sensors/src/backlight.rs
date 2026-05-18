//! Backlight — /sys/class/backlight/intel_backlight/
//!
//! Galaxy Book 4 uses intel_backlight. Write requires group video or polkit rule.

use std::path::PathBuf;

use crate::{read_sysfs_u32, write_sysfs, SensorError};

const BACKLIGHT_DIRS: &[&str] = &[
    "/sys/class/backlight/intel_backlight",
    "/sys/class/backlight/amdgpu_bl0",
    "/sys/class/backlight/acpi_video0",
];

pub struct Backlight {
    base: PathBuf,
}

impl Backlight {
    pub fn discover() -> Option<Self> {
        for dir in BACKLIGHT_DIRS {
            let p = PathBuf::from(dir);
            if p.exists() && p.join("brightness").exists() {
                return Some(Self { base: p });
            }
        }
        None
    }

    fn path(&self, file: &str) -> PathBuf {
        self.base.join(file)
    }

    pub fn brightness(&self) -> Result<u32, SensorError> {
        read_sysfs_u32(&self.path("brightness"))
    }

    pub fn max(&self) -> Result<u32, SensorError> {
        read_sysfs_u32(&self.path("max_brightness"))
    }

    pub fn percent(&self) -> Result<u8, SensorError> {
        let cur = self.brightness()? as f32;
        let max = self.max()? as f32;
        if max < 1.0 {
            return Err(SensorError::Parse("max_brightness is zero".into()));
        }
        Ok(((cur / max) * 100.0).round().clamp(0.0, 100.0) as u8)
    }

    /// Set brightness from percentage 0-100.
    /// Writes to brightness file — requires group video or polkit rule.
    pub fn set_percent(&self, pct: u8) -> Result<(), SensorError> {
        if pct > 100 {
            return Err(SensorError::OutOfRange(format!("brightness {pct} > 100")));
        }
        let max = self.max()? as f32;
        let raw = ((pct as f32 / 100.0) * max).round() as u32;
        // Ensure at least 1 so display doesn't go fully dark.
        let raw = raw.max(1);
        write_sysfs(&self.path("brightness"), &raw.to_string())
    }
}
