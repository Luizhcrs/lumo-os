//! lumo-osd-brightness — daemon que monitora /sys/class/backlight e dispara
//! OSD slider quando user ajusta brilho (XF86MonBrightnessUp/Down keys).
//!
//! Polling 100ms. Diff de valor: se >2% mudou, spawn OSD via framework.

mod backlight;

use backlight::{default_root, read_first, BacklightState};
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(100);
/// Diff threshold pra dispara OSD (evita spam fluctuation 1-tick).
const PCT_THRESHOLD: f32 = 1.0;

fn main() {
    lumo_error::hook::install_panic_hook("lumo-osd-brightness", lumo_error::Domain::Shell);
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lumo_osd_brightness=info".into()),
        )
        .init();

    let root = default_root();
    let mut last = read_first(&root);
    tracing::info!(?last, "lumo-osd-brightness inicio");

    loop {
        std::thread::sleep(POLL_INTERVAL);
        let now = read_first(&root);
        if should_show(last, now) {
            if let Some(state) = now {
                tracing::info!(
                    pct = state.pct(),
                    current = state.current,
                    max = state.max,
                    "brightness OSD spawn (stub)"
                );
                // M2: render OSD via lumo-osd-framework (Galaxy needed).
                spawn_osd(state);
            }
        }
        last = now;
    }
}

/// Decide se OSD deve aparecer: only quando diff pct > threshold.
fn should_show(prev: Option<BacklightState>, next: Option<BacklightState>) -> bool {
    match (prev, next) {
        (None, Some(_)) => true,
        (Some(p), Some(n)) => (p.pct() - n.pct()).abs() > PCT_THRESHOLD,
        _ => false,
    }
}

fn spawn_osd(_state: BacklightState) {
    // TODO M2: layer-shell client + slider draw via lumo-osd-framework.
}

#[cfg(test)]
mod tests {
    use super::*;

    fn st(c: u32, m: u32) -> BacklightState {
        BacklightState { current: c, max: m }
    }

    #[test]
    fn should_show_first_read() {
        assert!(should_show(None, Some(st(50, 100))));
    }

    #[test]
    fn should_show_significant_change() {
        assert!(should_show(Some(st(50, 100)), Some(st(70, 100))));
    }

    #[test]
    fn should_not_show_tiny_change() {
        // 50% → 50.5% = 0.5% diff < 1% threshold.
        assert!(!should_show(Some(st(500, 1000)), Some(st(505, 1000))));
    }

    #[test]
    fn should_not_show_unchanged() {
        assert!(!should_show(Some(st(50, 100)), Some(st(50, 100))));
    }

    #[test]
    fn should_not_show_when_state_disappears() {
        assert!(!should_show(Some(st(50, 100)), None));
    }

    #[test]
    fn should_not_show_when_both_none() {
        assert!(!should_show(None, None));
    }

    #[test]
    fn should_show_decrease_significant() {
        assert!(should_show(Some(st(80, 100)), Some(st(60, 100))));
    }

    #[test]
    fn should_show_at_threshold_boundary() {
        // 50% → 51.5% = 1.5% diff > 1% threshold = show.
        assert!(should_show(Some(st(500, 1000)), Some(st(515, 1000))));
    }
}
