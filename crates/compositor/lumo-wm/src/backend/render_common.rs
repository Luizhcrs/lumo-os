//! Elementos de render compartilhados entre backends winit e drm.
//!
//! Etapa 2B (A9): extrai cursor fallback, cursor xcursor, mascara de
//! cantos e sombras pretas pra um modulo unico. Antes ficavam so em
//! winit.rs; agora drm.rs reusa o mesmo pipeline visual (lapidado,
//! sem duplicar codigo).
//!
//! Etapa 2C (A9): adiciona variant `Space=SpaceRenderElements<...>`
//! pra carregar toplevels reais (xdg_shell) + layer-shell (background,
//! bottom, top, overlay) dentro do mesmo enum de elementos. DRM agora
//! renderiza clients de verdade, nao so chrome (cursor/cantos/sombras).
//!
//! Memory feedback_zero_neon_glow: sombras preto/transparente puro,
//! sem nenhum glow colorido. Cantos = quad preto solido.
//! Memory feedback_design_lapidado: cada constante justificada,
//! mesmo valor em DRM e winit (consistencia visual entre Lumo
//! nested e Lumo fullscreen).

use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::{render_elements, Id, Kind};
use smithay::backend::renderer::gles::GlesRenderer;
use smithay::backend::renderer::Color32F;
use smithay::desktop::space::{space_render_elements, SpaceRenderElements};
use smithay::desktop::{Space, Window};
use smithay::output::Output;
use smithay::utils::{Logical, Physical, Point, Rectangle};

use crate::cursor::LoadedCursor;

// Wrapper combinando SolidColor (cursor fallback / sombras / corner mask),
// MemoryRenderBuffer (cursor xcursor real) e SpaceRenderElements
// (toplevels xdg + layer-shell).
//
// Variant Space ja cobre layer-shell upper (top/overlay) + space toplevels
// + layer-shell lower (background/bottom) na ordem certa, porque
// `space_render_elements` faz esse Z-order internamente.
//
// Concreto sobre GlesRenderer porque SpaceRenderElements<R, ...> exige
// R: ImportAll (= ImportMemWl + ImportDmaWl + ImportEgl) no proprio
// enum, e o macro `render_elements!` so propaga bounds pro impl, nao
// pro enum em si. winit e drm usam GlesRenderer mesmo, entao concretizar
// nao perde nada.
render_elements! {
    pub LumoCustomElement<=GlesRenderer>;
    Solid=SolidColorRenderElement,
    Memory=MemoryRenderBufferRenderElement<GlesRenderer>,
    Space=SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
}

// Cursor solid fallback (10x14 cinza claro). Usado quando xcursor falha.
pub const CURSOR_COLOR: [f32; 4] = [0.6588, 0.6588, 0.6745, 1.0];
pub const CURSOR_W: i32 = 10;
pub const CURSOR_H: i32 = 14;

// Moldura desktop: corner radius simulado por quad preto.
pub const CORNER_RADIUS: i32 = 10;
pub const CORNER_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

// Sombras pretas neutras atras de toplevels.
pub const SHADOW_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.4];
pub const SHADOW_OFFSET_Y: i32 = 8;
pub const SHADOW_BLEED: i32 = 4;

// Lumo ink_deep (#0a0a0c) em sRGB linear -- cor de clear do framebuffer.
pub const CLEAR_INK_DEEP: [f32; 4] = [0.0030, 0.0030, 0.0037, 1.0];

pub fn cursor_solid_fallback(
    pointer_location: Point<f64, Logical>,
    frame_counter: u64,
    output_scale: f64,
) -> SolidColorRenderElement {
    let px = (pointer_location.x * output_scale).round() as i32;
    let py = (pointer_location.y * output_scale).round() as i32;
    let geo: Rectangle<i32, Physical> =
        Rectangle::new(Point::from((px, py)), (CURSOR_W, CURSOR_H).into());
    SolidColorRenderElement::new(
        Id::new(),
        geo,
        frame_counter as usize,
        Color32F::new(
            CURSOR_COLOR[0],
            CURSOR_COLOR[1],
            CURSOR_COLOR[2],
            CURSOR_COLOR[3],
        ),
        Kind::Cursor,
    )
}

