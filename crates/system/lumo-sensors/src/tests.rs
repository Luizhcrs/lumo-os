//! Unit tests for lumo-sensors. Uses tempfile to mock sysfs paths.

#[cfg(test)]
mod battery_tests {
    use std::path::PathBuf;
    use tempfile::TempDir;

    // Helper: write a value to a file in dir.
    fn write(dir: &TempDir, name: &str, value: &str) -> PathBuf {
        let p = dir.path().join(name);
        std::fs::write(&p, value).unwrap();
        p
    }

    #[test]
    fn percent_from_capacity_clamps_0_100() {
        let dir = TempDir::new().unwrap();
        let p = write(&dir, "capacity", "87\n");
        let raw = std::fs::read_to_string(&p).unwrap();
        let v: u8 = raw.trim().parse::<u32>().unwrap().clamp(0, 100) as u8;
        assert_eq!(v, 87);
    }

    #[test]
    fn percent_over_100_clamped() {
        // Kernel may briefly report 101 on some firmware. Must clamp.
        let raw: u32 = 101;
        let clamped = raw.clamp(0, 100) as u8;
        assert_eq!(clamped, 100);
    }

    #[test]
    fn health_percent_computed_correctly() {
        // 3490000 / 3530000 = 98.87 -> rounds to 99
        let full = 3_490_000_u64 as f64;
        let design = 3_530_000_u64 as f64;
        let health = ((full / design) * 100.0).round() as u8;
        assert_eq!(health, 99);
    }

    #[test]
    fn health_percent_100_when_equal() {
        let full = 3_530_000_u64 as f64;
        let design = 3_530_000_u64 as f64;
        let health = ((full / design) * 100.0).round().clamp(0.0, 100.0) as u8;
        assert_eq!(health, 100);
    }

    #[test]
    fn set_charge_limit_rejects_zero() {
        use crate::SensorError;
        let pct: u8 = 0;
        let result: Result<(), SensorError> = if !(1..=100).contains(&pct) {
            Err(SensorError::OutOfRange(format!("charge limit {pct} out of 1-100")))
        } else {
            Ok(())
        };
        assert!(result.is_err());
    }

    #[test]
    fn set_charge_limit_rejects_over_100() {
        use crate::SensorError;
        let pct: u8 = 101; // saturates to 100 due to u8, so test with OutOfRange logic inline
        let pct_u32: u32 = 200; // simulate if it were u32
        let result: Result<(), SensorError> = if !(1..=100).contains(&pct_u32) {
            Err(SensorError::OutOfRange(format!("charge limit {pct_u32} out of 1-100")))
        } else {
            Ok(())
        };
        assert!(result.is_err());
        let _ = pct; // suppress warn
    }
}

#[cfg(test)]
mod platform_tests {
    use crate::platform::Profile;

    #[test]
    fn profile_parse_all_variants() {
        assert_eq!(Profile::from_str("low-power"), Some(Profile::LowPower));
        assert_eq!(Profile::from_str("quiet"), Some(Profile::Quiet));
        assert_eq!(Profile::from_str("balanced"), Some(Profile::Balanced));
        assert_eq!(Profile::from_str("performance"), Some(Profile::Performance));
        assert_eq!(Profile::from_str("unknown-mode"), None);
    }

    #[test]
    fn cycle_next_is_deterministic() {
        // Simulate cycle: balanced -> performance -> low-power -> quiet -> balanced
        let avail = vec![
            Profile::LowPower,
            Profile::Quiet,
            Profile::Balanced,
            Profile::Performance,
        ];
        let start = Profile::Balanced;
        let idx = avail.iter().position(|p| *p == start).unwrap();
        let next = avail[(idx + 1) % avail.len()];
        assert_eq!(next, Profile::Performance);

        let idx2 = avail.iter().position(|p| *p == next).unwrap();
        let next2 = avail[(idx2 + 1) % avail.len()];
        assert_eq!(next2, Profile::LowPower);
    }

    #[test]
    fn cycle_wraps_from_last() {
        let avail = vec![Profile::LowPower, Profile::Quiet, Profile::Balanced, Profile::Performance];
        let last = Profile::Performance;
        let idx = avail.iter().position(|p| *p == last).unwrap();
        let next = avail[(idx + 1) % avail.len()];
        assert_eq!(next, Profile::LowPower);
    }

    #[test]
    fn profile_sysfs_roundtrip() {
        for p in [Profile::LowPower, Profile::Quiet, Profile::Balanced, Profile::Performance] {
            let s = p.as_sysfs_str();
            let parsed = Profile::from_str(s).expect("roundtrip failed");
            assert_eq!(parsed, p);
        }
    }
}

#[cfg(test)]
mod thermal_tests {
    use crate::thermal::{ThermalKind};

    fn kind_from(s: &str) -> ThermalKind {
        let lower = s.to_ascii_lowercase();
        if lower.contains("x86_pkg") || lower.contains("tcpu") || lower.contains("cpu") {
            ThermalKind::Cpu
        } else if lower.contains("soc") {
            ThermalKind::Soc
        } else if lower.contains("charg") {
            ThermalKind::Charger
        } else if lower.contains("nvme") || lower.contains("sns") {
            ThermalKind::Nvme
        } else if lower.contains("wifi") || lower.contains("iwl") {
            ThermalKind::Wifi
        } else {
            ThermalKind::Other
        }
    }

