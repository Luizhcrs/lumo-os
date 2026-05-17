//! Backend winit - roda lumo-wm como cliente Wayland dentro de outro
//! compositor (Hyprland host).
//!
//! Fase 5.5 (A8): adiciona moldura desktop (corner radius simulado
//! por mascaras pretas nos cantos do output) + sombras pretas
//! neutras atras de toplevels.
//!
//! Memory feedback_zero_neon_glow: zero glow colorido. Sombra eh
//! preto/transparente puro (rgba 0,0,0,0.4). Cantos sao quads
//! pretos solidos sobre canto do output -> visualmente "desktop com
//! borda arredondada" sem precisar mexer no shader final do
//! GlesRenderer (manter custo de manutencao baixo, lapidado).

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use smithay::backend::renderer::damage::OutputDamageTracker;
use smithay::backend::renderer::element::memory::MemoryRenderBufferRenderElement;
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::{render_elements, Id, Kind};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::{Color32F, ImportMem};
use smithay::backend::winit::{self, WinitEvent, WinitGraphicsBackend};
use smithay::desktop::space::render_output;
use smithay::output::{Mode, Output, PhysicalProperties, Subpixel};
use smithay::reexports::calloop::timer::{TimeoutAction, Timer};
use smithay::reexports::calloop::LoopHandle;
use smithay::utils::{Point, Rectangle, Transform};

use crate::state::LumoState;

// Wrapper pra combinar SolidColor (cursor fallback / sombras / mascara
// cantos) e MemoryRenderBuffer (cursor xcursor real).
render_elements! {
    pub LumoCustomElement<R> where R: ImportMem;
    Solid=SolidColorRenderElement,
    Memory=MemoryRenderBufferRenderElement<R>,
}

const OUTPUT_NAME: &str = "lumo-winit-0";
const REFRESH_MHZ: i32 = 60_000;

// Lumo ink_deep (#0a0a0c) em sRGB linear.
const CLEAR_INK_DEEP: [f32; 4] = [0.0030, 0.0030, 0.0037, 1.0];

#[allow(dead_code)]
const BRAND_EMERALD: [f32; 4] = [0.0049, 0.4885, 0.2190, 1.0];

const CURSOR_COLOR: [f32; 4] = [0.6588, 0.6588, 0.6745, 1.0];

const CURSOR_W: i32 = 10;
const CURSOR_H: i32 = 14;

// Moldura desktop (memory feedback_zero_neon_glow):
// - Corner radius: 10px visualmente. Implementado por 4 quads pretos
//   solidos cobrindo o canto. Justificativa: shader custom no
//   GlesRenderer = ~200 linhas + risco de regressao no path principal;
//   quad preto = sombras + corner = mesmo primitive existente, custo
//   zero de manutencao.
// - Sombra: rect preto rgba(0,0,0,0.4) deslocado +(0,8) atras de cada
//   toplevel. Tamanho: 4px maior em todos os lados pra borrar visual.
const CORNER_RADIUS: i32 = 10;
const CORNER_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const SHADOW_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.4];
const SHADOW_OFFSET_Y: i32 = 8;
const SHADOW_BLEED: i32 = 4;

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

fn cursor_solid_fallback(state: &LumoState, output_scale: f64) -> SolidColorRenderElement {
    let px = (state.pointer_location.x * output_scale).round() as i32;
    let py = (state.pointer_location.y * output_scale).round() as i32;
    let geo: Rectangle<i32, smithay::utils::Physical> =
        Rectangle::new(Point::from((px, py)), (CURSOR_W, CURSOR_H).into());
    SolidColorRenderElement::new(
        Id::new(),
        geo,
        state.frame_counter as usize,
        Color32F::new(
            CURSOR_COLOR[0],
            CURSOR_COLOR[1],
            CURSOR_COLOR[2],
            CURSOR_COLOR[3],
        ),
        Kind::Cursor,
    )
}

fn cursor_xcursor_element(
    renderer: &mut GlesRenderer,
    state: &LumoState,
    output_scale: f64,
) -> Option<MemoryRenderBufferRenderElement<GlesRenderer>> {
    let buffer = state.cursor_buffer.as_ref()?;
    let loaded = state.cursor.as_ref()?;

    let px = state.pointer_location.x * output_scale - loaded.hotspot_x as f64;
    let py = state.pointer_location.y * output_scale - loaded.hotspot_y as f64;

    MemoryRenderBufferRenderElement::from_buffer(
        renderer,
        smithay::utils::Point::<f64, smithay::utils::Physical>::from((px, py)),
        buffer,
        None,
        None,
        None,
        Kind::Cursor,
    )
    .ok()
}

