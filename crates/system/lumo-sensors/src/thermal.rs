//! Thermal zones — /sys/class/thermal/thermal_zone[0-7]
//!
//! Galaxy Book 4 has 8 zones. TCPU = x86_pkg_temp is the principal CPU sensor.
//! Temp files contain millidegree values (divide by 1000 for Celsius).

use std::path::PathBuf;

use crate::{read_sysfs_u32, SensorError};

const THERMAL_BASE: &str = "/sys/class/thermal";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThermalKind {
    Cpu,
    Soc,
    Charger,
    Nvme,
    Wifi,
    Other,
}

impl ThermalKind {
    fn from_type_str(s: &str) -> Self {
        let lower = s.to_ascii_lowercase();
        if lower.contains("x86_pkg") || lower.contains("tcpu") || lower.contains("cpu") {
            Self::Cpu
        } else if lower.contains("soc") {
            Self::Soc
        } else if lower.contains("charg") {
            Self::Charger
        } else if lower.contains("nvme") || lower.contains("sns") {
            Self::Nvme
        } else if lower.contains("wifi") || lower.contains("iwl") {
            Self::Wifi
        } else {
            Self::Other
        }
    }
}

pub struct ThermalZone {
    pub name: String,
    pub kind: ThermalKind,
    temp_path: PathBuf,
}

impl ThermalZone {
    pub fn discover_all() -> Vec<Self> {
        let mut zones = Vec::new();
        for idx in 0..16 {
            let base = PathBuf::from(format!("{THERMAL_BASE}/thermal_zone{idx}"));
            if !base.exists() {
                continue;
            }
            let type_path = base.join("type");
            let temp_path = base.join("temp");
            if !temp_path.exists() {
                continue;
            }
            let name = std::fs::read_to_string(&type_path)
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| format!("zone{idx}"));
            let kind = ThermalKind::from_type_str(&name);
            zones.push(ThermalZone {
                name,
                kind,
                temp_path,
            });
        }
        zones
    }

    /// Returns temperature in Celsius. sysfs exposes millidegrees.
    pub fn temp_celsius(&self) -> Result<f32, SensorError> {
        let raw = read_sysfs_u32(&self.temp_path)?;
        Ok(raw as f32 / 1000.0)
    }
}
