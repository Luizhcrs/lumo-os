//! lumo-osd — daemon unico OSD: locks + brightness + volume.
//!
//! A4 review: substitui 3 bins (lumo-osd-locks, brightness, volume) por 1
//! processo com polling unificado e arbitragem de prioridade.

use lumo_osd::dispatch::{brightness_should_show, pick_event, volume_should_show, OsdEvent};
use lumo_osd::sources::backlight::{default_root as backlight_root, read_first as read_backlight};
use lumo_osd::sources::lock_state::{diff as lock_diff, should_show_osd as lock_should_show};
use lumo_osd::sources::locks_sysfs::{default_leds_root, read_all as read_leds};
use std::process::Command;
use std::time::Duration;

const POLL_INTERVAL: Duration = Duration::from_millis(100);

fn main() {
    #[cfg(unix)]
    {
        lumo_error::hook::install_panic_hook("lumo-osd", lumo_error::Domain::Shell);
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "lumo_osd=info".into()),
            )
            .init();
    }
    tracing::info!("lumo-osd starting (consolidated: locks+brightness+volume @ 100ms)");

    let leds_root = default_leds_root();
    let backlight_root = backlight_root();
    let mut last_leds = read_leds(&leds_root);
    let mut last_brightness = read_backlight(&backlight_root);
    let mut last_volume = read_volume_state();

    loop {
        std::thread::sleep(POLL_INTERVAL);

        let now_leds = read_leds(&leds_root);
        let now_brightness = read_backlight(&backlight_root);
        let now_volume = read_volume_state();

        let mut candidates: Vec<OsdEvent> = Vec::new();

        let trans = lock_diff(&last_leds, &now_leds);
        if let Some((kind, on)) = lock_should_show(&trans) {
            candidates.push(OsdEvent::Lock { kind, on });
        }

        if brightness_should_show(last_brightness, now_brightness) {
            if let Some(state) = now_brightness {
                candidates.push(OsdEvent::Brightness(state));
            }
        }

        if volume_should_show(last_volume, now_volume) {
            if let Some(state) = now_volume {
                candidates.push(OsdEvent::Volume(state));
            }
        }

        if let Some(ev) = pick_event(candidates) {
            spawn_osd(&ev);
        }

        last_leds = now_leds;
        last_brightness = now_brightness;
        last_volume = now_volume;
    }
}

fn read_volume_state() -> Option<lumo_osd::sources::pactl_parse::VolumeState> {
    use lumo_osd::sources::pactl_parse::{parse_mute, parse_volume, VolumeState};
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

fn spawn_osd(ev: &OsdEvent) {
    // M2: render real via lumo-osd-framework + layer-shell. Galaxy needed.
    tracing::info!(?ev, "OSD event ready (render via framework pendente)");
}
