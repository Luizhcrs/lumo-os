//! Battery — /sys/class/power_supply/BAT1/
//!
//! Galaxy Book 4 uses BAT1 (not BAT0). charge_control_end_threshold is
//! writable via samsung-galaxybook driver.

use std::path::PathBuf;

use crate::{read_sysfs_trimmed, read_sysfs_u32, write_sysfs, SensorError};

const SUPPLY_DIRS: &[&str] = &[
    "/sys/class/power_supply/BAT1",
    "/sys/class/power_supply/BAT0",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChargingStatus {
    Charging,
    Discharging,
    Full,
    NotCharging,
    Unknown,
}

impl ChargingStatus {
    fn parse(s: &str) -> Self {
        match s {
            "Charging" => Self::Charging,
            "Discharging" => Self::Discharging,
            "Full" => Self::Full,
            "Not charging" | "Not Charging" => Self::NotCharging,
            _ => Self::Unknown,
        }
    }
}

pub struct Battery {
    base: PathBuf,
    pub name: String,
}

impl Battery {
    /// Find first available supply dir. Returns error if none present.
    pub fn discover() -> Result<Self, SensorError> {
        for dir in SUPPLY_DIRS {
            let p = PathBuf::from(dir);
            if p.exists() {
                let name = p
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("BAT?")
                    .to_string();
                return Ok(Self { base: p, name });
            }
        }
        Err(SensorError::NotSupported(
            "no battery supply found under /sys/class/power_supply/".into(),
        ))
    }

    fn path(&self, file: &str) -> PathBuf {
        self.base.join(file)
    }

    // charge_now / charge_full in uAh; convert to percent.
    pub fn percent(&self) -> Result<u8, SensorError> {
        // Try kernel-provided capacity first (simpler, kernel-smoothed).
        let cap_path = self.path("capacity");
        if cap_path.exists() {
            let v = read_sysfs_u32(&cap_path)?;
            return Ok(v.clamp(0, 100) as u8);
        }
        // Fallback: compute from charge_now / charge_full.
        let now = read_sysfs_u32(&self.path("charge_now"))? as f32;
        let full = read_sysfs_u32(&self.path("charge_full"))? as f32;
        if full < 1.0 {
            return Err(SensorError::Parse("charge_full is zero".into()));
        }
        Ok(((now / full) * 100.0).round().clamp(0.0, 100.0) as u8)
    }

    /// Wear percentage: charge_full / charge_full_design * 100.
    pub fn health_percent(&self) -> Result<u8, SensorError> {
        let full = read_sysfs_u32(&self.path("charge_full"))? as f64;
        let design = read_sysfs_u32(&self.path("charge_full_design"))? as f64;
        if design < 1.0 {
            return Err(SensorError::Parse("charge_full_design is zero".into()));
        }
        Ok(((full / design) * 100.0).round().clamp(0.0, 100.0) as u8)
    }

    pub fn cycle_count(&self) -> Option<u32> {
        let p = self.path("cycle_count");
        if p.exists() {
            read_sysfs_u32(&p).ok()
        } else {
            None
        }
    }

    pub fn charge_limit(&self) -> Option<u8> {
        let p = self.path("charge_control_end_threshold");
        if p.exists() {
            read_sysfs_u32(&p).ok().map(|v| v.clamp(0, 100) as u8)
        } else {
            None
        }
    }

    /// Write charge_control_end_threshold. Requires polkit rule or group write access.
    /// pct must be in 1..=100.
    pub fn set_charge_limit(&self, pct: u8) -> Result<(), SensorError> {
        if !(1..=100).contains(&pct) {
            return Err(SensorError::OutOfRange(format!(
                "charge limit {pct} out of 1-100"
            )));
        }
        let p = self.path("charge_control_end_threshold");
        if !p.exists() {
            return Err(SensorError::NotSupported(
                "charge_control_end_threshold not present; samsung-galaxybook driver required"
                    .into(),
            ));
        }
        write_sysfs(&p, &pct.to_string())
    }

    pub fn status(&self) -> ChargingStatus {
        let p = self.path("status");
        match read_sysfs_trimmed(&p) {
            Ok(s) => ChargingStatus::parse(&s),
            Err(_) => ChargingStatus::Unknown,
        }
    }

    /// Estimated seconds remaining. Computed from charge_now and current_now.
    /// Returns None if not discharging or current data unavailable.
    pub fn time_remaining_secs(&self) -> Option<u32> {
        let status = self.status();
        let current_raw = read_sysfs_i32(&self.path("current_now")).ok()? as f64;
        let current = current_raw.abs();
        if current < 1.0 {
            return None;
        }
        match status {
            ChargingStatus::Discharging => {
                let now = read_sysfs_u32(&self.path("charge_now")).ok()? as f64;
                Some(((now / current) * 3600.0).round() as u32)
            }
            ChargingStatus::Charging => {
                let now = read_sysfs_u32(&self.path("charge_now")).ok()? as f64;
                let full = read_sysfs_u32(&self.path("charge_full")).ok()? as f64;
                let remaining = (full - now).max(0.0);
                Some(((remaining / current) * 3600.0).round() as u32)
            }
            _ => None,
        }
    }
}

// ============================================================
// ChargePolicy - P5: charge limit 80% + weekly cell balance
// ============================================================

/// Policy governing charge limit and weekly cell balance cycle.
///
/// Default: limit 80%, balance every Friday 22:00 (cron "0 22 * * 5"),
/// charging to 100% for 12 hours then returning to 80%.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChargePolicy {
    /// Normal charge ceiling in percent (1-100). Default 80.
    pub limit_percent: u8,
    /// Cron expression for weekly balance. Default "0 22 * * 5" (Fri 22h).
    pub balance_schedule_cron: String,
    /// Temporary target during balance cycle. Default 100.
    pub balance_target: u8,
    /// Hours to hold balance_target before reverting to limit_percent. Default 12.
    pub balance_duration_hours: u8,
}

impl Default for ChargePolicy {
    fn default() -> Self {
        Self {
            limit_percent: 80,
            balance_schedule_cron: "0 22 * * 5".to_string(),
            balance_target: 100,
            balance_duration_hours: 12,
        }
    }
}

impl Battery {
    /// Apply charge policy: writes charge_control_end_threshold to
    /// policy.limit_percent.
    pub fn apply_policy(&self, policy: &ChargePolicy) -> Result<(), SensorError> {
        self.set_charge_limit(policy.limit_percent)
    }

    /// Begin a balance cycle: temporarily raises the charge limit to
    /// policy.balance_target. Returns the duration the caller should wait
    /// before calling end_balance_cycle.
    pub fn begin_balance_cycle(
        &self,
        policy: &ChargePolicy,
    ) -> Result<std::time::Duration, SensorError> {
        self.set_charge_limit(policy.balance_target)?;
        Ok(std::time::Duration::from_secs(
            policy.balance_duration_hours as u64 * 3600,
        ))
    }

    /// End a balance cycle: restores the charge limit to policy.limit_percent.
    pub fn end_balance_cycle(&self, policy: &ChargePolicy) -> Result<(), SensorError> {
        self.set_charge_limit(policy.limit_percent)
    }
}
