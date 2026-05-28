//! lumo-osd-volume — daemon monitora pactl + dispara OSD volume slider
//! ou mute toggle quando muda.

mod pactl_parse;

use pactl_parse::{parse_mute, parse_volume, VolumeState};
use std::process::Command;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(150);
const PCT_THRESHOLD: u32 = 1;

fn main() {
    lumo_error::hook::install_panic_hook("lumo-osd-volume", lumo_error::Domain::Shell);
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lumo_osd_volume=info".into()),
        )
        .init();

    let mut last = read_state();
    tracing::info!(?last, "lumo-osd-volume start");

    loop {
        std::thread::sleep(POLL_INTERVAL);
        let now = read_state();
        if should_show(last, now) {
            if let Some(state) = now {
                tracing::info!(
                    pct = state.pct,
                    muted = state.muted,
                    "volume OSD spawn (stub)"
                );
                spawn_osd(state);
            }
        }
        last = now;
    }
}

fn read_state() -> Option<VolumeState> {
    let vol_out = Command::new("pactl")
        .args(["get-sink-volume", "@DEFAULT_SINK@"])
        .output()
        .ok()?;
    let mute_out = Command::new("pactl")
        .args(["get-sink-mute", "@DEFAULT_SINK@"])
        .output()
        .ok()?;
    let vol_str = std::str::from_utf8(&vol_out.stdout).ok()?;
    let mute_str = std::str::from_utf8(&mute_out.stdout).ok()?;
    let pct = parse_volume(vol_str)?;
    let muted = parse_mute(mute_str)?;
    Some(VolumeState { pct, muted })
}

fn should_show(prev: Option<VolumeState>, next: Option<VolumeState>) -> bool {
    match (prev, next) {
        (None, Some(_)) => true,
        (Some(p), Some(n)) => p.muted != n.muted || diff_u32(p.pct, n.pct) > PCT_THRESHOLD,
        _ => false,
    }
}

fn diff_u32(a: u32, b: u32) -> u32 {
    if a > b {
        a - b
    } else {
        b - a
    }
}

fn spawn_osd(_state: VolumeState) {
    // TODO M2: layer-shell render slider + mute icon via lumo-osd-framework.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(pct: u32, muted: bool) -> VolumeState {
        VolumeState { pct, muted }
    }

    #[test]
    fn should_show_first_read() {
        assert!(should_show(None, Some(st(50, false))));
    }

    #[test]
    fn should_show_mute_toggle() {
        assert!(should_show(Some(st(50, false)), Some(st(50, true))));
    }

    #[test]
    fn should_show_volume_increase() {
        assert!(should_show(Some(st(50, false)), Some(st(60, false))));
    }

    #[test]
    fn should_not_show_unchanged() {
        assert!(!should_show(Some(st(50, false)), Some(st(50, false))));
    }

    #[test]
    fn should_not_show_tiny_change() {
        assert!(!should_show(Some(st(50, false)), Some(st(51, false))));
    }

    #[test]
    fn should_show_at_threshold_boundary() {
        // 50 -> 52 = diff 2 > 1.
        assert!(should_show(Some(st(50, false)), Some(st(52, false))));
    }

    #[test]
    fn should_show_volume_decrease_with_mute_off() {
        assert!(should_show(Some(st(80, false)), Some(st(60, false))));
    }

    #[test]
    fn should_not_show_when_next_none() {
        assert!(!should_show(Some(st(50, false)), None));
    }

    #[test]
    fn should_not_show_when_both_none() {
        assert!(!should_show(None, None));
    }

    #[test]
    fn diff_u32_basic() {
        assert_eq!(diff_u32(10, 5), 5);
        assert_eq!(diff_u32(5, 10), 5);
        assert_eq!(diff_u32(0, 0), 0);
        assert_eq!(diff_u32(100, 100), 0);
    }
}
