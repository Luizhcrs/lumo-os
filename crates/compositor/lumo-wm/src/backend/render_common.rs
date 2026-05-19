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

use super::corner_shader::{CornerShader, RoundedSurfaceElement};
use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::texture::TextureRenderElement;
use smithay::backend::renderer::element::{render_elements, Id, Kind};
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::Color32F;
use smithay::desktop::space::{space_render_elements, SpaceRenderElements};
use smithay::desktop::{Space, Window};
use smithay::wayland::seat::WaylandFocus;
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
    Texture=TextureRenderElement<GlesTexture>,
    Space=SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>,
    Rounded=RoundedSurfaceElement,
}

// M1: SSD titlebar dimensions.
/// Altura da titlebar SSD em pixels logicos.
pub const TITLEBAR_H: i32 = 30;
/// Cor de fundo da titlebar: #1a1a1c (dark neutral).
pub const TITLEBAR_BG: [f32; 4] = [0.098, 0.098, 0.106, 1.0];
/// Cor do botao close: vermelho #c0392b (sem glow — sombra neutra).
pub const CLOSE_BTN_COLOR: [f32; 4] = [0.753, 0.224, 0.169, 1.0];
/// Tamanho do botao close (quadrado).
pub const CLOSE_BTN_SIZE: i32 = 12;
/// Margem direita do botao close.
pub const CLOSE_BTN_MARGIN: i32 = 9;

/// Retorna rect do close button dado o geometry da window no space.
/// Coordenadas em Physical (scale 1.0).
pub fn close_btn_rect(win_loc: smithay::utils::Point<i32, smithay::utils::Logical>, win_w: i32) -> smithay::utils::Rectangle<i32, Physical> {
    let x = win_loc.x + win_w - CLOSE_BTN_SIZE - CLOSE_BTN_MARGIN;
    let y = win_loc.y + (TITLEBAR_H - CLOSE_BTN_SIZE) / 2;
    smithay::utils::Rectangle::new(
        smithay::utils::Point::from((x, y)).to_physical_precise_round(1.0),
        (CLOSE_BTN_SIZE, CLOSE_BTN_SIZE).into(),
    )
}

/// Retorna rect do close button em coordenadas Logicas (para hit-test de input).
pub fn ssd_close_btn_rect_logical(
    win_loc: smithay::utils::Point<i32, smithay::utils::Logical>,
    win_w: i32,
) -> smithay::utils::Rectangle<i32, smithay::utils::Logical> {
    let x = win_loc.x + win_w - CLOSE_BTN_SIZE - CLOSE_BTN_MARGIN;
    let y = win_loc.y - TITLEBAR_H + (TITLEBAR_H - CLOSE_BTN_SIZE) / 2;
    smithay::utils::Rectangle::new(
        smithay::utils::Point::from((x, y)),
        (CLOSE_BTN_SIZE, CLOSE_BTN_SIZE).into(),
    )
}

/// Retorna rect da titlebar inteira em coordenadas Logicas (para hit-test de input).
pub fn ssd_titlebar_rect_logical(
    win_loc: smithay::utils::Point<i32, smithay::utils::Logical>,
    win_w: i32,
) -> smithay::utils::Rectangle<i32, smithay::utils::Logical> {
    smithay::utils::Rectangle::new(
        smithay::utils::Point::from((win_loc.x, win_loc.y - TITLEBAR_H)),
        (win_w, TITLEBAR_H).into(),
    )
}

