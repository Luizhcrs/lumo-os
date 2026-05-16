//! Input dispatch - converte WinitEvent::Input em eventos pra Seat.

use smithay::backend::input::{
    AbsolutePositionEvent, ButtonState, Event as _, InputBackend, InputEvent,
    KeyboardKeyEvent, PointerButtonEvent,
};
use smithay::input::keyboard::FilterResult;
use smithay::input::pointer::{ButtonEvent, MotionEvent};
use smithay::utils::SERIAL_COUNTER;

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
                keyboard.input::<(), _>(
                    self,
                    keycode,
                    state,
                    serial,
                    time,
                    |_state, _mods, _kh| FilterResult::Forward,
                );
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
