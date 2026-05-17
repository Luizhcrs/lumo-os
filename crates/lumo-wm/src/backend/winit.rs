//! Backend winit - roda lumo-wm como cliente Wayland dentro do Hyprland.
//!
//! Fase 5.4: alem de 5.3 (overlay + cursor + dispatch), oculta o
//! cursor do host na janela winit pra eliminar "cursor duplo".
//!
//! Estrategia (caminho B - decisao A6.3): GlesRenderer continua sendo o
//! renderer "real" pros clientes; overlay Lumo (brand dot + cursor) sai
//! como `SolidColorRenderElement` custom passado pra `render_output`.
//! Sem ponte wgpu->smithay - mantemos lumo-gfx-core focado em UI shell
//! (lumo-bar, gallery), nao em compositor.

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

// Fix 5 (5.4): wrapper pra combinar SolidColor (brand dot + fallback) e
// MemoryRenderBuffer (cursor xcursor real). render_output exige um unico
// tipo C: RenderElement<R>; o macro do Smithay gera o enum + impls.
render_elements! {
    pub LumoCustomElement<R> where R: ImportMem;
    Solid=SolidColorRenderElement,
    Memory=MemoryRenderBufferRenderElement<R>,
}

const OUTPUT_NAME: &str = "lumo-winit-0";
const REFRESH_MHZ: i32 = 60_000;

// Lumo ink_deep (#0a0a0c) em sRGB linear (gamma 2.4 approx)
const CLEAR_INK_DEEP: [f32; 4] = [0.0030, 0.0030, 0.0037, 1.0];

// Lumo emerald (#10b981) -> linear
#[allow(dead_code)]
const BRAND_EMERALD: [f32; 4] = [0.0049, 0.4885, 0.2190, 1.0];

// Cursor cinza claro (#d4d4d8) -> linear
const CURSOR_COLOR: [f32; 4] = [0.6588, 0.6588, 0.6745, 1.0];

#[allow(dead_code)]
const BRAND_DOT_SIZE: i32 = 8;
#[allow(dead_code)]
const BRAND_DOT_MARGIN: i32 = 12;
const CURSOR_W: i32 = 10;
const CURSOR_H: i32 = 14;

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

    // Fix 1 (5.4): oculta cursor do host (Hyprland) dentro da nossa
    // janela. Nosso cursor stub server-side (CURSOR_W x CURSOR_H) eh
    // renderizado pelo lumo-wm; sem esse set_cursor_visible(false) o
    // cursor do host aparece sobreposto = "cursor duplo".
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

/// Brand dot emerald 8x8 fixo no canto top-left.
#[allow(dead_code)]
fn brand_dot_element() -> SolidColorRenderElement {
    let geo: Rectangle<i32, smithay::utils::Physical> = Rectangle::new(
        Point::from((BRAND_DOT_MARGIN, BRAND_DOT_MARGIN)),
        (BRAND_DOT_SIZE, BRAND_DOT_SIZE).into(),
    );
    SolidColorRenderElement::new(
        Id::new(),
        geo,
        0,
        Color32F::new(
            BRAND_EMERALD[0],
            BRAND_EMERALD[1],
            BRAND_EMERALD[2],
            BRAND_EMERALD[3],
        ),
        Kind::Unspecified,
    )
}

/// Cursor server-side fallback: bloco solido 10x14 no pointer_location.
/// Usado quando nao ha tema xcursor disponivel.
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

/// Cursor xcursor real via MemoryRenderBuffer. Hotspot ajusta a
/// posicao pra que a ponta da seta caia exatamente no
/// pointer_location.
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

    // Overlay Lumo (custom elements ficam em cima dos space elements).
    // Cursor xcursor real se carregado; senao fallback SolidColor.
    let mut overlay: Vec<LumoCustomElement<GlesRenderer>> = Vec::with_capacity(2);
    if let Some(elem) = cursor_xcursor_element(renderer, state, 1.0) {
        overlay.push(LumoCustomElement::Memory(elem));
    } else {
        overlay.push(LumoCustomElement::Solid(cursor_solid_fallback(state, 1.0)));
    }
    // A7: brand dot removido do overlay do compositor; agora e responsabilidade
    // do lumo-bar exibir identidade visual. Antes confundia usuario quando bar
    // nao subia (parecia que so o dot estava na tela).

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

    // Frame callbacks pros clientes que pediram.
    // Fix peer-reset: throttle Some(16ms) pra agrupar callbacks. Throttle
    // ZERO em 5.2 fazia flood -> alguns clientes (foot) dropavam
    // callbacks rapidamente e confundiam commit-vs-callback ordering.
    // Smithay docs recomendam throttle = refresh interval do output.
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
