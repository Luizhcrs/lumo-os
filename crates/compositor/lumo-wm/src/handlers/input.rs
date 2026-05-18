//! Input dispatch - converte eventos de backend em acoes do compositor.
//!
//! B2: KeyboardConfig com 16+ bindings Apple-style carregados de
//! ~/.config/lumo/keyboard.toml (fallback para default_bindings()).
//! Handler handle_input procura match na lista de bindings e executa
//! a acao correspondente.

use smithay::backend::input::{
    AbsolutePositionEvent, ButtonState, Event as _, GestureBeginEvent,
    GestureEndEvent, GesturePinchUpdateEvent, GestureSwipeUpdateEvent,
    InputBackend, InputEvent, KeyState, KeyboardKeyEvent, PointerButtonEvent,
    PointerMotionEvent,
};
use smithay::input::keyboard::FilterResult;
use smithay::input::pointer::{ButtonEvent, MotionEvent};
use smithay::utils::SERIAL_COUNTER;
use smithay::wayland::seat::WaylandFocus;

use crate::input::keyboard::{KeyAction, TileDir};
use crate::input::touchpad::SwipeDirection;
use crate::state::LumoState;

impl LumoState {
    pub fn handle_input<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = event.time_msec();
                let keycode = event.key_code();
                let state = event.state();
                let keyboard = self.keyboard.clone();
                let press = state == KeyState::Pressed;

                // A40: Cell pra capturar sym calculado dentro do closure.
                let last_sym_for_a40 = std::cell::Cell::new(
                    smithay::input::keyboard::xkb::Keysym::NoSymbol
                );
                let action_opt = keyboard.input::<KeyAction, _>(
                    self,
                    keycode,
                    state,
                    serial,
                    time,
                    |state, mods, kh| {
                        if !press {
                            return FilterResult::Forward;
                        }
                        let sym = kh.modified_sym();
                        last_sym_for_a40.set(sym);
                        // Bug Luiz 2026-05-18 v3: caps/num lock LED sync direto
                        // via sysfs — SeatHandler::led_state_changed nao disparou.
                        use smithay::input::keyboard::xkb::Keysym;
                        if sym == Keysym::Caps_Lock {
                            state.caps_lock_on = !state.caps_lock_on;
                            write_sys_led("capslock", state.caps_lock_on);
                        } else if sym == Keysym::Num_Lock {
                            state.num_lock_on = !state.num_lock_on;
                            write_sys_led("numlock", state.num_lock_on);
                        }
                        if let Some(action) =
                            state.keyboard_config.match_binding(mods, sym)
                        {
                            FilterResult::Intercept(action.clone())
                        } else {
                            FilterResult::Forward
                        }
                    },
                );

