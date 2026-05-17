//! wl_output - anuncia 1 output virtual no MVP nested.

use smithay::wayland::output::OutputHandler;

use crate::state::LumoState;

impl OutputHandler for LumoState {}

smithay::delegate_output!(LumoState);