/// Gera elementos de titlebar (fundo + close button) pra todas SSD windows.
/// Retorna lista front->back: close button na frente, fundo atras.
/// Caller insere esses elementos ANTES do Space na lista de render.
pub fn titlebar_elements(
    space: &Space<Window>,
    ssd_windows: &std::collections::HashSet<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>,
) -> Vec<SolidColorRenderElement> {
    let mut out = Vec::new();
    let bg_color = Color32F::new(TITLEBAR_BG[0], TITLEBAR_BG[1], TITLEBAR_BG[2], TITLEBAR_BG[3]);
    let btn_color = Color32F::new(CLOSE_BTN_COLOR[0], CLOSE_BTN_COLOR[1], CLOSE_BTN_COLOR[2], CLOSE_BTN_COLOR[3]);

    for window in space.elements() {
        let is_ssd = window
            .wl_surface()
            .map(|s| ssd_windows.contains(&*s))
            .unwrap_or(false);
        if !is_ssd {
            continue;
        }
        let loc = space.element_location(window).unwrap_or_default();
        let geo = window.geometry();
        let win_w = geo.size.w;

        // Fundo da titlebar.
        let bar_rect: Rectangle<i32, Physical> = Rectangle::new(
            smithay::utils::Point::from((loc.x, loc.y - TITLEBAR_H))
                .to_physical_precise_round(1.0),
            (win_w, TITLEBAR_H).into(),
        );
        out.push(SolidColorRenderElement::new(
            Id::new(),
            bar_rect,
            0,
            bg_color,
            Kind::Unspecified,
        ));

        // Close button.
        let btn_rect = close_btn_rect(
            smithay::utils::Point::from((loc.x, loc.y - TITLEBAR_H)),
            win_w,
        );
        out.push(SolidColorRenderElement::new(
            Id::new(),
            btn_rect,
            0,
            btn_color,
            Kind::Unspecified,
        ));
    }
    out
}


/// T1.1: gera elementos SolidColor para o menu popup de titlebar SSD.
/// menu_pos = canto top-left do menu em coordenadas logicas.
/// hover_idx = item em hover (usize::MAX = nenhum).
pub fn titlebar_menu_elements(
    menu_pos: smithay::utils::Point<i32, smithay::utils::Logical>,
    hover_idx: usize,
) -> Vec<SolidColorRenderElement> {
    let mut out = Vec::new();
    let menu_w = 180i32;
    let item_h = 22i32;
    let num_items = 5i32;
    let bg_rect: Rectangle<i32, Physical> = Rectangle::new(
        smithay::utils::Point::from((menu_pos.x, menu_pos.y)).to_physical_precise_round(1.0),
        (menu_w, item_h * num_items).into(),
    );
    out.push(SolidColorRenderElement::new(
        Id::new(), bg_rect, 0,
        Color32F::new(0.10, 0.10, 0.11, 0.95), Kind::Unspecified,
    ));
    let sep_y = menu_pos.y + item_h * 3 + item_h / 2 - 1;
    let sep_rect: Rectangle<i32, Physical> = Rectangle::new(
        smithay::utils::Point::from((menu_pos.x + 8, sep_y)).to_physical_precise_round(1.0),
        (menu_w - 16, 1).into(),
    );
    out.push(SolidColorRenderElement::new(
        Id::new(), sep_rect, 0,
        Color32F::new(0.3, 0.3, 0.3, 0.5), Kind::Unspecified,
    ));
    if hover_idx < 5 && hover_idx != 3 {
        let hl_rect: Rectangle<i32, Physical> = Rectangle::new(
            smithay::utils::Point::from((menu_pos.x + 2, menu_pos.y + item_h * hover_idx as i32 + 1)).to_physical_precise_round(1.0),
            (menu_w - 4, item_h - 2).into(),
        );
        out.push(SolidColorRenderElement::new(
            Id::new(), hl_rect, 0,
            Color32F::new(0.24, 0.24, 0.28, 0.90), Kind::Unspecified,
        ));
    }
    out
}

// Cursor solid fallback (10x14 cinza claro). Usado quando xcursor falha.
pub const CURSOR_COLOR: [f32; 4] = [0.6588, 0.6588, 0.6745, 1.0];
pub const CURSOR_W: i32 = 10;
pub const CURSOR_H: i32 = 14;

// Moldura desktop: corner radius simulado por quad pintado na mesma cor
// do clear background. A14: ANTES era preto fixo -> em light theme
// aparecia ponto preto visivel; AGORA acompanha tema via
// `lumo_foundation::corner_mask_color_linear()` runtime.
pub const CORNER_RADIUS: i32 = 10;

/// Radius dos cantos de cada toplevel (A37). Quads pintados com cor
/// do background cobrem os cantos da janela, simulando borda arredondada.
/// Valor 12px.
pub const CORNER_RADIUS_WINDOW: i32 = 12;