                if let Some(action) = action_opt {
                    self.execute_key_action(action);
                }
                // A40: Return sem binding + sem toplevel focado
                // -> roteia pra desktop abrir icone selecionado.
                if press && last_sym_for_a40.get() == smithay::input::keyboard::xkb::Keysym::Return {
                    let has_focus = self.keyboard.current_focus().is_some();
                    if !has_focus {
                        tracing::info!("A40: Return sem toplevel -> DesktopOpenSelected");
                        self.broadcast_desktop_open_selected();
                    }
                }
            }

            InputEvent::PointerMotion { event } => {
                let dx = event.delta_x();
                let dy = event.delta_y();
                let new_x = (self.pointer_location.x + dx).clamp(0.0, 1919.0);
                let new_y = (self.pointer_location.y + dy).clamp(0.0, 1079.0);
                self.pointer_location = (new_x, new_y).into();

                let serial = SERIAL_COUNTER.next_serial();
                let under = self.surface_under(self.pointer_location);
                let pointer = self.pointer.clone();
                pointer.motion(
                    self,
                    under.clone().map(|(s, loc)| (s, loc.to_f64())),
                    &MotionEvent {
                        location: self.pointer_location,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);

                #[cfg(feature = "drm-backend")]
                {
                    self.drm_force_repaint = true;
                }
            }

            InputEvent::PointerMotionAbsolute { event } => {
                let x = event.x_transformed(1280);
                let y = event.y_transformed(720);
                self.pointer_location = (x, y).into();

                let serial = SERIAL_COUNTER.next_serial();
                let under = self.surface_under(self.pointer_location);

                let pointer = self.pointer.clone();
                pointer.motion(
                    self,
                    under.clone().map(|(s, loc)| (s, loc.to_f64())),
                    &MotionEvent {
                        location: self.pointer_location,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);

                if let Some((surface, _)) = under {
                    let kb = self.keyboard.clone();
                    if kb.current_focus().as_ref() != Some(&surface) {
                        kb.set_focus(self, Some(surface), serial);
                    }
                }
            }

            InputEvent::PointerButton { event } => {
                let serial = SERIAL_COUNTER.next_serial();
                let button = event.button_code();
                let state: ButtonState = event.state();
                let pointer = self.pointer.clone();

                // C3 debug: log raw button code pra diagnostico BTN_RIGHT.
                tracing::debug!(button, state = ?state, pos = ?(self.pointer_location.x as i32, self.pointer_location.y as i32), "C3 PointerButton");

                if state == ButtonState::Pressed {
                    let kb = self.keyboard.clone();
                    let new_focus = if let Some((surface, _)) = self.surface_under(self.pointer_location) {
                        // L1: FocusManager centraliza policy de foco.
                        use smithay::wayland::shell::xdg::XdgToplevelSurfaceData;
                        let is_toplevel = smithay::wayland::compositor::with_states(
                            &surface,
                            |states| states.data_map.get::<XdgToplevelSurfaceData>().is_some(),
                        );
                        if is_toplevel {
                            self.focus_manager.click_toplevel(surface)
                        } else {
                            // Layer-shell (bar, desktop) -> sem foco de teclado.
                            self.focus_manager.click_layer_shell()
                        }
                    } else {
                        // Area sem surface -> sem foco.
                        self.focus_manager.click_layer_shell()
                    };
                    kb.set_focus(self, new_focus, serial);
                }

                pointer.button(
                    self,
                    &ButtonEvent {
                        button,
                        state,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }

            InputEvent::GestureSwipeBegin { event } => {
                let fingers = event.fingers();
                self.gesture.on_swipe_begin(fingers);
                tracing::debug!(fingers, "gesture swipe begin");
            }

            InputEvent::GestureSwipeUpdate { event } => {
                self.gesture.on_swipe_update(event.delta_x(), event.delta_y());
            }

            InputEvent::GestureSwipeEnd { event } => {
                if let Some((fingers, dir)) = self.gesture.on_swipe_end(event.cancelled()) {
                    self.handle_swipe_gesture(fingers, dir);
                }
            }

            InputEvent::GesturePinchBegin { event } => {
                self.gesture.on_pinch_begin(event.fingers());
                tracing::debug!("gesture pinch begin");
            }

            InputEvent::GesturePinchUpdate { event } => {
                self.gesture.on_pinch_update(event.scale());
            }

            InputEvent::GesturePinchEnd { event } => {
                if let Some(scale) = self.gesture.on_pinch_end(event.cancelled()) {
                    tracing::info!(scale, "gesture pinch end -> forward cliente (futuro)");
                }
            }

            _ => {}
        }
    }

    fn handle_swipe_gesture(&mut self, fingers: u32, dir: SwipeDirection) {
        use lumo_ipc::MAX_WORKSPACES;
        match fingers {
            3 => match dir {
                SwipeDirection::Left => {
                    let next = (self.active_workspace % MAX_WORKSPACES) + 1;
                    tracing::info!(from = self.active_workspace, to = next, "3-finger left -> workspace next");
                    self.set_workspace(next);
                }
                SwipeDirection::Right => {
                    let prev = if self.active_workspace <= 1 {
                        MAX_WORKSPACES
                    } else {
                        self.active_workspace - 1
                    };
                    tracing::info!(from = self.active_workspace, to = prev, "3-finger right -> workspace prev");
                    self.set_workspace(prev);
                }
                SwipeDirection::Up => {
                    tracing::info!("3-finger up -> mission control (stub)");
                }
                SwipeDirection::Down => {
                    tracing::info!("3-finger down -> app expose (stub)");
                }
            },
            4 => {
                tracing::info!(dir = ?dir, "4-finger swipe -> desktop reveal (stub)");
            }
            _ => {
                tracing::debug!(fingers, dir = ?dir, "swipe gesture nao mapeado");
            }
        }
    }

    /// Executa uma KeyAction. Centraliza o dispatch pos-match.
    pub fn execute_key_action(&mut self, action: KeyAction) {
        match action {
            KeyAction::Spawn(cmd) => {
                self.spawn_cmd(&cmd);
            }
            KeyAction::CloseWindow => {
                self.close_focused_window();
            }
            KeyAction::Lock => {
                tracing::info!("lock pendente A40");
            }
            KeyAction::Launcher => {
                tracing::info!("launcher pendente A38");
            }
            KeyAction::Workspace(n) => {
                self.set_workspace(n);
            }
            KeyAction::MoveToWorkspace(n) => {
                tracing::info!(workspace = n, "MoveToWorkspace pendente (sem multi-workspace map)");
            }
            KeyAction::CycleWindow(delta) => {
                // L1: SUPER+Tab -> FocusManager.cycle.
                let serial = smithay::utils::SERIAL_COUNTER.next_serial();
                let kb = self.keyboard.clone();
                let new_focus = self.focus_manager.cycle(&kb, &self.space, delta);
                kb.set_focus(self, new_focus, serial);
            }
            KeyAction::TileMove(dir) => {
                let dir_str = match dir {
                    TileDir::Up    => "Up",
                    TileDir::Down  => "Down",
                    TileDir::Left  => "Left",
                    TileDir::Right => "Right",
                };
                tracing::info!(dir = dir_str, "TileMove pendente (tiling nao implementado)");
            }
            KeyAction::FullscreenToggle => {
                self.toggle_fullscreen_focused();
            }
            KeyAction::Minimize => {
                tracing::info!("minimize pendente (sem iconify protocol)");
            }
            KeyAction::Quit => {
                tracing::info!("Ctrl+Alt+Backspace -> sair");
                self.running = false;
            }
            KeyAction::SwitchVt(n) => {
                #[cfg(feature = "drm-backend")]
                {
                    use smithay::backend::session::Session as _;
                    if let Some(sess) = self.session.as_mut() {
                        if let Err(err) = sess.change_vt(n) {
                            tracing::warn!(vt = n, ?err, "change_vt falhou");
                        } else {
                            tracing::info!(vt = n, "change_vt ok");
                        }
                    } else {
                        tracing::info!(vt = n, "switch_vt request sem session");
                    }
                }
                #[cfg(not(feature = "drm-backend"))]
                {
                    tracing::info!(vt = n, "switch_vt request (no-op fora de DRM)");
                }
            }
        }
    }

    /// Spawna um processo com o ambiente Wayland correto.
    fn spawn_cmd(&self, cmd: &str) {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/root".to_string());
        let xdg = std::env::var("XDG_CONFIG_HOME")
            .unwrap_or_else(|_| format!("{home}/.config"));
        let mut proc = std::process::Command::new(cmd);
        proc.env("HOME", &home);
        proc.env("XDG_CONFIG_HOME", &xdg);
        proc.env("LC_CTYPE", "C.UTF-8");
        if let Some(sock) = self.socket_name.as_deref() {
            proc.env("WAYLAND_DISPLAY", sock);
        }
        if cmd == "foot" {
            proc.arg("-c").arg(format!("{home}/.config/foot/foot.ini"));
        }
        match proc.spawn() {
            Ok(child) => tracing::info!(pid = child.id(), cmd, "spawn ok"),
            Err(err) => tracing::warn!(?err, cmd, "spawn falhou"),
        }
    }

    /// Fecha a janela com foco via xdg_toplevel send_close.
    fn close_focused_window(&self) {
        let kb = self.keyboard.clone();
        if let Some(focused) = kb.current_focus() {
            let window = self
                .space
                .elements()
                .find(|w| {
                    w.wl_surface()
                        .map(|s| *s == focused)
                        .unwrap_or(false)
                })
                .cloned();
            if let Some(win) = window {
                if let Some(toplevel) = win.toplevel() {
                    toplevel.send_close();
                }
            }
        }
    }

    /// Cicla o foco entre janelas no espaco.
    fn cycle_window_focus(&mut self, delta: i8) {
        let windows: Vec<_> = self.space.elements().cloned().collect();
        if windows.is_empty() {
            return;
        }
        let kb = self.keyboard.clone();
        let current = kb.current_focus();
        let current_idx = current.as_ref().and_then(|focused| {
            windows.iter().position(|w| {
                w.wl_surface()
                    .map(|s| *s == *focused)
                    .unwrap_or(false)
            })
        });
        let len = windows.len() as isize;
        let next_idx = match current_idx {
            Some(i) => ((i as isize + delta as isize).rem_euclid(len)) as usize,
            None => 0,
        };
        if let Some(next_win) = windows.get(next_idx) {
            if let Some(surface) = next_win.wl_surface() {
                let serial = SERIAL_COUNTER.next_serial();
                let owned = surface.into_owned();
                kb.set_focus(self, Some(owned), serial);
            }
        }
    }

    /// Alterna fullscreen na janela com foco.
    fn toggle_fullscreen_focused(&self) {
        use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as XdgState;
        let kb = self.keyboard.clone();
        if let Some(focused) = kb.current_focus() {
            let window = self
                .space
                .elements()
                .find(|w| {
                    w.wl_surface()
                        .map(|s| *s == focused)
                        .unwrap_or(false)
                })
                .cloned();
            if let Some(win) = window {
                if let Some(toplevel) = win.toplevel() {
                    let is_fs = toplevel
                        .current_state()
                        .states
                        .contains(XdgState::Fullscreen);
                    toplevel.with_pending_state(|state| {
                        if is_fs {
                            state.states.unset(XdgState::Fullscreen);
                        } else {
                            state.states.set(XdgState::Fullscreen);
                        }
                    });
                    toplevel.send_configure();
                }
            }
        }
    }
}


fn write_sys_led(name: &str, on: bool) {
    let dir = std::path::Path::new("/sys/class/leds");
    let val = if on { b"1" as &[u8] } else { b"0" };
    let suffix = format!("::{}", name);
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let n = e.file_name().to_string_lossy().to_string();
            if n.ends_with(&suffix) {
                let _ = std::fs::write(format!("/sys/class/leds/{}/brightness", n), val);
            }
        }
    }
}
