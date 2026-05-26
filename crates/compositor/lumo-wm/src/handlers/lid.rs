//! handlers/lid.rs - Lid switch handler for Galaxy Book 4.
//!
//! Polls /proc/acpi/button/lid/LID*/state every 500ms in a background
//! thread and communicates changes via calloop channel.
//!
//! Behavior:
//!   LID_CLOSE -> dim backlight 50% + start 30s timer
//!   timer fires -> systemctl suspend
//!   LID_OPEN before timer -> cancel timer + restore backlight

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use smithay::reexports::calloop::{channel, LoopHandle};

use crate::state::LumoState;
use lumo_sensors::Backlight;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LidEvent {
    Open,
    Closed,
}

/// Spawns a background polling thread for the lid switch.
/// Registers a calloop channel that delivers LidEvent to the compositor event loop.
pub fn register_lid_watcher(loop_handle: &LoopHandle<'static, LumoState>) {
    let (tx, rx) = channel::channel::<LidEvent>();
    let state_path = find_lid_state_path();
    if state_path.is_none() {
        tracing::warn!("[lid] ACPI lid state not found, handler disabled");
        return;
    }
    let state_path = state_path.unwrap();

    std::thread::spawn(move || {
        let mut last = LidEvent::Open;
        loop {
            let current = read_lid_state(&state_path);
            if current != last {
                last = current;
                if tx.send(current).is_err() {
                    break; // channel closed, compositor exited
                }
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });

    let lid_state = Arc::new(std::sync::Mutex::new(LidHandlerState::default()));
    let lid_state_clone = lid_state.clone();

    loop_handle
        .insert_source(rx, move |event, _, state| {
            if let channel::Event::Msg(lid_event) = event {
                handle_lid_event(lid_event, state, &lid_state_clone);
            }
        })
        .ok();

    tracing::info!("[lid] lid watcher registered");
}

#[derive(Default)]
pub struct LidHandlerState {
    pub closed_at: Option<Instant>,
    pub saved_brightness: Option<u8>,
    pub suspended: bool,
}

fn handle_lid_event(
    event: LidEvent,
    _state: &mut LumoState,
    lid: &Arc<std::sync::Mutex<LidHandlerState>>,
) {
    let mut s = match lid.lock() {
        Ok(g) => g,
        Err(_) => return,
    };
    match event {
        LidEvent::Closed => {
            if s.closed_at.is_some() {
                return; // already handling
            }
            tracing::info!("[lid] closed: dimming backlight 50%, starting 30s suspend timer");
            s.closed_at = Some(Instant::now());
            // Save current brightness and dim to 50%.
            let cur = read_brightness_pct();
            s.saved_brightness = Some(cur);
            s.suspended = false;
            set_brightness_pct(cur / 2);
        }
        LidEvent::Open => {
            if let Some(closed_at) = s.closed_at.take() {
                tracing::info!(
                    "[lid] opened after {}s, cancelling suspend",
                    closed_at.elapsed().as_secs()
                );
                // Restore brightness.
                if let Some(saved) = s.saved_brightness.take() {
                    set_brightness_pct(saved.max(10)); // min 10% so screen is visible
                }
                s.suspended = false;
            }
        }
    }
}

/// Poll the suspend timer — call from compositor main loop tick.
/// Returns true if suspend was triggered.
pub fn tick_lid_timer(lid: &Arc<std::sync::Mutex<LidHandlerState>>) -> bool {
    let mut s = match lid.lock() {
        Ok(g) => g,
        Err(_) => return false,
    };
    if let Some(closed_at) = s.closed_at {
        if !s.suspended && closed_at.elapsed() >= Duration::from_secs(30) {
            tracing::info!("[lid] 30s elapsed, suspending");
            s.suspended = true;
            drop(s);
            trigger_suspend();
            return true;
        }
    }
    false
}

// ============================================================
// sysfs helpers
// ============================================================

fn find_lid_state_path() -> Option<PathBuf> {
    let base = PathBuf::from("/proc/acpi/button/lid");
    if !base.exists() {
        return None;
    }
    for entry in std::fs::read_dir(&base).ok()?.flatten() {
        let p = entry.path().join("state");
        if p.exists() {
            return Some(p);
        }
    }
    None
}

fn read_lid_state(path: &PathBuf) -> LidEvent {
    match std::fs::read_to_string(path) {
        Ok(s) if s.contains("closed") => LidEvent::Closed,
        _ => LidEvent::Open,
    }
}

fn read_brightness_pct() -> u8 {
    let dirs = [
        "/sys/class/backlight/intel_backlight",
        "/sys/class/backlight/amdgpu_bl0",
    ];
    for dir in &dirs {
        let cur = std::fs::read_to_string(format!("{}/brightness", dir))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok());
        let max = std::fs::read_to_string(format!("{}/max_brightness", dir))
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok());
        if let (Some(c), Some(m)) = (cur, max) {
            if m > 0 {
                return ((c as f32 / m as f32) * 100.0).round().clamp(0.0, 100.0) as u8;
            }
        }
    }
    50
}

/// T1.8: usa Backlight::set_percent -- fonte de verdade unica.
fn set_brightness_pct(pct: u8) {
    if let Some(bl) = Backlight::discover() {
        let _ = bl.set_percent(pct);
    }
}

fn trigger_suspend() {
    let _ = std::process::Command::new("systemctl")
        .args(["suspend"])
        .spawn();
}
