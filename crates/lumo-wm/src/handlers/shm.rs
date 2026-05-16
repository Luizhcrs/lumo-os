//! wl_shm delegate - buffers de memoria compartilhada (cliente -> compositor).

use smithay::wayland::shm::{ShmHandler, ShmState};

use crate::state::LumoState;

impl ShmHandler for LumoState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

smithay::delegate_shm!(LumoState);
