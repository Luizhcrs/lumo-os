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

use super::render_common::{
    build_winit_elements, clear_color_linear, LumoCustomElement, OverlayInputs,
};
use super::wallpaper::LumoWallpaper;

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
    let (mut backend, winit_loop) = winit::init::<GlesRenderer>()
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

    // A10 frente 1: cria dmabuf-v1 global usando formats reportados pelo
    // EGLContext do renderer. Render node vem do EGLDevice -> dev_id.
    // Falha (driver sem render node, fallback EGL software) loga warn
    // e segue sem o global -- clients GPU caem em SHM.
    {
        use smithay::backend::egl::EGLDevice;
        use smithay::wayland::dmabuf::DmabufFeedbackBuilder;

        let renderer = backend.renderer();
        let egl_context = renderer.egl_context();
        let render_formats: Vec<_> = egl_context.dmabuf_render_formats().iter().copied().collect();
        let egl_display = egl_context.display();

        // Sem feature backend_drm aqui, usamos drm_device_path + stat
        // pra extrair dev_t (rdev) -- mesma info que DrmNode::dev_id
        // mas sem puxar dep drm.
        let dev_id_opt: Option<u64> = EGLDevice::device_for_display(egl_display)
            .ok()
            .and_then(|dev| dev.drm_device_path().ok())
            .and_then(|path| std::fs::metadata(&path).ok())
            .map(|md| {
                use std::os::unix::fs::MetadataExt;
                md.rdev()
            });

        match dev_id_opt {
            Some(dev_id) => {
                let formats_count = render_formats.len();
                match DmabufFeedbackBuilder::new(dev_id, render_formats).build() {
                    Ok(feedback) => {
                        let global = state.dmabuf_state.create_global_with_default_feedback::<LumoState>(
                            &state.display_handle,
                            &feedback,
                        );
                        state.dmabuf_global = Some(global);
                        tracing::info!(
                            dev_id,
                            formats = formats_count,
                            "dmabuf-v1 global criado (winit)"
                        );
                    }
                    Err(err) => {
                        tracing::warn!(?err, "DmabufFeedback build falhou; dmabuf desativado");
                    }
                }
            }
            None => {
                tracing::warn!(
                    "EGLDevice sem render node disponivel; dmabuf desativado (winit fallback)"
                );
            }
        }
    }

    // A19: carrega wallpaper antes de mover backend pra Rc. Path via env
    //      LUMO_WALLPAPER ou $HOME/.config/lumo-wallpaper.jpg. Falha = warn + None.
    let wallpaper = LumoWallpaper::try_load(backend.renderer());
    state.wallpaper = wallpaper;
    // A38: compila shader SDF corner radius.
    state.corner_shader = match crate::backend::corner_shader::CornerShader::compile(backend.renderer()) {
        Ok(cs) => Some(cs),
        Err(e) => { tracing::warn!("corner_shader compile falhou: {:?}", e); None }
    };

    let backend = Rc::new(RefCell::new(backend));
    let damage_tracker = Rc::new(RefCell::new(damage_tracker));

    // Salva handle ao backend pra DmabufHandler conseguir importar.
    state.winit_backend = Some(backend.clone());

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

    let mode = output.current_mode().unwrap_or(Mode {
        size: (1280, 720).into(),
        refresh: REFRESH_MHZ,
    });
    let (ow, oh) = (mode.size.w, mode.size.h);

    // A39: tick boot curtain state.
    if !state.boot_ready && state.boot_clients_ready() {
        state.boot_ready = true;
    }
    if state.boot_ready && state.boot_curtain_alpha > 0.001 {
        state.boot_curtain_alpha = (state.boot_curtain_alpha - 0.067).max(0.0);
    }

    let inputs = OverlayInputs {
        boot_curtain_alpha: state.boot_curtain_alpha,
        wallpaper: state.wallpaper.as_ref(),
        pointer_location: state.pointer_location,
        frame_counter: state.frame_counter,
        cursor: state.cursor.as_ref(),
        cursor_buffer: state.cursor_buffer.as_ref(),
        space: &state.space,
        output_w: ow,
        output_h: oh,
        corner_shader: state.corner_shader.as_ref(),
    };
    // A19: lista combinada (chrome + space + wallpaper). Passamos space iter
    // vazio pra render_output, todos elementos vao via custom_elements --
    // unica forma de wallpaper ficar ATRAS de Space.
    let all_elements = build_winit_elements(renderer, &inputs, output);
    let empty_spaces: std::iter::Empty<&smithay::desktop::Space<smithay::desktop::Window>> =
        std::iter::empty();

    let render_result = render_output::<_, LumoCustomElement, _, _>(
        output,
        renderer,
        &mut framebuffer,
        1.0,
        0,
        empty_spaces,
        &all_elements,
        damage_tracker,
        {
            let c = clear_color_linear();
            Color32F::new(c[0], c[1], c[2], c[3])
        },
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
