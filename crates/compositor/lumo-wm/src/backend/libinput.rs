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

use crate::state::LumoState;

/// Inicializa libinput context vinculado a sessao libseat + udev seat0.
/// Registra como event source na calloop. Eventos chegam em
/// state.handle_input(InputEvent<LibinputInputBackend>) - mesmo path
/// do winit gracas ao trait InputBackend.
pub fn init(
    session: &LibSeatSession,
    loop_handle: &LoopHandle<'static, LumoState>,
) -> Result<()> {
    let seat_name = session.seat();

    // libinput context backed by udev. Vai abrir /dev/input/event* via
    // session.open() (libseat passa fd ja com permissao).
    let mut context = libinput::Libinput::new_with_udev::<
        LibinputSessionInterface<LibSeatSession>,
    >(session.clone().into());

    context
        .udev_assign_seat(&seat_name)
        .map_err(|_| anyhow!("udev_assign_seat falhou pra seat {seat_name}"))?;

    tracing::info!(seat = %seat_name, "libinput context iniciado via udev");

    let backend = LibinputInputBackend::new(context);

    loop_handle
        .insert_source(backend, |event, _, state| {
            // Memory feedback_input_feedback_imediato: warn em lag > 100ms.
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
