//! Backend libinput direto - usa /dev/input atraves de sessao libseat.
//!
//! So compila com feature drm-backend (deps input + libseat puxadas la).
//! Reaproveita o trait InputBackend ja implementado por LibinputInputBackend
//! pra que state.handle_input siga o mesmo path do winit.
//!
//! Memory feedback_input_feedback_imediato: eventos sao dispatched a cada
//! ciclo da calloop, sem buffer intermediario. Smithay calloop ja garante
//! ordem e atomicidade.

use anyhow::{anyhow, Result};
use smithay::backend::input::{Event as _, InputEvent};
use smithay::backend::libinput::{LibinputInputBackend, LibinputSessionInterface};
use smithay::backend::session::libseat::LibSeatSession;
use smithay::backend::session::Session;
use smithay::reexports::calloop::LoopHandle;
use smithay::reexports::input as libinput;

use crate::input::TouchpadConfig;
use crate::state::LumoState;

pub fn init(
    session: &LibSeatSession,
    loop_handle: &LoopHandle<'static, LumoState>,
) -> Result<()> {
    let seat_name = session.seat();

    let mut context = libinput::Libinput::new_with_udev::<
        LibinputSessionInterface<LibSeatSession>,
    >(session.clone().into());

    context
        .udev_assign_seat(&seat_name)
        .map_err(|_| anyhow!("udev_assign_seat falhou pra seat {seat_name}"))?;

    tracing::info!(seat = %seat_name, "libinput context iniciado via udev");

    let touchpad_cfg = TouchpadConfig::load();

    let backend = LibinputInputBackend::new(context);

    loop_handle
        .insert_source(backend, move |event, _, state| {
            if let InputEvent::DeviceAdded { ref device } = event {
                let mut d: smithay::reexports::input::Device = device.clone();
                tracing::info!(
                    name = %d.name(),
                    sysname = %d.sysname(),
                    has_pointer = d.has_capability(smithay::reexports::input::DeviceCapability::Pointer),
                    has_kbd = d.has_capability(smithay::reexports::input::DeviceCapability::Keyboard),
                    "libinput DeviceAdded"
                );
                touchpad_cfg.apply_to_device(&mut d);
            }
            if let InputEvent::DeviceRemoved { ref device } = event {
                tracing::warn!(name = %device.name(), "libinput DeviceRemoved");
            }
            if let InputEvent::Keyboard { ref event } = event {
                let now_ms = state.clock.now().as_millis() as u64;
                let evt_ms = event.time_msec() as u64;
                let lag = now_ms.saturating_sub(evt_ms);
                if lag > 100 {
                    tracing::warn!(lag_ms = lag, "input lag > 100ms");
                }
            }
            state.handle_input(event);
        })
        .map_err(|e| anyhow!("insert_source libinput: {e}"))?;

    Ok(())
}