pub fn cursor_xcursor_element(
    renderer: &mut GlesRenderer,
    pointer_location: Point<f64, Logical>,
    cursor: Option<&LoadedCursor>,
    cursor_buffer: Option<&MemoryRenderBuffer>,
    output_scale: f64,
) -> Option<MemoryRenderBufferRenderElement<GlesRenderer>> {
    let buffer = cursor_buffer?;
    let loaded = cursor?;

    let px = pointer_location.x * output_scale - loaded.hotspot_x as f64;
    let py = pointer_location.y * output_scale - loaded.hotspot_y as f64;

    MemoryRenderBufferRenderElement::from_buffer(
        renderer,
        Point::<f64, Physical>::from((px, py)),
        buffer,
        None,
        None,
        None,
        Kind::Cursor,
    )
    .ok()
}

/// Quatro quads pretos cobrindo os cantos do output -- simula corner
/// radius sem custom shader (memory feedback_design_lapidado:
/// reduzir custo de manutencao do path principal).
pub fn corner_mask_elements(output_w: i32, output_h: i32) -> [SolidColorRenderElement; 4] {
    let r = CORNER_RADIUS;
    let color = Color32F::new(
        CORNER_COLOR[0],
        CORNER_COLOR[1],
        CORNER_COLOR[2],
        CORNER_COLOR[3],
    );
    let make = |x: i32, y: i32| -> SolidColorRenderElement {
        let geo: Rectangle<i32, Physical> =
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

/// Sombras pretas (rgba 0,0,0,0.4) deslocadas +(0,8) atras de cada
/// toplevel. Memory feedback_zero_neon_glow: zero glow colorido.
pub fn shadow_elements(space: &Space<Window>) -> Vec<SolidColorRenderElement> {
    let mut out = Vec::with_capacity(space.elements().count());
    let color = Color32F::new(
        SHADOW_COLOR[0],
        SHADOW_COLOR[1],
        SHADOW_COLOR[2],
        SHADOW_COLOR[3],
    );
    for window in space.elements() {
        let loc = space.element_location(window).unwrap_or_default();
        let geo = window.geometry();
        let shadow_rect = Rectangle::new(
            Point::from((loc.x - SHADOW_BLEED, loc.y + SHADOW_OFFSET_Y - SHADOW_BLEED))
                .to_physical_precise_round(1.0),
            (geo.size.w + SHADOW_BLEED * 2, geo.size.h + SHADOW_BLEED * 2).into(),
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

/// Args agrupados pra build_overlay -- evita conflito de borrow
/// quando o caller ja tem &mut em outro campo de LumoState.
pub struct OverlayInputs<'a> {
    pub pointer_location: Point<f64, Logical>,
    pub frame_counter: u64,
    pub cursor: Option<&'a LoadedCursor>,
    pub cursor_buffer: Option<&'a MemoryRenderBuffer>,
    pub space: &'a Space<Window>,
    pub output_w: i32,
    pub output_h: i32,
}

/// Constroi overlay completo (cursor + corner mask + shadows).
/// Usado pelo backend winit, que ja recebe space_iter separado em
/// `render_output` -- aqui so coletamos chrome (cursor/cantos/sombras).
/// Output em scale 1.0 (HiDPI = futuro).
pub fn build_overlay(
    renderer: &mut GlesRenderer,
    inputs: &OverlayInputs<'_>,
) -> Vec<LumoCustomElement> {
    let mut overlay: Vec<LumoCustomElement> = Vec::with_capacity(16);

    // 1. Cursor (em cima).
    if let Some(elem) = cursor_xcursor_element(
        renderer,
        inputs.pointer_location,
        inputs.cursor,
        inputs.cursor_buffer,
        1.0,
    ) {
        overlay.push(LumoCustomElement::Memory(elem));
    } else {
        overlay.push(LumoCustomElement::Solid(cursor_solid_fallback(
            inputs.pointer_location,
            inputs.frame_counter,
            1.0,
        )));
    }

    // 2. Mascara de cantos do output.
    for elem in corner_mask_elements(inputs.output_w, inputs.output_h) {
        overlay.push(LumoCustomElement::Solid(elem));
    }

    // 3. Sombras das toplevels.
    for elem in shadow_elements(inputs.space) {
        overlay.push(LumoCustomElement::Solid(elem));
    }

    overlay
}

/// Args agrupados pra collect_drm_elements -- evita too_many_arguments
/// e mantem chamada legivel no render_drm.
pub struct DrmCollectInputs<'a> {
    pub space: &'a Space<Window>,
    pub output: &'a Output,
    pub pointer_location: Point<f64, Logical>,
    pub frame_counter: u64,
    pub cursor: Option<&'a LoadedCursor>,
    pub cursor_buffer: Option<&'a MemoryRenderBuffer>,
    pub output_w: i32,
    pub output_h: i32,
}

/// Coleta TODOS elementos pra render direto no DrmCompositor: chrome
/// (cursor, cantos, sombras) + Space (layer-shell + toplevels).
///
/// Ordem (stack baixo -> cima):
/// 1. Mascara dos 4 cantos pretos.
/// 2. Cursor (Kind::Cursor pra DrmCompositor poder mover pra HW plane
///    no futuro -- Etapa 2D).
/// 3. Sombras pretas atras de cada toplevel.
/// 4. SpaceRenderElements: layer top/overlay -> toplevels space ->
///    layer background/bottom (ordem interna garantida pelo smithay).
///
/// Como `render_frame` desenha de tras pra frente recebendo elementos
/// em ordem **front-first**, a lista vai do mais alto pro mais baixo:
/// cursor primeiro, sombras + space depois, cantos por ultimo (cobrem
/// pixels nos 4 cantos). Cor de clear (ink_deep) preenche resto.
pub fn collect_drm_elements(
    renderer: &mut GlesRenderer,
    inputs: &DrmCollectInputs<'_>,
) -> Vec<LumoCustomElement> {
    let mut out: Vec<LumoCustomElement> = Vec::with_capacity(64);

    // 1. Cursor primeiro (mais alto na pilha visual).
    if let Some(elem) = cursor_xcursor_element(
        renderer,
        inputs.pointer_location,
        inputs.cursor,
        inputs.cursor_buffer,
        1.0,
    ) {
        out.push(LumoCustomElement::Memory(elem));
    } else {
        out.push(LumoCustomElement::Solid(cursor_solid_fallback(
            inputs.pointer_location,
            inputs.frame_counter,
            1.0,
        )));
    }

    // 2. Mascara de cantos (cobre pixels dos cantos por cima de tudo
    //    que vier do space, antes do clear preto preencher fora).
    for elem in corner_mask_elements(inputs.output_w, inputs.output_h) {
        out.push(LumoCustomElement::Solid(elem));
    }

    // 3. Sombras pretas atras das toplevels.
    for elem in shadow_elements(inputs.space) {
        out.push(LumoCustomElement::Solid(elem));
    }

    // 4. Toplevels + layer-shell via space_render_elements.
    //    Smithay ja ordena: layer top/overlay -> space windows ->
    //    layer bottom/background (front->back).
    match space_render_elements::<_, Window, _>(
        renderer,
        std::iter::once(inputs.space),
        inputs.output,
        1.0,
    ) {
        Ok(elements) => {
            for el in elements {
                out.push(LumoCustomElement::Space(el));
            }
        }
        Err(err) => {
            tracing::warn!(?err, "space_render_elements falhou no DRM path");
        }
    }

    out
}