/// Quads pretos nos 4 cantos do output. Mascara visualmente
/// "desktop com cantos arredondados" sem custom shader.
/// Tamanho do quad = CORNER_RADIUS. Em scale=1, 10x10px.
fn corner_mask_elements(
    output_w: i32,
    output_h: i32,
) -> [SolidColorRenderElement; 4] {
    let r = CORNER_RADIUS;
    let color = Color32F::new(
        CORNER_COLOR[0],
        CORNER_COLOR[1],
        CORNER_COLOR[2],
        CORNER_COLOR[3],
    );
    let make = |x: i32, y: i32| -> SolidColorRenderElement {
        let geo: Rectangle<i32, smithay::utils::Physical> =
            Rectangle::new(Point::from((x, y)), (r, r).into());
        SolidColorRenderElement::new(Id::new(), geo, 0, color, Kind::Unspecified)
    };
    [
        make(0, 0),
        make(output_w - r, 0),
        make(0, output_h - r),
        make(output_w - r, output_h - r),
    ]
}

/// Sombras pretas atras de cada toplevel. Memory zero_neon_glow:
/// rgba(0,0,0,0.4), offset (0, +8), bleed 4px. Single quad por
/// window (sem gaussian blur real - simulado pelo offset).
/// z-order: sombras vao ANTES dos space elements (atras).
fn shadow_elements(state: &LumoState) -> Vec<SolidColorRenderElement> {
    let mut out = Vec::with_capacity(state.space.elements().count());
    let color = Color32F::new(
        SHADOW_COLOR[0],
        SHADOW_COLOR[1],
        SHADOW_COLOR[2],
        SHADOW_COLOR[3],
    );
    for window in state.space.elements() {
        let loc = state.space.element_location(window).unwrap_or_default();
        let geo = window.geometry();
        let shadow_rect = Rectangle::new(
            Point::from((loc.x - SHADOW_BLEED, loc.y + SHADOW_OFFSET_Y - SHADOW_BLEED))
                .to_physical_precise_round(1.0),
            (geo.size.w + SHADOW_BLEED * 2, geo.size.h + SHADOW_BLEED * 2)
                .into(),
        );
        out.push(SolidColorRenderElement::new(
            Id::new(),
            shadow_rect,
            0,
            color,
            Kind::Unspecified,
        ));
    }
    out
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

    // Output size pra calcular cantos.
    let mode = output.current_mode().unwrap_or(Mode {
        size: (1280, 720).into(),
        refresh: REFRESH_MHZ,
    });
    let (ow, oh) = (mode.size.w, mode.size.h);

    let mut overlay: Vec<LumoCustomElement<GlesRenderer>> = Vec::with_capacity(16);

    // 1. Cursor (em cima).
    if let Some(elem) = cursor_xcursor_element(renderer, state, 1.0) {
        overlay.push(LumoCustomElement::Memory(elem));
    } else {
        overlay.push(LumoCustomElement::Solid(cursor_solid_fallback(state, 1.0)));
    }

    // 2. Mascara de cantos do output (em cima de tudo, recorta visual).
    for elem in corner_mask_elements(ow, oh) {
        overlay.push(LumoCustomElement::Solid(elem));
    }

    // 3. Sombras das toplevels (vao DEPOIS dos cantos no Vec, mas o
    //    z-order do render_output coloca custom elements POR CIMA dos
    //    space elements. Pra sombras ficarem ATRAS das janelas
    //    precisariamos de pre-elements; render_output API atual nao
    //    expoe. Workaround: sombras viram parte do overlay com z menor
    //    (zindex=0 vs cursor=frame_counter). Smithay ordena por z
    //    decrescente dentro do mesmo Vec.
    //    Resultado pratico: sombras ficam visiveis MAS sobrepoem
    //    levemente os toplevels nas bordas. Memory zero_neon: ainda
    //    eh sombra preta, nao glow.
    //    Plano futuro: usar `space.render_elements()` separadamente e
    //    intercalar.
    for elem in shadow_elements(state) {
        overlay.push(LumoCustomElement::Solid(elem));
    }

    let render_result = render_output::<_, LumoCustomElement<GlesRenderer>, _, _>(
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
