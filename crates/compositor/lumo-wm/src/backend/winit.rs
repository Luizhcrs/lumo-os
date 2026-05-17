//! Backend winit - roda lumo-wm como cliente Wayland dentro de outro
//! compositor (Hyprland host).
//!
//! Etapa 2B (A9): helpers visuais (cursor/cantos/sombras) extraidos
//! pra `backend::render_common` -- mesma visual entre winit nested e
//! drm fullscreen.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::Color32F;
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend};
use smithay::desktop::space::render_output;
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::LoopHandle;
use smithay::utils::Transform;

use crate::state::LumoState;

use super::render_common::{build_overlay, LumoCustomElement, OverlayInputs, CLEAR_INK_DEEP};

const OUTPUT_NAME: &str = "lumo-winit-0";
const REFRESH_MHZ: i32 = 60_000;

pub struct WinitData {
    pub backend: Rc<RefCell<WinitGraphicsBackend<GlesRenderer>>>,
    pub damage_tracker: Rc<RefCell<OutputDamageTracker>>,
    pub output: Output,
}

pub fn init(
    loop_handle: LoopHandle<'static, LumoState>,
    state: &mut LumoState,
) -> Result<WinitData> {
    let (backend, winit_loop) = winit::init::<GlesRenderer>()
        .map_err(|e| anyhow!("falha init winit backend: {e:?}"))?;

    backend.window().set_cursor_visible(false);

    let size = backend.window_size();
    let mode = Mode {
        size,
        refresh: REFRESH_MHZ,
    };

    let output = Output::new(
        OUTPUT_NAME.to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Lumo".into(),
            model: "Winit".into(),
        },
    );
    let _global = output.create_global::<LumoState>(&state.display_handle);
    output.change_current_state(
        Some(mode),
        Some(Transform::Flipped180),
        None,
        Some((0, 0).into()),
    );
    output.set_preferred(mode);
    state.space.map_output(&output, (0, 0));

    let damage_tracker = OutputDamageTracker::from_output(&output);

    let backend = Rc::new(RefCell::new(backend));
    let damage_tracker = Rc::new(RefCell::new(damage_tracker));

    let backend_for_evt = backend.clone();
    let dt_for_evt = damage_tracker.clone();
    let output_for_evt = output.clone();
    loop_handle
        .insert_source(winit_loop, move |event, _, state| match event {
            WinitEvent::Resized { size, .. } => {
                tracing::debug!(?size, "winit resized");
                let mode = Mode {
                    size,
                    refresh: REFRESH_MHZ,
                };
                output_for_evt.change_current_state(Some(mode), None, None, None);
            }
            WinitEvent::Input(input_event) => {
                state.handle_input(input_event);
            }
            WinitEvent::CloseRequested => {
                tracing::info!("winit close requested");
                state.running = false;
            }
            WinitEvent::Redraw => {
                if let Err(err) = redraw(
                    &mut backend_for_evt.borrow_mut(),
                    &mut dt_for_evt.borrow_mut(),
                    &output_for_evt,
                    state,
                ) {
                    tracing::warn!(?err, "Falha no redraw");
                }
            }
            _ => {}
        })
        .map_err(|e| anyhow!("falha ao registrar winit event source: {e}"))?;

    let backend_for_timer = backend.clone();
    let dt_for_timer = damage_tracker.clone();
    let output_for_timer = output.clone();
    loop_handle
        .insert_source(
            Timer::from_duration(Duration::from_millis(16)),
            move |_, _, state| {
                if !state.running {
                    return TimeoutAction::Drop;
                }
                if let Err(err) = redraw(
                    &mut backend_for_timer.borrow_mut(),
                    &mut dt_for_timer.borrow_mut(),
                    &output_for_timer,
                    state,
                ) {
                    tracing::warn!(?err, "Falha no redraw (timer)");
                }
                TimeoutAction::ToDuration(Duration::from_millis(16))
            },
        )
        .map_err(|e| anyhow!("falha ao registrar timer redraw: {e}"))?;

    Ok(WinitData {
        backend,
        damage_tracker,
        output,
    })
}

fn redraw(
    backend: &mut WinitGraphicsBackend<GlesRenderer>,
    damage_tracker: &mut OutputDamageTracker,
    output: &Output,
    state: &mut LumoState,
) -> Result<()> {
    state.frame_counter = state.frame_counter.wrapping_add(1);
    let trace = std::env::var("LUMO_TRACE_FRAMES").is_ok();

    let (renderer, mut framebuffer) = backend
        .bind()
        .map_err(|e| anyhow!("bind framebuffer: {e:?}"))?;

    let space_iter = std::iter::once(&state.space);

    let mode = output.current_mode().unwrap_or(Mode {
        size: (1280, 720).into(),
        refresh: REFRESH_MHZ,
    });
    let (ow, oh) = (mode.size.w, mode.size.h);

    let inputs = OverlayInputs {
        pointer_location: state.pointer_location,
        frame_counter: state.frame_counter,
        cursor: state.cursor.as_ref(),
        cursor_buffer: state.cursor_buffer.as_ref(),
        space: &state.space,
        output_w: ow,
        output_h: oh,
    };
    let overlay = build_overlay(renderer, &inputs);

    let render_result = render_output::<_, LumoCustomElement, _, _>(
        output,
        renderer,
        &mut framebuffer,
        1.0,
        0,
        space_iter,
        &overlay,
        damage_tracker,
        Color32F::new(
            CLEAR_INK_DEEP[0],
            CLEAR_INK_DEEP[1],
            CLEAR_INK_DEEP[2],
            CLEAR_INK_DEEP[3],
        ),
    )
    .map_err(|e| anyhow!("render_output: {e:?}"))?;

    let damage_opt = render_result.damage.cloned();
    drop(framebuffer);

    if let Some(damage) = damage_opt {
        if !damage.is_empty() {
            backend
                .submit(Some(&damage))
                .map_err(|e| anyhow!("submit: {e:?}"))?;
        }
    }

    let time = state.start_time.elapsed();
    let throttle = Some(Duration::from_millis(16));
    let mut sent = 0usize;
    for window in state.space.elements() {
        window.send_frame(output, time, throttle, |_, _| Some(output.clone()));
        sent += 1;
    }
    if trace {
        tracing::debug!(
            frame = state.frame_counter,
            sent_to = sent,
            "frame callbacks dispatched"
        );
    }

    Ok(())
}
