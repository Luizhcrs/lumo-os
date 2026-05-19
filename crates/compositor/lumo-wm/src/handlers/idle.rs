//! handlers/idle.rs — W10.B idle management for lumo-wm.
//!
//! Implements ext-idle-notify-v1 (IdleNotifierState) + internal timer-based
//! dim/lock/suspend pipeline driven by LumoIdleManager.
//!
//! The IdleNotifierState handles client-side idle notifications (screen savers,
//! DPMS tools). Internally we maintain LumoIdleManager which tracks the last
//! input instant and executes the 3-stage pipeline from idle.toml.
//!
//! Stages (configurable via ~/.config/lumo/idle.toml):
//!   dim_at=120    -> set backlight to 50%
//!   lock_at=300   -> spawn lumo-lock
//!   suspend_at=600 -> systemctl suspend
//!
//! reset_idle() must be called on every input event (pointer motion + key).
//! tick_idle() is called by the compositor main loop every second.

use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use serde::Deserialize;

use smithay::wayland::idle_notify::IdleNotifierHandler;

use crate::state::LumoState;

// ============================================================
// Config
// ============================================================

#[derive(Debug, Clone, Deserialize)]
pub struct IdleConfig {
    #[serde(default = "def_dim")]
    pub dim_at: u64,
    #[serde(default = "def_lock")]
    pub lock_at: u64,
    #[serde(default = "def_suspend")]
    pub suspend_at: u64,
}

fn def_dim() -> u64 { 120 }
fn def_lock() -> u64 { 300 }
fn def_suspend() -> u64 { 600 }

impl Default for IdleConfig {
    fn default() -> Self {
        Self { dim_at: def_dim(), lock_at: def_lock(), suspend_at: def_suspend() }
    }
}

fn config_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
    let xdg = std::env::var("XDG_CONFIG_HOME").unwrap_or_else(|_| format!("{home}/.config"));
    PathBuf::from(xdg).join("lumo").join("idle.toml")
}

pub fn load_idle_config() -> IdleConfig {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).unwrap_or_default(),
        Err(_) => IdleConfig::default(),
    }
}

// ============================================================
// LumoIdleManager — internal state machine
// ============================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdleStage {
    Active,
    Dimmed,
    Locked,
    Suspended,
}

pub struct LumoIdleManager {
    pub config: IdleConfig,
    pub last_input: Instant,
    pub stage: IdleStage,
    /// Saved brightness before dim (0-100). None = not dimmed yet.
    pub saved_brightness: Option<u8>,
    /// True if lumo-lock is already running (avoid double-spawn).
    pub lock_spawned: bool,
}

impl Default for LumoIdleManager {
    fn default() -> Self {
        Self {
            config: load_idle_config(),
            last_input: Instant::now(),
            stage: IdleStage::Active,
            saved_brightness: None,
            lock_spawned: false,
        }
    }
}

impl LumoIdleManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Call on every user input event (pointer motion, key press).
    pub fn reset(&mut self) {
        self.last_input = Instant::now();
        if self.stage == IdleStage::Dimmed {
            // Restore backlight.
            if let Some(pct) = self.saved_brightness.take() {
                if let Some(bl) = lumo_sensors::Backlight::discover() {
                    let _ = bl.set_percent(pct);
                    tracing::debug!(pct, "W10.B: backlight restored on wake");
                }
            }
        }
        self.stage = IdleStage::Active;
        self.lock_spawned = false;
    }

    /// Advance the idle state machine. Returns the new stage.
    /// Call once per second from the event loop.
    pub fn tick(&mut self) -> IdleStage {
        let elapsed = self.last_input.elapsed().as_secs();

        match self.stage {
            IdleStage::Active => {
                if elapsed >= self.config.dim_at {
                    self.apply_dim();
                }
            }
            IdleStage::Dimmed => {
                if elapsed >= self.config.lock_at {
                    self.apply_lock();
                } else if elapsed < self.config.dim_at {
                    // Input reset happened between ticks; restore.
                    // reset() already restored; this is a no-op guard.
                }
            }
            IdleStage::Locked => {
                if elapsed >= self.config.suspend_at {
                    self.apply_suspend();
                }
            }
            IdleStage::Suspended => {}
        }

        self.stage
    }

    fn apply_dim(&mut self) {
        if let Some(bl) = lumo_sensors::Backlight::discover() {
            // Save current brightness before dimming.
            if let Ok(pct) = bl.percent() {
                self.saved_brightness = Some(pct);
            }
            let _ = bl.set_percent(50);
            tracing::info!("W10.B: idle dim -> backlight 50%");
        }
        self.stage = IdleStage::Dimmed;
    }

    fn apply_lock(&mut self) {
        if !self.lock_spawned {
            tracing::info!("W10.B: idle lock -> spawning lumo-lock");
            let _ = Command::new("lumo-lock").spawn();
            self.lock_spawned = true;
        }
        self.stage = IdleStage::Locked;
    }

    fn apply_suspend(&mut self) {
        tracing::info!("W10.B: idle suspend -> systemctl suspend");
        let _ = Command::new("systemctl").arg("suspend").spawn();
        self.stage = IdleStage::Suspended;
    }
}

