//! wl_seat delegate - input devices (keyboard, pointer, touch).
//!
//! MVP: implementa SeatHandler com cursor sem renderizacao custom.

use smithay::input::keyboard::LedState;
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::Resource; // .id() pra debug log cursor surface
use smithay::wayland::seat::WaylandFocus;

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
    let parts: Vec<&str> = pattern
        .rsplit("/")
        .next()
        .unwrap_or("")
        .splitn(2, "*")
        .collect();
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
                    // Cliente trocou pra Named = sair de custom surface mode.
                    self.cursor_custom_surface = None;
                    // W19.4: forca repaint imediato pra cursor icon mudar
                    // no proximo frame (sem esperar vsync pending_flip).
                    #[cfg(feature = "drm-backend")]
                    {
                        self.drm_force_repaint = true;
                    }
                    tracing::info!(?icon, hotspot = format!("({},{})", self.cursor.as_ref().map(|c| c.hotspot_x).unwrap_or(0), self.cursor.as_ref().map(|c| c.hotspot_y).unwrap_or(0)), "cursor_image::Named swap");
                } else {
                    tracing::warn!(?icon, "cursor_image::Named xcursor NOT FOUND, keeping");
                }
            }
            CursorImageStatus::Surface(s) => {
                tracing::info!(surface_id = ?s.id(), "cursor_image::Surface custom");
                // Cliente (Chrome, Firefox, etc) entrega wl_surface
                // pra renderizar como cursor. Hotspot armazenado em
                // CursorImageSurfaceData no surface.data_map via
                // wl_pointer.set_cursor. Render path compose surface
                // em pointer_location ajustado pelo hotspot.
                // Antes: comment "handled by render pipeline" era falso —
                // render path NAO usava custom surface, mantinha xcursor.
                // Causa mismatch hotspot (Chrome I-beam vs left_ptr) =
                // clicks erravam alvos pequenos.
                self.cursor_custom_surface = Some(s.clone());
                self.cursor_buffer = None;
                #[cfg(feature = "drm-backend")]
                {
                    self.drm_force_repaint = true;
                }
                tracing::debug!("cursor_image: Surface custom adoptada");
            }
            CursorImageStatus::Hidden => {
                // Hide cursor — clear todos buffers.
                self.cursor_buffer = None;
                self.cursor_custom_surface = None;
                tracing::debug!("cursor_image: Hidden");
            }
        }
    }

    fn focus_changed(&mut self, _seat: &Seat<Self>, focused: Option<&WlSurface>) {
        // C5: broadcast ActiveApp a cada troca de foco de teclado.
        // Quando focused=None, envia campos vazios + pid=0 pra bar limpar menubar.
        use lumo_ipc::LumoEvent;
        use smithay::reexports::wayland_server::Resource;
        use smithay::wayland::compositor as wl_compositor;
        use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;
        let (app_id, title, pid) = if let Some(surf) = focused {
            let mut root = surf.clone();
            while let Some(parent) = wl_compositor::get_parent(&root) {
                root = parent;
            }
            let is_mapped = self.space.elements().any(|w| {
                w.wl_surface().map(|s| s.as_ref() == &root).unwrap_or(false)
            });

            if is_mapped {
                // W37.5: le XdgToplevelSurfaceData de ROOT (nao surf). Subsurfaces
                // do toolkit (Iced/winit) nao tem XdgToplevelSurfaceData -> antes
                // retornava app_id vazio e quebrava appmenu.
                let (app_id, title) = wl_compositor::with_states(&root, |states| {
                    if let Some(data) = states.data_map.get::<XdgToplevelSurfaceData>() {
                        let lock = data
                            .lock()
                            .expect("XdgToplevelSurfaceData mutex: nao deve envenenar");
                        (
                            lock.app_id.clone().unwrap_or_default(),
                            lock.title.clone().unwrap_or_default(),
                        )
                    } else {
                        (String::new(), String::new())
                    }
                });
                // pid do root (toplevel), nao da subsurface.
                let pid = root
                    .client()
                    .and_then(|c| c.get_credentials(&self.display_handle).ok())
                    .map(|creds| creds.pid as u32)
                    .unwrap_or(0);
                (app_id, title, pid)
            } else {
                (String::new(), String::new(), 0u32)
            }
        } else {
            (String::new(), String::new(), 0u32)
        };
        // W34.13: se app_id vazio (Iced bug) E pid em cache, resolve.
        let (mut app_id, mut title, pid) = (app_id, title, pid);
        if app_id.is_empty() && pid != 0 {
            if let Some((cached_id, cached_title)) = self.pid_app_cache.get(&pid) {
                app_id = cached_id.clone();
                title = cached_title.clone();
                eprintln!(
                    "[wm] W34.13 resolved focus app_id={:?} via cache pid={}",
                    app_id, pid
                );
            }
        }
        // F1: apps GTK/Qt em Wayland puro (Mousepad/Kate/Chromium) nao
        // registram via appmenu Registrar e podem nao popular xdg_toplevel
        // app_id/title imediatamente. Fallback final: le /proc/<pid>/comm
        // pra ter pelo menos label do binario na bar.
        if app_id.is_empty() && title.is_empty() && pid != 0 {
            let comm_path = format!("/proc/{}/comm", pid);
            if let Ok(comm) = std::fs::read_to_string(&comm_path) {
                let c = comm.trim();
                if !c.is_empty() {
                    title = capitalize_first(c);
                }
            }
        }
        tracing::debug!(%app_id, %title, pid, "C5: focus_changed -> ActiveApp broadcast");
        eprintln!(
            "[wm] focus_changed -> ActiveApp app_id={:?} title={:?} pid={} focused_some={}",
            app_id,
            title,
            pid,
            focused.is_some()
        );
        // W37.5: decisao via fn pura -> evita appmenu piscar em focus events
        // transientes de surfaces internas Iced/winit.
        let last_ref = self
            .last_active_app
            .as_ref()
            .map(|(id, _, p)| (id.as_str(), *p));
        let decision = decide_focus_broadcast(&app_id, pid, last_ref);
        match decision {
            FocusBroadcastDecision::Ignore => {
                eprintln!(
                    "[wm] W37.5 ignora focus_changed app_id='' pid={} == last_pid",
                    pid
                );
            }
            FocusBroadcastDecision::KeepLast => {
                if let Some((last_id, last_title, last_pid)) = self.last_active_app.clone() {
                    eprintln!(
                        "[wm] W37.5 re-broadcast last app={:?} pid={}",
                        last_id, last_pid
                    );
                    self.ipc.broadcast(&LumoEvent::ActiveApp {
                        app_id: last_id,
                        title: last_title,
                        pid: last_pid,
                    });
                }
            }
            FocusBroadcastDecision::Clear => {
                self.last_active_app = None;
                self.ipc.broadcast(&LumoEvent::ActiveApp {
                    app_id: String::new(),
                    title: String::new(),
                    pid: 0,
                });
            }
            FocusBroadcastDecision::Update => {
                // W37.9: app_prefers_csd revertido (gtk3-nocsd suprime CSD na
                // origem via lumo-launch.sh). SSD do Lumo sempre presente.
                self.last_active_app = Some((app_id.clone(), title.clone(), pid));
                self.ipc
                    .broadcast(&LumoEvent::ActiveApp { app_id, title, pid });
            }
        }
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

/// W37.8: apps cujo app_id matche estes patterns preferem CSD propria
/// e nao devem receber SSD do Lumo (evita 2 titlebars empilhadas).
/// Lista pode crescer; idealmente cliente sinaliza via xdg-decoration
/// set_mode(ClientSide), mas Mousepad/Xfce4 nao chamam (criam interface
/// e ficam mudos -> Smithay default ServerSide).
pub fn app_prefers_csd(app_id: &str) -> bool {
    if app_id.is_empty() {
        return false;
    }
    let lower = app_id.to_ascii_lowercase();
    // Xfce4 apps (Mousepad, Thunar, etc).
    if lower.starts_with("org.xfce.") || lower.starts_with("xfce4-") {
        return true;
    }
    // GNOME apps libhandy/libadwaita (header bar CSD).
    if lower.starts_with("org.gnome.") {
        return true;
    }
    false
}

/// W37.5: decisao pura de broadcast pra focus_changed.
/// Recebe (incoming_app_id, incoming_pid, last_active_app) e retorna
/// qual broadcast deve sair (ou Skip se manter estado).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FocusBroadcastDecision {
    /// Limpa appmenu (broadcast empty).
    Clear,
    /// Mantem estado anterior (re-broadcast last).
    KeepLast,
    /// Ignora o evento sem broadcast nenhum (transient toolkit event).
    Ignore,
    /// Novo app focado, broadcast (app_id, title, pid).
    Update,
}

