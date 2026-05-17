//! wl_data_device - clipboard / DnD entre clientes.
//!
//! MVP: usa Smithay default behavior, sem hooks custom ainda.

use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
};
use smithay::wayland::selection::SelectionHandler;

use crate::state::LumoState;

impl DataDeviceHandler for LumoState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl SelectionHandler for LumoState {
    type SelectionUserData = ();
}

impl ClientDndGrabHandler for LumoState {}
impl ServerDndGrabHandler for LumoState {}

smithay::delegate_data_device!(LumoState);
