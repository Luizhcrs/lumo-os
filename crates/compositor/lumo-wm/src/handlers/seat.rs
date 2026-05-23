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

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: CursorImageStatus) {
        // W10.C: handle cursor shape requests from wp-cursor-shape-v1 clients.
        match &image {
            CursorImageStatus::Named(icon) => {
                if self.active_cursor_icon == *icon {
                    return;
                }
                self.active_cursor_icon = *icon;
                // Load named xcursor icon from theme.
                let xcursor_name = cursor_icon_to_xcursor_name(*icon);
                if let Some(loaded) = crate::cursor::try_load_named(xcursor_name, 24) {
                    use smithay::backend::allocator::Fourcc;
                    use smithay::backend::renderer::element::memory::MemoryRenderBuffer;
                    use smithay::utils::Transform;
                    let buf = MemoryRenderBuffer::from_slice(
                        &loaded.pixels,
                        Fourcc::Abgr8888,
                        (loaded.width as i32, loaded.height as i32),
                        1,
                        Transform::Normal,
                        None,
                    );
                    self.cursor = Some(loaded);
                    self.cursor_buffer = Some(buf);
                    // W19.4: forca repaint imediato pra cursor icon mudar
                    // no proximo frame (sem esperar vsync pending_flip).
                    #[cfg(feature = "drm-backend")]
                    { self.drm_force_repaint = true; }
                    tracing::debug!(?icon, "W10.C: cursor shape swapped");
                } else {
                    tracing::debug!(?icon, "W10.C: xcursor not found for shape, keeping current");
                }
            }
            CursorImageStatus::Surface(_) => {
                // Client provides custom cursor surface — handled by render pipeline.
                tracing::trace!("cursor_image: Surface (custom)");
            }
            CursorImageStatus::Hidden => {
                // Hide cursor — clear buffer.
                self.cursor_buffer = None;
                tracing::debug!("cursor_image: Hidden");
            }
        }
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
        // W34.13: se app_id vazio (Iced bug) E pid em cache, resolve.
        let (mut app_id, mut title, pid) = (app_id, title, pid);
        if app_id.is_empty() && pid != 0 {
            if let Some((cached_id, cached_title)) = self.pid_app_cache.get(&pid) {
                app_id = cached_id.clone();
                title = cached_title.clone();
                eprintln!("[wm] W34.13 resolved focus app_id={:?} via cache pid={}", app_id, pid);
            }
        }
        tracing::debug!(%app_id, %title, pid, "C5: focus_changed -> ActiveApp broadcast");
        eprintln!("[wm] focus_changed -> ActiveApp app_id={:?} title={:?} pid={} focused_some={}",
            app_id, title, pid, focused.is_some());
        if !app_id.is_empty() {
            self.last_active_app = Some((app_id.clone(), title.clone(), pid));
        } else {
            self.last_active_app = None;
        }
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

/// Maps smithay CursorIcon variants to xcursor theme icon names.
/// W10.C: covers the most common contextual cursors apps request.
pub fn cursor_icon_to_xcursor_name(icon: smithay::input::pointer::CursorIcon) -> &'static str {
    use smithay::input::pointer::CursorIcon;
    match icon {
        CursorIcon::Default    => "default",
        CursorIcon::Text       => "text",
        CursorIcon::Pointer    => "pointer",
        CursorIcon::Move       => "move",
        CursorIcon::Grab       => "grab",
        CursorIcon::Grabbing   => "grabbing",
        CursorIcon::Copy       => "copy",
        CursorIcon::Alias      => "alias",
        CursorIcon::NoDrop     => "no-drop",
        CursorIcon::NotAllowed => "not-allowed",
        CursorIcon::EResize    => "e-resize",
        CursorIcon::NResize    => "n-resize",
        CursorIcon::NeResize   => "ne-resize",
        CursorIcon::NwResize   => "nw-resize",
        CursorIcon::SResize    => "s-resize",
        CursorIcon::SeResize   => "se-resize",
        CursorIcon::SwResize   => "sw-resize",
        CursorIcon::WResize    => "w-resize",
        CursorIcon::EwResize   => "ew-resize",
        CursorIcon::NsResize   => "ns-resize",
        CursorIcon::ColResize  => "col-resize",
        CursorIcon::RowResize  => "row-resize",
        CursorIcon::AllScroll  => "all-scroll",
        CursorIcon::ZoomIn     => "zoom-in",
        CursorIcon::ZoomOut    => "zoom-out",
        CursorIcon::Crosshair  => "crosshair",
        CursorIcon::Wait       => "wait",
        CursorIcon::Progress   => "progress",
        CursorIcon::Help       => "help",
        CursorIcon::ContextMenu => "context-menu",
        CursorIcon::VerticalText => "vertical-text",
        CursorIcon::NeswResize => "nesw-resize",
        CursorIcon::NwseResize => "nwse-resize",
        _ => "default",
    }
}

#[cfg(test)]
mod cursor_shape_tests {
    use super::*;
    use smithay::input::pointer::CursorIcon;

    #[test]
    fn default_icon_maps_to_default() {
        assert_eq!(cursor_icon_to_xcursor_name(CursorIcon::Default), "default");
    }

    #[test]
    fn text_icon_maps_to_text() {
        assert_eq!(cursor_icon_to_xcursor_name(CursorIcon::Text), "text");
    }

    #[test]
    fn pointer_icon_maps_to_pointer() {
        assert_eq!(cursor_icon_to_xcursor_name(CursorIcon::Pointer), "pointer");
    }

    #[test]
    fn resize_icons_map_correctly() {
        assert_eq!(cursor_icon_to_xcursor_name(CursorIcon::EwResize), "ew-resize");
        assert_eq!(cursor_icon_to_xcursor_name(CursorIcon::NsResize), "ns-resize");
    }

    #[test]
    fn fallback_to_default_for_unknown() {
        // AllScroll is defined; test it maps to something.
        let name = cursor_icon_to_xcursor_name(CursorIcon::AllScroll);
        assert!(!name.is_empty());
    }
}

