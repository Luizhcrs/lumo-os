//! wl_seat delegate - input devices (keyboard, pointer, touch).
//!
//! MVP: implementa SeatHandler com cursor sem renderizacao custom.

use smithay::input::keyboard::LedState;
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

use crate::state::LumoState;

/// Sync LED hardware via /sys/class/leds/input*::{capslock,numlock,scrolllock}/brightness.
/// Bug Luiz 2026-05-18: Caps Lock nao acendia LED do teclado.
fn write_led(name: &str, on: bool) {
    let pattern = format!("/sys/class/leds/input*::{}", name);
    let val = if on { b"1" as &[u8] } else { b"0" };
    if let Ok(paths) = glob_simple(&pattern) {
        for p in paths {
            let _ = std::fs::write(format!("{}/brightness", p), val);
        }
    }
}

fn glob_simple(pattern: &str) -> Result<Vec<String>, std::io::Error> {
    let dir = std::path::Path::new("/sys/class/leds");
    let parts: Vec<&str> = pattern.rsplit("/").next().unwrap_or("").splitn(2, "*").collect();
    let prefix = parts.get(0).copied().unwrap_or("");
    let suffix = parts.get(1).copied().unwrap_or("");
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with(prefix) && name.ends_with(suffix) {
            out.push(format!("/sys/class/leds/{}", name));
        }
    }
    Ok(out)
}

impl SeatHandler for LumoState {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: CursorImageStatus) {
        // Cursor rendering entra na Fase 5.3 (lumo-gfx integration).
    }

    fn focus_changed(&mut self, _seat: &Seat<Self>, focused: Option<&WlSurface>) {
        // C5: broadcast ActiveApp a cada troca de foco de teclado.
        // Quando focused=None, envia campos vazios + pid=0 pra bar limpar menubar.
        use smithay::reexports::wayland_server::Resource;
        use smithay::wayland::compositor as wl_compositor;
        use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;
        use lumo_ipc::LumoEvent;
        let (app_id, title, pid) = if let Some(surf) = focused {
            let (app_id, title) = wl_compositor::with_states(surf, |states| {
                if let Some(data) = states.data_map.get::<XdgToplevelSurfaceData>() {
                    let lock = data.lock().expect("XdgToplevelSurfaceData mutex: nao deve envenenar");
                    (
                        lock.app_id.clone().unwrap_or_default(),
                        lock.title.clone().unwrap_or_default(),
                    )
                } else {
                    (String::new(), String::new())
                }
            });
            let pid = surf
                .client()
                .and_then(|c| c.get_credentials(&self.display_handle).ok())
                .map(|creds| creds.pid as u32)
                .unwrap_or(0);
            (app_id, title, pid)
        } else {
            (String::new(), String::new(), 0u32)
        };
        tracing::debug!(%app_id, %title, pid, "C5: focus_changed -> ActiveApp broadcast");
        self.ipc.broadcast(&LumoEvent::ActiveApp { app_id, title, pid });
    }

    fn led_state_changed(&mut self, _seat: &Seat<Self>, led_state: LedState) {
        // Bug Luiz 2026-05-18: Caps Lock nao acendia LED.
        // smithay propaga LedState do xkbcommon; gravar em sysfs reflete em HW.
        write_led("capslock", led_state.caps.unwrap_or(false));
        write_led("numlock", led_state.num.unwrap_or(false));
        write_led("scrolllock", led_state.scroll.unwrap_or(false));
    }
}

smithay::delegate_seat!(LumoState);