/// Cor da mascara de cantos. Runtime (le tema corrente) — necessario
/// porque const eval nao consegue chamar `current_colors()`. Custo
/// desprezivel (4 mults + env lookup por frame, igual `clear_color_linear`).
pub fn corner_color() -> [f32; 4] {
    lumo_foundation::corner_mask_color_linear()
}

// Sombras pretas neutras atras de toplevels (independente de tema).
// Bug Luiz 2026-05-18 v2: sombra solida grande -> sutil. Alpha 0.4 -> 0.15,
// offset 8 -> 3, bleed 4 -> 2. Tiny-skia SolidColor nao tem blur real;
// solucao final eh shader SDF (futuro A37/A38). Por ora suaviza.
pub const SHADOW_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 0.15];
pub const SHADOW_OFFSET_Y: i32 = 3;
pub const SHADOW_BLEED: i32 = 2;

/// Cor de clear do framebuffer. Le LUMO_THEME do env e devolve linear
/// pronto pra surface sRGB. A13: substitui constante `CLEAR_INK_DEEP`
/// pra suportar light/dark dinamico.
///
/// Memory feedback_design_lapidado: nao cachear (chamada acontece a cada
/// frame mas custo eh 1 lookup env + 4 multiplicacoes — desprezivel).
pub fn clear_color_linear() -> [f32; 4] {
    lumo_foundation::clear_color_linear()
}

/// Legacy compat: aponta pra `clear_color_linear()` em runtime. Mantido
/// como nome esperado por call-sites antigos durante migracao.
#[deprecated(note = "use clear_color_linear() dinamico")]
pub fn legacy_clear_ink_deep() -> [f32; 4] {
    clear_color_linear()
}

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

/// Quatro quads cobrindo os cantos do output, pintados na MESMA cor do
/// clear background (theme-aware). Simula corner radius sem custom
/// shader. A14: theme-aware (era preto fixo).
///
/// Memory feedback_design_lapidado: reduzir custo de manutencao do path
/// principal.
pub fn corner_mask_elements(output_w: i32, output_h: i32) -> [SolidColorRenderElement; 4] {
    let r = CORNER_RADIUS;
    let cc = corner_color();
    let color = Color32F::new(cc[0], cc[1], cc[2], cc[3]);
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
    // M2: clip sombra pra fora dos toplevels acima no z-order.
    // Coleta todos os rects de janela (Logical) de uma vez.
    let wins: Vec<(Point<i32, Logical>, Rectangle<i32, Logical>)> = space
        .elements()
        .map(|w| {
            let loc = space.element_location(w).unwrap_or_default();
            let geo = w.geometry();
            (loc, geo)
        })
        .collect();

    let color = Color32F::new(
        SHADOW_COLOR[0],
        SHADOW_COLOR[1],
        SHADOW_COLOR[2],
        SHADOW_COLOR[3],
    );

    let mut out = Vec::new();

    for (idx, (loc, geo)) in wins.iter().enumerate() {
        // Faixa de sombra: so ABAIXO da janela (drop-shadow classico).
        let sx = loc.x - SHADOW_BLEED;
        let sy = loc.y + geo.size.h;
        let sw = geo.size.w + SHADOW_BLEED * 2;
        let sh = SHADOW_OFFSET_Y + SHADOW_BLEED;

        if sw <= 0 || sh <= 0 {
            continue;
        }

        // Inicia com o rect completo, depois clipa contra janelas acima.
        let mut rects: Vec<(i32, i32, i32, i32)> = vec![(sx, sy, sw, sh)];

        // Janelas ACIMA no z-order (back-to-front: indices idx+1.. sao mais altos).
        for (abv_loc, abv_geo) in wins.iter().skip(idx + 1) {
            let occ_x = abv_loc.x;
            let occ_y = abv_loc.y;
            let occ_w = abv_geo.size.w;
            let occ_h = abv_geo.size.h;
            let mut next: Vec<(i32, i32, i32, i32)> = Vec::new();
            for &(rx, ry, rw, rh) in &rects {
                shadow_subtract_rect(rx, ry, rw, rh, occ_x, occ_y, occ_w, occ_h, &mut next);
            }
            rects = next;
            if rects.is_empty() {
                break;
            }
        }

        for (rx, ry, rw, rh) in rects {
            if rw <= 0 || rh <= 0 {
                continue;
            }
            let phys: Rectangle<i32, Physical> = Rectangle::new(
                Point::from((rx, ry)).to_physical_precise_round(1.0),
                (rw, rh).into(),
            );
            out.push(SolidColorRenderElement::new(
                Id::new(),
                phys,
                0,
                color,
                Kind::Unspecified,
            ));
        }
    }
    out
}

