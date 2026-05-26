//! # lumo-sensors
//!
//! Proposito: Registry de sensores sysfs para Galaxy Book 4 (Arch Linux, kernel 7.x).
//!
//! ## Invariantes
//! - Paths cacheados em discover(); leituras sao sempre fresh (sem cache interno).
//! - Write ops exigem polkit rule 49-lumo-sensors.rules ou membership no grupo correto.
//! - Validado empiricamente em Galaxy Book 4 NP750XGJ-* (2026-05-18).
//!
//! ## Memory refs
//! - [[feedback-design-lapidado]]
//! - [[project-lumo-os]]

pub mod backlight;
pub mod battery;
pub mod lid;
pub mod platform;
pub mod thermal;

pub use backlight::Backlight;
pub use battery::{Battery, ChargingStatus};
pub use lid::{LidState, LidSwitch};
pub use platform::{PlatformProfile, Profile, SysfsPlatformProfile};
pub use thermal::{ThermalKind, ThermalZone};

use std::path::PathBuf;

// ============================================================
// SensorError
// ============================================================

#[derive(Debug)]
pub enum SensorError {
    Io(std::io::Error),
    Parse(String),
    OutOfRange(String),
    NotSupported(String),
}

impl std::fmt::Display for SensorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "io: {e}"),
            Self::Parse(s) => write!(f, "parse: {s}"),
            Self::OutOfRange(s) => write!(f, "out of range: {s}"),
            Self::NotSupported(s) => write!(f, "not supported: {s}"),
        }
    }
}

impl std::error::Error for SensorError {}

impl From<std::io::Error> for SensorError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

// ============================================================
// SensorRegistry
// ============================================================

/// Central registry — paths resolved once at discover().
pub struct SensorRegistry {
    battery: Battery,
    platform_profile: Option<SysfsPlatformProfile>,
    lid_switch: Option<LidSwitch>,
    thermal_zones: Vec<ThermalZone>,
    backlight: Option<Backlight>,
}

impl SensorRegistry {
    /// Probe sysfs and build registry. Errors only if battery path is absent.
    pub fn discover() -> Result<Self, SensorError> {
        let battery = Battery::discover()?;

        let platform_profile = SysfsPlatformProfile::discover();

        let lid_switch = LidSwitch::discover();

        let thermal_zones = ThermalZone::discover_all();

        let backlight = Backlight::discover();

        Ok(Self {
            battery,
            platform_profile,
            lid_switch,
            thermal_zones,
            backlight,
        })
    }

    pub fn battery(&self) -> &Battery {
        &self.battery
    }

    pub fn platform_profile(&self) -> Option<&SysfsPlatformProfile> {
        self.platform_profile.as_ref()
    }

    pub fn lid_switch(&self) -> Option<&LidSwitch> {
        self.lid_switch.as_ref()
    }

    pub fn thermal_zones(&self) -> &[ThermalZone] {
        &self.thermal_zones
    }

    pub fn backlight(&self) -> Option<&Backlight> {
        self.backlight.as_ref()
    }
}

// ============================================================
// internal helpers
// ============================================================

pub(crate) fn read_sysfs_trimmed(path: &PathBuf) -> Result<String, SensorError> {
    let s = std::fs::read_to_string(path)?;
    Ok(s.trim().to_string())
}

pub(crate) fn read_sysfs_u32(path: &PathBuf) -> Result<u32, SensorError> {
    let s = read_sysfs_trimmed(path)?;
    s.parse::<u32>()
        .map_err(|_| SensorError::Parse(format!("not u32: {s:?} at {path:?}")))
}

pub(crate) fn write_sysfs(path: &PathBuf, value: &str) -> Result<(), SensorError> {
    std::fs::write(path, value).map_err(SensorError::Io)
}
#[cfg(test)]
mod tests;