pub fn decide_focus_broadcast(
    incoming_app_id: &str,
    incoming_pid: u32,
    last: Option<(&str, u32)>,
) -> FocusBroadcastDecision {
    if incoming_app_id.is_empty() {
        match last {
            Some((_, last_pid)) if incoming_pid != 0 && incoming_pid == last_pid => {
                // Mesmo pid -> surface transient da mesma janela.
                FocusBroadcastDecision::Ignore
            }
            Some(_) if incoming_pid == 0 => {
                // pid zero (Iced winit internal) -> rebroadcast ultimo.
                FocusBroadcastDecision::KeepLast
            }
            Some(_) => {
                // pid diferente e nao zero -> outro processo sem app_id.
                FocusBroadcastDecision::Clear
            }
            None => FocusBroadcastDecision::Clear,
        }
    } else {
        FocusBroadcastDecision::Update
    }
}

/// Maps smithay CursorIcon variants to xcursor theme icon names.
/// W10.C: covers the most common contextual cursors apps request.
pub fn cursor_icon_to_xcursor_name(icon: smithay::input::pointer::CursorIcon) -> &'static str {
    use smithay::input::pointer::CursorIcon;
    match icon {
        CursorIcon::Default => "default",
        CursorIcon::Text => "text",
        CursorIcon::Pointer => "pointer",
        CursorIcon::Move => "move",
        CursorIcon::Grab => "grab",
        CursorIcon::Grabbing => "grabbing",
        CursorIcon::Copy => "copy",
        CursorIcon::Alias => "alias",
        CursorIcon::NoDrop => "no-drop",
        CursorIcon::NotAllowed => "not-allowed",
        CursorIcon::EResize => "e-resize",
        CursorIcon::NResize => "n-resize",
        CursorIcon::NeResize => "ne-resize",
        CursorIcon::NwResize => "nw-resize",
        CursorIcon::SResize => "s-resize",
        CursorIcon::SeResize => "se-resize",
        CursorIcon::SwResize => "sw-resize",
        CursorIcon::WResize => "w-resize",
        CursorIcon::EwResize => "ew-resize",
        CursorIcon::NsResize => "ns-resize",
        CursorIcon::ColResize => "col-resize",
        CursorIcon::RowResize => "row-resize",
        CursorIcon::AllScroll => "all-scroll",
        CursorIcon::ZoomIn => "zoom-in",
        CursorIcon::ZoomOut => "zoom-out",
        CursorIcon::Crosshair => "crosshair",
        CursorIcon::Wait => "wait",
        CursorIcon::Progress => "progress",
        CursorIcon::Help => "help",
        CursorIcon::ContextMenu => "context-menu",
        CursorIcon::VerticalText => "vertical-text",
        CursorIcon::NeswResize => "nesw-resize",
        CursorIcon::NwseResize => "nwse-resize",
        _ => "default",
    }
}