/// Subtrai o retangulo oclusor (ox,oy,ow,oh) de (sx,sy,sw,sh).
/// Adiciona os sub-rects restantes (ate 4) em . Coords Logical.
fn shadow_subtract_rect(
    sx: i32, sy: i32, sw: i32, sh: i32,
    ox: i32, oy: i32, ow: i32, oh: i32,
    out: &mut Vec<(i32, i32, i32, i32)>,
) {
    let ix = sx.max(ox);
    let iy = sy.max(oy);
    let ix2 = (sx + sw).min(ox + ow);
    let iy2 = (sy + sh).min(oy + oh);

    if ix >= ix2 || iy >= iy2 {
        // Sem intersecao: source intacto.
        out.push((sx, sy, sw, sh));
        return;
    }

    // Fatia superior (acima da intersecao).
    if iy > sy {
        out.push((sx, sy, sw, iy - sy));
    }
    // Fatia inferior (abaixo da intersecao).
    if iy2 < sy + sh {
        out.push((sx, iy2, sw, (sy + sh) - iy2));
    }
    // Fatia esquerda (faixa vertical da intersecao, lado esquerdo).
    if ix > sx {
        out.push((sx, iy, ix - sx, iy2 - iy));
    }
    // Fatia direita (faixa vertical da intersecao, lado direito).
    if ix2 < sx + sw {
        out.push((ix2, iy, (sx + sw) - ix2, iy2 - iy));
    }
}

/// Quads pintados na cor do background sobre os 4 cantos de cada toplevel.
/// Simula corner radius sem shader customizado (A37 Opcao A).
/// Lista front->back: inserir ANTES do Space (z-order acima do toplevel).
pub fn window_corner_elements(_space: &Space<Window>) -> Vec<SolidColorRenderElement> {
    // Bug Luiz 2026-05-18 v3: mask color era branco no tema light,
    // criando 4 quadrados brancos nas quinas (UX feia). Toplevels voltam
    // quadrados ate A38 implementar shader SDF real com clip transparente.
    Vec::new()
}