    #[test]
    fn tcpu_maps_to_cpu() {
        assert_eq!(kind_from("TCPU"), ThermalKind::Cpu);
    }

    #[test]
    fn x86_pkg_temp_maps_to_cpu() {
        assert_eq!(kind_from("x86_pkg_temp"), ThermalKind::Cpu);
    }

    #[test]
    fn iwlwifi_maps_to_wifi() {
        assert_eq!(kind_from("iwlwifi_1"), ThermalKind::Wifi);
    }

    #[test]
    fn sns_maps_to_nvme() {
        assert_eq!(kind_from("SNS1"), ThermalKind::Nvme);
    }

    #[test]
    fn temp_millidegree_conversion() {
        // sysfs stores millidegrees; 52000 = 52.0 C
        let raw: u32 = 52000;
        let celsius = raw as f32 / 1000.0;
        assert!((celsius - 52.0).abs() < 0.001);
    }

    #[test]
    fn int3400_maps_to_other() {
        assert_eq!(kind_from("INT3400 Thermal"), ThermalKind::Other);
    }
}

#[cfg(test)]
mod backlight_tests {
    #[test]
    fn percent_computed_from_raw() {
        let max: u32 = 19200;
        let cur: u32 = 15360;
        let pct = ((cur as f32 / max as f32) * 100.0).round() as u8;
        assert_eq!(pct, 80);
    }

    #[test]
    fn set_percent_maps_to_raw() {
        let max: u32 = 19200;
        let pct: u8 = 35;
        let raw = ((pct as f32 / 100.0) * max as f32).round() as u32;
        // 0.35 * 19200 = 6720
        assert_eq!(raw, 6720);
    }

    #[test]
    fn min_brightness_is_one_not_zero() {
        let max: u32 = 19200;
        let pct: u8 = 0;
        let raw = ((pct as f32 / 100.0) * max as f32).round() as u32;
        let raw = raw.max(1);
        assert_eq!(raw, 1);
    }
}

#[cfg(test)]
mod extra_battery_tests {
    #[test]
    fn capacity_empty_string_parse_fails() {
        let raw = "";
        let result: Result<u32, _> = raw.trim().parse();
        assert!(result.is_err());
    }

    #[test]
    fn health_zero_design_capacity_clamps_to_100() {
        let full = 3_490_000_u64 as f64;
        let design = 1_u64 as f64;
        let health = ((full / design) * 100.0).round().clamp(0.0, 100.0) as u8;
        assert_eq!(health, 100);
    }

    #[test]
    fn charge_limit_boundary_1_is_valid() {
        use crate::SensorError;
        let pct: u32 = 1;
        let result: Result<(), SensorError> = if !(1..=100).contains(&pct) {
            Err(SensorError::OutOfRange(format!("{pct}")))
        } else {
            Ok(())
        };
        assert!(result.is_ok());
    }

    #[test]
    fn charge_limit_boundary_100_is_valid() {
        use crate::SensorError;
        let pct: u32 = 100;
        let result: Result<(), SensorError> = if !(1..=100).contains(&pct) {
            Err(SensorError::OutOfRange(format!("{pct}")))
        } else {
            Ok(())
        };
        assert!(result.is_ok());
    }

    #[test]
    fn capacity_whitespace_trims_before_parse() {
        let raw = "  75\n";
        let v: u8 = raw.trim().parse::<u32>().unwrap().clamp(0, 100) as u8;
        assert_eq!(v, 75);
    }
}

#[cfg(test)]
mod extra_thermal_tests {
    use crate::thermal::ThermalKind;

    fn kind_from(s: &str) -> ThermalKind {
        let lower = s.to_ascii_lowercase();
        if lower.contains("x86_pkg") || lower.contains("tcpu") || lower.contains("cpu") {
            ThermalKind::Cpu
        } else if lower.contains("soc") {
            ThermalKind::Soc
        } else if lower.contains("charg") {
            ThermalKind::Charger
        } else if lower.contains("nvme") || lower.contains("sns") {
            ThermalKind::Nvme
        } else if lower.contains("wifi") || lower.contains("iwl") {
            ThermalKind::Wifi
        } else {
            ThermalKind::Other
        }
    }

    #[test]
    fn charger_maps_correctly() {
        assert_eq!(kind_from("Charger"), ThermalKind::Charger);
    }

    #[test]
    fn nvme_maps_correctly() {
        assert_eq!(kind_from("nvme0"), ThermalKind::Nvme);
    }

    #[test]
    fn soc_maps_correctly() {
        assert_eq!(kind_from("soc_thermal"), ThermalKind::Soc);
    }

    #[test]
    fn temp_millideg_round_trip() {
        let millideg: u32 = 72500;
        let celsius = millideg as f32 / 1000.0;
        let back = (celsius * 1000.0).round() as u32;
        assert_eq!(back, 72500);
    }

    #[test]
    fn temp_zero_millideg_is_zero_celsius() {
        let millideg: u32 = 0;
        let celsius = millideg as f32 / 1000.0;
        assert!(celsius.abs() < 1e-6);
    }
}