/// F1: capitaliza primeira letra. Usado como fallback de display
/// quando xdg_toplevel app_id/title vazios — le /proc/<pid>/comm
/// "mousepad" -> "Mousepad".
fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().chain(chars).collect(),
    }
}

#[cfg(test)]
mod capitalize_tests {
    use super::capitalize_first;

    #[test]
    fn empty_returns_empty() {
        assert_eq!(capitalize_first(""), "");
    }

    #[test]
    fn lowercase_first_capitalized() {
        assert_eq!(capitalize_first("mousepad"), "Mousepad");
    }

    #[test]
    fn already_uppercase_unchanged() {
        assert_eq!(capitalize_first("Kate"), "Kate");
    }

    #[test]
    fn unicode_first_capitalized() {
        assert_eq!(capitalize_first("agua"), "Agua");
    }

    #[test]
    fn single_char_capitalized() {
        assert_eq!(capitalize_first("a"), "A");
    }
}

#[cfg(test)]
mod csd_detection_tests {
    use super::app_prefers_csd;

    #[test]
    fn w37_8_xfce4_mousepad_prefer_csd() {
        assert!(app_prefers_csd("org.xfce.Mousepad"));
        assert!(app_prefers_csd("org.xfce.mousepad"));
        assert!(app_prefers_csd("xfce4-mousepad"));
    }

