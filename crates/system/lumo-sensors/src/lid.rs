//! Lid switch — /proc/acpi/button/lid/*/state
//!
//! Galaxy Book 4 exposes lid state via ACPI button. SW_LID events also
//! available via evdev (handled in lumo-wm lid_handler).

use std::path::PathBuf;

const ACPI_LID_GLOB: &str = "/proc/acpi/button/lid";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LidState {
    Open,
    Closed,
}

pub struct LidSwitch {
    state_path: PathBuf,
}

impl LidSwitch {
    pub fn discover() -> Option<Self> {
        // Enumerate /proc/acpi/button/lid/LID*/state
        let base = PathBuf::from(ACPI_LID_GLOB);
        if !base.exists() {
            return None;
        }
        let entries = std::fs::read_dir(&base).ok()?;
        for entry in entries.flatten() {
            let state_path = entry.path().join("state");
            if state_path.exists() {
                return Some(Self { state_path });
            }
        }
        None
    }

    pub fn current_state(&self) -> LidState {
        match std::fs::read_to_string(&self.state_path) {
            Ok(s) => {
                if s.contains("closed") {
                    LidState::Closed
                } else {
                    LidState::Open
                }
            }
            Err(_) => LidState::Open, // safe default
        }
    }
}
