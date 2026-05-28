//! lumo-osd-locks — daemon que mostra OSD pra Caps/Num/Scroll Lock.
//!
//! Loop: polling 100ms /sys/class/leds → detect transition →
//! spawn OSD via layer-shell (Wayland) → fade out apos 2s.
//!
//! Roda como child do compositor (herda WAYLAND_DISPLAY).

mod lock_state;
mod sysfs;

use std::time::{Duration, Instant};

use lock_state::{LockState, should_show_osd};
use sysfs::{default_leds_root, read_all};

const POLL_INTERVAL: Duration = Duration::from_millis(100);

fn main() {
    lumo_error::hook::install_panic_hook("lumo-osd-locks", lumo_error::Domain::Shell);
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lumo_osd_locks=info".into()),
        )
        .init();

    tracing::info!("lumo-osd-locks starting (polling /sys/class/leds @ 100ms)");

    #[cfg(unix)]
    {
        run_polling_loop();
    }
    #[cfg(not(unix))]
    {
        eprintln!("lumo-osd-locks: unsupported platform (unix only)");
        std::process::exit(1);
    }
}

#[cfg(unix)]
fn run_polling_loop() {
    let root = default_leds_root();
    let mut last_state = read_all(&root);
    tracing::info!(?last_state, "initial lock state");

    loop {
        std::thread::sleep(POLL_INTERVAL);
        let now_state = read_all(&root);
        let trans = lock_state::diff(&last_state, &now_state);
        if let Some((kind, on)) = should_show_osd(&trans) {
            tracing::info!(?kind, on, "lock transition → OSD spawn");
            // M2: render OSD via layer-shell. Stub por enquanto:
            // logger so. Pra render real precisa SCTK + lumo-osd-framework
            // wire (Galaxy hardware).
            spawn_osd(kind, on);
        }
        last_state = now_state;
    }
}

#[cfg(unix)]
fn spawn_osd(kind: lock_state::LockKind, on: bool) {
    // TODO M2: instanciar layer-shell client + render frame
    // usando lumo_osd_framework::paint + animator.
    // Pra esta sessao sem hardware, log so.
    tracing::warn!(
        kind = ?kind,
        on,
        label = kind.label(),
        "OSD spawn (stub — render via lumo-osd-framework pendente Wayland integration)"
    );
}