    #[test]
    fn w37_8_xfce4_thunar_prefer_csd() {
        assert!(app_prefers_csd("org.xfce.Thunar"));
    }

    #[test]
    fn w37_8_gnome_apps_prefer_csd() {
        assert!(app_prefers_csd("org.gnome.TextEditor"));
        assert!(app_prefers_csd("org.gnome.Files"));
    }

    #[test]
    fn w37_8_lumo_apps_nao_preferem_csd() {
        // Apps Lumo nativas usam SSD.
        assert!(!app_prefers_csd("com.lumo.files"));
        assert!(!app_prefers_csd("com.lumo.editor"));
    }

    #[test]
    fn w37_8_app_id_vazio_default_ssd() {
        assert!(!app_prefers_csd(""));
    }

    #[test]
    fn w37_8_qt5_apps_default_ssd() {
        // Qt5 honra ServerSide via xdg-decoration; nao precisa heuristic.
        assert!(!app_prefers_csd("org.kde.kate"));
    }
}

#[cfg(test)]
mod focus_broadcast_tests {
    use super::{decide_focus_broadcast, FocusBroadcastDecision};

    #[test]
    fn w37_5_app_id_valido_atualiza() {
        let d = decide_focus_broadcast("com.lumo.files", 100, None);
        assert_eq!(d, FocusBroadcastDecision::Update);
    }

    #[test]
    fn w37_5_app_id_vazio_mesmo_pid_ignora() {
        // Surface transient da mesma janela -> mantem state.
        let d = decide_focus_broadcast("", 100, Some(("com.lumo.files", 100)));
        assert_eq!(d, FocusBroadcastDecision::Ignore);
    }

    #[test]
    fn w37_5_app_id_vazio_pid_zero_keeps_last() {
        // Iced/winit internal surface -> rebroadcast ultimo.
        let d = decide_focus_broadcast("", 0, Some(("com.lumo.files", 100)));
        assert_eq!(d, FocusBroadcastDecision::KeepLast);
    }

    #[test]
    fn w37_5_app_id_vazio_pid_diferente_clear() {
        // Outro processo sem app_id -> limpa.
        let d = decide_focus_broadcast("", 200, Some(("com.lumo.files", 100)));
        assert_eq!(d, FocusBroadcastDecision::Clear);
    }

    #[test]
    fn w37_5_app_id_vazio_sem_last_clear() {
        // Estado inicial sem janela focada.
        let d = decide_focus_broadcast("", 0, None);
        assert_eq!(d, FocusBroadcastDecision::Clear);
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
        assert_eq!(
            cursor_icon_to_xcursor_name(CursorIcon::EwResize),
            "ew-resize"
        );
        assert_eq!(
            cursor_icon_to_xcursor_name(CursorIcon::NsResize),
            "ns-resize"
        );
    }

    #[test]
    fn fallback_to_default_for_unknown() {
        // AllScroll is defined; test it maps to something.
        let name = cursor_icon_to_xcursor_name(CursorIcon::AllScroll);
        assert!(!name.is_empty());
    }
}
