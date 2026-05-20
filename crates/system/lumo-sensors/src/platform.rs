//! Platform profile — /sys/firmware/acpi/platform_profile
//!
//! Galaxy Book 4 exposes 4 modes: low-power, quiet, balanced, performance.
//! Kernel docs mention 3 but samsung-galaxybook adds "quiet" between
//! low-power and balanced.

use std::path::PathBuf;

use crate::{read_sysfs_trimmed, write_sysfs, SensorError};

const PROFILE_PATH: &str = "/sys/firmware/acpi/platform_profile";
const CHOICES_PATH: &str = "/sys/firmware/acpi/platform_profile_choices";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    LowPower,
    Quiet,
    Balanced,
    Performance,
}

impl Profile {
    pub fn from_sysfs(s: &str) -> Option<Self> {
        match s {
            "low-power" => Some(Self::LowPower),
            "quiet" => Some(Self::Quiet),
            "balanced" => Some(Self::Balanced),
            "performance" => Some(Self::Performance),
            _ => None,
        }
    }

    pub fn as_sysfs_str(&self) -> &'static str {
        match self {
            Self::LowPower => "low-power",
            Self::Quiet => "quiet",
            Self::Balanced => "balanced",
            Self::Performance => "performance",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Self::LowPower => "Economia",
            Self::Quiet => "Silencioso",
            Self::Balanced => "Equilibrado",
            Self::Performance => "Performance",
        }
    }
}

pub trait PlatformProfile {
    fn current(&self) -> Profile;
    fn available(&self) -> Vec<Profile>;
    fn set(&self, p: Profile) -> Result<(), SensorError>;
    fn cycle_next(&self) -> Result<Profile, SensorError>;
}

pub struct SysfsPlatformProfile {
    profile_path: PathBuf,
    choices_path: PathBuf,
}

impl SysfsPlatformProfile {
    pub fn discover() -> Option<Self> {
        let p = PathBuf::from(PROFILE_PATH);
        if p.exists() {
            Some(Self {
                profile_path: p,
                choices_path: PathBuf::from(CHOICES_PATH),
            })
        } else {
            None
        }
    }
}

impl PlatformProfile for SysfsPlatformProfile {
    fn current(&self) -> Profile {
        match read_sysfs_trimmed(&self.profile_path) {
            Ok(s) => Profile::from_sysfs(&s).unwrap_or(Profile::Balanced),
            Err(_) => Profile::Balanced,
        }
    }

    fn available(&self) -> Vec<Profile> {
        let fallback = vec![Profile::LowPower, Profile::Balanced, Profile::Performance];
        if !self.choices_path.exists() {
            return fallback;
        }
        match read_sysfs_trimmed(&self.choices_path) {
            Ok(s) => {
                let v: Vec<Profile> = s
                    .split_whitespace()
                    .filter_map(Profile::from_sysfs)
                    .collect();
                if v.is_empty() { fallback } else { v }
            }
            Err(_) => fallback,
        }
    }

    fn set(&self, p: Profile) -> Result<(), SensorError> {
        write_sysfs(&self.profile_path, p.as_sysfs_str())
    }

    fn cycle_next(&self) -> Result<Profile, SensorError> {
        let avail = self.available();
        let current = self.current();
        let next = avail
            .iter()
            .position(|p| *p == current)
            .map(|idx| avail[(idx + 1) % avail.len()])
            .unwrap_or(Profile::Balanced);
        self.set(next)?;
        Ok(next)
    }
}