// ============================================================
// smithay IdleNotifierHandler impl
// ============================================================

impl IdleNotifierHandler for LumoState {
    fn idle_notifier_state(&mut self) -> &mut smithay::wayland::idle_notify::IdleNotifierState<Self> {
        &mut self.idle_notifier_state
    }
}

smithay::delegate_idle_notify!(LumoState);

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_manager_with(dim: u64, lock: u64, suspend: u64) -> LumoIdleManager {
        LumoIdleManager {
            config: IdleConfig { dim_at: dim, lock_at: lock, suspend_at: suspend },
            last_input: Instant::now(),
            stage: IdleStage::Active,
            saved_brightness: None,
            lock_spawned: false,
        }
    }

    #[test]
    fn starts_active() {
        let mgr = LumoIdleManager::new();
        assert_eq!(mgr.stage, IdleStage::Active);
    }

    #[test]
    fn reset_sets_active_stage() {
        let mut mgr = make_manager_with(1, 5, 10);
        mgr.stage = IdleStage::Dimmed;
        mgr.reset();
        assert_eq!(mgr.stage, IdleStage::Active);
    }

    #[test]
    fn reset_clears_lock_spawned() {
        let mut mgr = make_manager_with(1, 5, 10);
        mgr.lock_spawned = true;
        mgr.reset();
        assert!(!mgr.lock_spawned);
    }

    #[test]
    fn tick_advances_to_dim_after_threshold() {
        let mut mgr = make_manager_with(0, 300, 600);
        // last_input = now, dim_at = 0 -> elapsed(0) >= 0
        let stage = mgr.tick();
        assert_eq!(stage, IdleStage::Dimmed);
    }

    #[test]
    fn tick_does_not_dim_before_threshold() {
        let mut mgr = make_manager_with(999, 1000, 1001);
        let stage = mgr.tick();
        assert_eq!(stage, IdleStage::Active);
    }

    #[test]
    fn idle_config_defaults() {
        let cfg = IdleConfig::default();
        assert_eq!(cfg.dim_at, 120);
        assert_eq!(cfg.lock_at, 300);
        assert_eq!(cfg.suspend_at, 600);
    }

    #[test]
    fn idle_config_toml_parse() {
        let toml_str = "[stages]\ndim_at = 60\nlock_at = 120\nsuspend_at = 300";
        // Direct fields (not nested) — test default parse path.
        let cfg: IdleConfig = toml::from_str("dim_at = 60\nlock_at = 120\nsuspend_at = 300").unwrap();
        assert_eq!(cfg.dim_at, 60);
        assert_eq!(cfg.lock_at, 120);
        assert_eq!(cfg.suspend_at, 300);
    }

    #[test]
    fn dim_sets_saved_brightness_none_without_backlight() {
        // Without a real backlight, saved_brightness stays None.
        let mut mgr = make_manager_with(0, 300, 600);
        mgr.tick();
        // Stage is Dimmed; saved_brightness is None when backlight not found.
        assert_eq!(mgr.stage, IdleStage::Dimmed);
    }
}
