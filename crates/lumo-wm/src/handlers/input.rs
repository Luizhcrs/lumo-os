//! Input dispatch - converte WinitEvent::Input em eventos pra Seat.
//!
//! Fase 5.4: adiciona keybinds SUPER (logo) interceptados antes do
//! forward pro cliente focado:
//!   SUPER+Q ou SUPER+Return -> spawn `foot`
//!   SUPER+L                 -> sai do compositor (debug)
//!   SUPER+1..9              -> placeholder workspaces (no-op)

use smithay::backend::input::{
    AbsolutePositionEvent, ButtonState, Event as _, InputBackend, InputEvent, KeyState,
    KeyboardKeyEvent, PointerButtonEvent,
};
use smithay::input::keyboard::{FilterResult, Keysym};
use smithay::input::pointer::{ButtonEvent, MotionEvent};
use smithay::utils::SERIAL_COUNTER;

use crate::state::LumoState;

/// Acao interceptada por keybind do compositor. Retornada como
/// FilterResult::Intercept pro evento NAO ser encaminhado ao cliente.
enum Action {
    SpawnFoot,
    Quit,
    Workspace(u8),
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
                        // So intercepta no press; release sempre forward.
                        if !press {
                            return FilterResult::Forward;
                        }
                        if !mods.logo {
                            return FilterResult::Forward;
                        }
                        let sym = kh.modified_sym();
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
                            Keysym::_6 => FilterResult::Intercept(Action::Workspace(6)),
                            Keysym::_7 => FilterResult::Intercept(Action::Workspace(7)),
                            Keysym::_8 => FilterResult::Intercept(Action::Workspace(8)),
                            Keysym::_9 => FilterResult::Intercept(Action::Workspace(9)),
                            _ => FilterResult::Forward,
                        }
                    },
                );
                if let Some(action) = action {
                    match action {
                        Action::SpawnFoot => {
                            // A7 fix bug 3: propaga env explicito pro foot
                            // achar foot.ini (tema Lumo emerald/ink_deep)
                            // mesmo quando lumo-wm e lancado de contexto
                            // sem XDG_CONFIG_HOME populado.
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
                            // Detach: nao queremos zumbi quando foot encerra.
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
                            tracing::info!("SUPER+L -> sair");
                            self.running = false;
                        }
                        Action::Workspace(n) => {
                            tracing::debug!(workspace = n, "switch workspace (no-op)");
                        }
                    }
                }
            }

            InputEvent::PointerMotionAbsolute { event } => {
                // MVP: assume output 1280x720.
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

                // Focus-follows-mouse MVP.
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
