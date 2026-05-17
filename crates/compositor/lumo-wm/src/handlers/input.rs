//! Input dispatch - converte WinitEvent::Input em eventos pra Seat.
//!
//! Fase 5.5 (A8): SUPER+1..5 agora chama state.set_workspace(N) e
//! dispara broadcast IPC pra lumo-bar. Antes era no-op.
//!
//! Keybinds:
//!   SUPER+Q ou SUPER+Return  -> spawn `foot`
//!   SUPER+L                  -> sai do compositor (debug)
//!   SUPER+1..5               -> troca workspace ativo + broadcast IPC
//!   Ctrl+Alt+F1..F12         -> switch_vt (so DRM backend; winit ignora)
//!   Ctrl+Alt+Backspace       -> exit clean (DRM safety)

use smithay::backend::input::{
    AbsolutePositionEvent, ButtonState, Event as _, InputBackend, InputEvent, KeyState,
    KeyboardKeyEvent, PointerButtonEvent, PointerMotionEvent,
};
use smithay::input::keyboard::{FilterResult, Keysym};
use smithay::input::pointer::{ButtonEvent, MotionEvent};
use smithay::utils::SERIAL_COUNTER;

use crate::state::LumoState;

/// Acao interceptada por keybind do compositor.
pub enum Action {
    SpawnFoot,
    Quit,
    Workspace(u8),
    SwitchVt(i32),
}

impl LumoState {
    pub fn handle_input<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = event.time_msec();
                let keycode = event.key_code();
                let state = event.state();
                let keyboard = self.keyboard.clone();
                let socket_name = self.socket_name.clone();
                let press = state == KeyState::Pressed;
                let action = keyboard.input::<Action, _>(
                    self,
                    keycode,
                    state,
                    serial,
                    time,
                    |_state, mods, kh| {
                        if !press {
                            return FilterResult::Forward;
                        }
                        let sym = kh.modified_sym();
                        // Ctrl+Alt+F1..F12 -> VT switch (DRM only).
                        // Ctrl+Alt+Backspace -> quit safety.
                        if mods.ctrl && mods.alt {
                            match sym {
                                Keysym::XF86_Switch_VT_1 | Keysym::F1 => {
                                    return FilterResult::Intercept(Action::SwitchVt(1));
                                }
                                Keysym::XF86_Switch_VT_2 | Keysym::F2 => {
                                    return FilterResult::Intercept(Action::SwitchVt(2));
                                }
                                Keysym::XF86_Switch_VT_3 | Keysym::F3 => {
                                    return FilterResult::Intercept(Action::SwitchVt(3));
                                }
                                Keysym::XF86_Switch_VT_4 | Keysym::F4 => {
                                    return FilterResult::Intercept(Action::SwitchVt(4));
                                }
                                Keysym::XF86_Switch_VT_5 | Keysym::F5 => {
                                    return FilterResult::Intercept(Action::SwitchVt(5));
                                }
                                Keysym::XF86_Switch_VT_6 | Keysym::F6 => {
                                    return FilterResult::Intercept(Action::SwitchVt(6));
                                }
                                Keysym::XF86_Switch_VT_7 | Keysym::F7 => {
                                    return FilterResult::Intercept(Action::SwitchVt(7));
                                }
                                Keysym::BackSpace => {
                                    return FilterResult::Intercept(Action::Quit);
                                }
                                _ => {}
                            }
                        }
                        if !mods.logo {
                            return FilterResult::Forward;
                        }
                        match sym {
                            Keysym::q | Keysym::Q | Keysym::Return => {
                                FilterResult::Intercept(Action::SpawnFoot)
                            }
                            Keysym::l | Keysym::L => FilterResult::Intercept(Action::Quit),
                            Keysym::_1 => FilterResult::Intercept(Action::Workspace(1)),
                            Keysym::_2 => FilterResult::Intercept(Action::Workspace(2)),
                            Keysym::_3 => FilterResult::Intercept(Action::Workspace(3)),
                            Keysym::_4 => FilterResult::Intercept(Action::Workspace(4)),
                            Keysym::_5 => FilterResult::Intercept(Action::Workspace(5)),
                            _ => FilterResult::Forward,
                        }
                    },
                );
                if let Some(action) = action {
                    match action {
                        Action::SpawnFoot => {
                            let home = std::env::var("HOME")
                                .unwrap_or_else(|_| "/root".to_string());
                            let xdg = std::env::var("XDG_CONFIG_HOME")
                                .unwrap_or_else(|_| format!("{home}/.config"));
                            let foot_cfg = format!("{home}/.config/foot/foot.ini");
                            let mut cmd = std::process::Command::new("foot");
                            cmd.arg("-c").arg(&foot_cfg);
                            cmd.env("HOME", &home);
                            cmd.env("XDG_CONFIG_HOME", &xdg);
                            cmd.env("LC_CTYPE", "C.UTF-8");
                            if let Some(sock) = socket_name.as_deref() {
                                cmd.env("WAYLAND_DISPLAY", sock);
                            }
                            match cmd.spawn() {
                                Ok(child) => {
                                    tracing::info!(pid = child.id(), "spawn foot");
                                }
                                Err(err) => {
                                    tracing::warn!(?err, "Falha spawn foot");
                                }
                            }
                        }
                        Action::Quit => {
                            tracing::info!("SUPER+L / Ctrl+Alt+Backspace -> sair");
                            self.running = false;
                        }
                        Action::Workspace(n) => {
                            self.set_workspace(n);
                        }
                        Action::SwitchVt(n) => {
                            // DRM real: chama session.change_vt(n). Se nao
                            // tem session (winit) so loga.
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

                // Forca repaint pro cursor mover visualmente no proximo frame
                self.drm_force_repaint = true;
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

                if state == ButtonState::Pressed {
                    if let Some((surface, _)) = self.surface_under(self.pointer_location) {
                        let kb = self.keyboard.clone();
                        kb.set_focus(self, Some(surface), serial);
                    }
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

            _ => {}
        }
    }
}
