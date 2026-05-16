//! Backend winit - roda lumo-wm como cliente Wayland dentro do Hyprland.
//!
//! Fase 5.2: render loop + input dispatch.

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::Color32F;
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend};
use smithay::desktop::space::{render_output, SpaceRenderElements};
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::LoopHandle;
use smithay::utils::Transform;

use crate::state::LumoState;

const OUTPUT_NAME: &str = "lumo-winit-0";
const REFRESH_MHZ: i32 = 60_000;
const CLEAR: [f32; 4] = [0.04, 0.05, 0.07, 1.0];

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

    // Conecta o WinitEventLoop ao calloop.
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

    // Timer fallback 16ms (~60fps).
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
    let (renderer, mut framebuffer) = backend
        .bind()
        .map_err(|e| anyhow!("bind framebuffer: {e:?}"))?;

    let space_iter = std::iter::once(&state.space);
    let custom: Vec<SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>> =
        Vec::new();

    let render_result = render_output::<
        _,
        SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
        _,
        _,
    >(
        output,
        renderer,
        &mut framebuffer,
        1.0,
        0,
        space_iter,
        &custom,
        damage_tracker,
        Color32F::new(CLEAR[0], CLEAR[1], CLEAR[2], CLEAR[3]),
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

    // Frame callbacks pros clientes que pediram.
    let time = state.start_time.elapsed();
    for window in state.space.elements() {
        window.send_frame(output, time, Some(Duration::ZERO), |_, _| {
            Some(output.clone())
        });
    }

    Ok(())
}