/// Converte SpaceRenderElements de toplevel (Element variant) em
/// RoundedSurfaceElement com SDF corner radius (A38).
/// Layer-shell (Surface variant) passa sem wrapper.
fn wrap_space_elements_rounded(
    elements: Vec<SpaceRenderElements<GlesRenderer, WaylandSurfaceRenderElement<GlesRenderer>>>,
    corner_shader: &CornerShader,
) -> Vec<LumoCustomElement> {
    elements
        .into_iter()
        .map(|el| match el {
            SpaceRenderElements::Element(_) => {
                LumoCustomElement::Rounded(RoundedSurfaceElement::new(
                    el,
                    corner_shader.program.clone(),
                    CORNER_RADIUS_WINDOW as f32,
                ))
            }
            _ => LumoCustomElement::Space(el),
        })
        .collect()
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
    /// A38: corner shader opcional. None = sem corner radius.
    pub corner_shader: Option<&'a CornerShader>,
    /// A19: wallpaper opcional. None = clear color de fundo (igual A18).
    pub wallpaper: Option<&'a crate::backend::wallpaper::LumoWallpaper>,
    /// A39: boot curtain alpha. 0.0 = sem cortina; 1.0 = tela preta total.
    pub boot_curtain_alpha: f32,
    /// M1: surfaces SSD para pintar titlebar.
    pub ssd_windows: &'a std::collections::HashSet<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>,
    /// T1.1: menu popup titlebar ativo. None = sem menu.
    pub titlebar_menu: Option<(smithay::utils::Point<i32, smithay::utils::Logical>, usize)>,
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

    // A39: boot curtain full-screen.
    if inputs.boot_curtain_alpha > 0.001 {
        let alpha = inputs.boot_curtain_alpha.clamp(0.0, 1.0);
        let geo: Rectangle<i32, Physical> =
            Rectangle::new(Point::from((0, 0)), (inputs.output_w, inputs.output_h).into());
        overlay.push(LumoCustomElement::Solid(SolidColorRenderElement::new(
            Id::new(),
            geo,
            0,
            Color32F::new(0.0, 0.0, 0.0, alpha),
            Kind::Unspecified,
        )));
        return overlay;
    }

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
    // A19.6: corner_mask removido (quadrados brancos no canto sobrepondo wallpaper)

    // 3. Sombras das toplevels.
    for elem in shadow_elements(inputs.space) {
        overlay.push(LumoCustomElement::Solid(elem));
    }

    // 4. Corner radius: quads sobre cantos de cada toplevel.
    for elem in window_corner_elements(inputs.space) {
        overlay.push(LumoCustomElement::Solid(elem));
    }

    // M1: SSD titlebars (fundo + close button).
    for elem in titlebar_elements(inputs.space, inputs.ssd_windows) {
        overlay.push(LumoCustomElement::Solid(elem));
    }
    // T1.1: menu popup SSD.
    if let Some((menu_pos, hover)) = inputs.titlebar_menu {
        for elem in titlebar_menu_elements(menu_pos, hover) {
            overlay.push(LumoCustomElement::Solid(elem));
        }
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
    /// A38: corner shader opcional.
    pub corner_shader: Option<&'a CornerShader>,
    /// A19: wallpaper opcional (vide OverlayInputs).
    pub wallpaper: Option<&'a crate::backend::wallpaper::LumoWallpaper>,
    /// A39: boot curtain alpha.
    pub boot_curtain_alpha: f32,
    /// M1: surfaces SSD para pintar titlebar.
    pub ssd_windows: &'a std::collections::HashSet<smithay::reexports::wayland_server::protocol::wl_surface::WlSurface>,
    /// T1.1: menu popup titlebar ativo. None = sem menu.
    pub titlebar_menu: Option<(smithay::utils::Point<i32, smithay::utils::Logical>, usize)>,
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

    // A39: boot curtain. Se alpha > 0.001, injeta quad preto full-screen
    // na FRENTE de tudo (lista front-first = primeiro elemento).
    // Early return depois: nao precisa renderizar o resto durante fade.
    if inputs.boot_curtain_alpha > 0.001 {
        let alpha = inputs.boot_curtain_alpha.clamp(0.0, 1.0);
        let geo: Rectangle<i32, Physical> =
            Rectangle::new(Point::from((0, 0)), (inputs.output_w, inputs.output_h).into());
        out.push(LumoCustomElement::Solid(SolidColorRenderElement::new(
            Id::new(),
            geo,
            0,
            Color32F::new(0.0, 0.0, 0.0, alpha),
            Kind::Unspecified,
        )));
        return out;
    }

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
    // A19.6: corner_mask removido (quadrados brancos no canto sobrepondo wallpaper)

    // 3. Sombras pretas atras das toplevels.
    for elem in shadow_elements(inputs.space) {
        out.push(LumoCustomElement::Solid(elem));
    }

    // 3b. Corner radius sobre cantos dos toplevels.
    for elem in window_corner_elements(inputs.space) {
        out.push(LumoCustomElement::Solid(elem));
    }

    // M1: SSD titlebars.
    for elem in titlebar_elements(inputs.space, inputs.ssd_windows) {
        out.push(LumoCustomElement::Solid(elem));
    }
    // T1.1: menu popup SSD.
    if let Some((menu_pos, hover)) = inputs.titlebar_menu {
        for elem in titlebar_menu_elements(menu_pos, hover) {
            out.push(LumoCustomElement::Solid(elem));
        }
    }

    // 4. Toplevels + layer-shell via space_render_elements.
    //    A38: toplevels (Element variant) recebem SDF corner radius.
    //    Layer-shell (Surface variant) passa direto.
    match space_render_elements::<_, Window, _>(
        renderer,
        std::iter::once(inputs.space),
        inputs.output,
        1.0,
    ) {
        Ok(elements) => {
            if let Some(cs) = inputs.corner_shader {
                for el in wrap_space_elements_rounded(elements, cs) {
                    out.push(el);
                }
            } else {
                for el in elements {
                    out.push(LumoCustomElement::Space(el));
                }
            }
        }
        Err(err) => {
            tracing::warn!(?err, "space_render_elements falhou no DRM path");
        }
    }

    // 5. A19: wallpaper por ULTIMO (backmost). Convention smithay: lista
    //    front-first, ultimo elemento eh o mais ao fundo. Wallpaper cobre
    //    output_w x output_h -- clear color so pinta se houver gap (nao tem).
    if let Some(wp) = inputs.wallpaper {
        out.push(LumoCustomElement::Texture(
            wp.element(inputs.output_w, inputs.output_h),
        ));
    }

    out
}


/// A19: pra o winit path injetar wallpaper ATRAS de Space, precisamos
/// construir uma lista combinada (chrome + space + wallpaper) e usar
/// damage_tracker.render_output direto -- nao da pra passar wallpaper
/// via render_output(space, custom_elements) porque custom vem na frente
/// do space.
///
/// Esta funcao retorna a lista pronta na ordem front->back:
///   1. chrome (cursor, cantos, sombras) -- mesma ordem do build_overlay
///   2. Space (toplevels + layer-shell, ja ordenado internamente)
///   3. wallpaper (se presente)
///
/// Caller passa essa lista direto pra damage_tracker.render_output
/// com space iter vazio.
pub fn build_winit_elements(
    renderer: &mut GlesRenderer,
    inputs: &OverlayInputs<'_>,
    output: &Output,
) -> Vec<LumoCustomElement> {
    let mut out: Vec<LumoCustomElement> = Vec::with_capacity(64);

    // A39: boot curtain full-screen.
    if inputs.boot_curtain_alpha > 0.001 {
        let alpha = inputs.boot_curtain_alpha.clamp(0.0, 1.0);
        let geo: Rectangle<i32, Physical> =
            Rectangle::new(Point::from((0, 0)), (inputs.output_w, inputs.output_h).into());
        out.push(LumoCustomElement::Solid(SolidColorRenderElement::new(
            Id::new(),
            geo,
            0,
            Color32F::new(0.0, 0.0, 0.0, alpha),
            Kind::Unspecified,
        )));
        return out;
    }

    // 1. Cursor primeiro (mais alto).
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

    // 2. Mascara de cantos.
    // A19.6: corner_mask removido (quadrados brancos no canto sobrepondo wallpaper)

    // 3. Sombras pretas.
    for elem in shadow_elements(inputs.space) {
        out.push(LumoCustomElement::Solid(elem));
    }

    // 3b. Corner radius sobre cantos dos toplevels.
    for elem in window_corner_elements(inputs.space) {
        out.push(LumoCustomElement::Solid(elem));
    }

    // M1: SSD titlebars.
    for elem in titlebar_elements(inputs.space, inputs.ssd_windows) {
        out.push(LumoCustomElement::Solid(elem));
    }
    // T1.1: menu popup SSD.
    if let Some((menu_pos, hover)) = inputs.titlebar_menu {
        for elem in titlebar_menu_elements(menu_pos, hover) {
            out.push(LumoCustomElement::Solid(elem));
        }
    }

    // 4. Space (toplevels + layer-shell).
    //    A38: toplevels (Element variant) recebem SDF corner radius.
    match space_render_elements::<_, Window, _>(
        renderer,
        std::iter::once(inputs.space),
        output,
        1.0,
    ) {
        Ok(elements) => {
            if let Some(cs) = inputs.corner_shader {
                for el in wrap_space_elements_rounded(elements, cs) {
                    out.push(el);
                }
            } else {
                for el in elements {
                    out.push(LumoCustomElement::Space(el));
                }
            }
        }
        Err(err) => {
            tracing::warn!(?err, "space_render_elements falhou no winit path");
        }
    }

    // 5. Wallpaper no fundo.
    if let Some(wp) = inputs.wallpaper {
        out.push(LumoCustomElement::Texture(
            wp.element(inputs.output_w, inputs.output_h),
        ));
    }

    out
}