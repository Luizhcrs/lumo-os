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

use super::corner_shader::{CornerMaskShader, CornerShader, RoundedSurfaceElement};
use smithay::backend::renderer::element::memory::{
    MemoryRenderBuffer, MemoryRenderBufferRenderElement,
};
use smithay::backend::renderer::element::solid::SolidColorRenderElement;
use smithay::backend::renderer::element::surface::WaylandSurfaceRenderElement;
use smithay::backend::renderer::element::texture::TextureRenderElement;
use smithay::backend::renderer::element::{render_elements, Id, Kind};
use smithay::backend::renderer::gles::element::PixelShaderElement;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::Color32F;
use smithay::desktop::space::{space_render_elements, SpaceRenderElements};
use smithay::desktop::{Space, Window};
use smithay::output::Output;
use smithay::utils::{Logical, Physical, Point, Rectangle};
use smithay::wayland::seat::WaylandFocus;

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
    Pixel=PixelShaderElement,
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

/// W17.1: cor botao minimize (amarelo #F39C12).
pub const BTN_MIN_COLOR: [f32; 4] = [0.953, 0.612, 0.071, 1.0];
/// W17.1: cor botao maximize (verde #2ECC71).
pub const BTN_MAX_COLOR: [f32; 4] = [0.180, 0.800, 0.443, 1.0];
/// W17.1: gap horizontal entre botoes (margem visual).
pub const BTN_GAP: i32 = 4;

/// Retorna rect do close button dado o geometry da window no space.
/// Coordenadas em Physical (scale 1.0).
pub fn close_btn_rect(
    win_loc: smithay::utils::Point<i32, smithay::utils::Logical>,
    win_w: i32,
) -> smithay::utils::Rectangle<i32, Physical> {
    // W24.4: Protecao contra overflow em janelas gigantes.
    let win_w = win_w.clamp(0, 4096);
    let x = win_loc.x.saturating_add(win_w).saturating_sub(CLOSE_BTN_SIZE).saturating_sub(CLOSE_BTN_MARGIN);
    let y = win_loc.y.saturating_add((TITLEBAR_H - CLOSE_BTN_SIZE) / 2);
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
    let win_w = win_w.clamp(0, 4096);
    let x = win_loc.x.saturating_add(win_w).saturating_sub(CLOSE_BTN_SIZE).saturating_sub(CLOSE_BTN_MARGIN);
    let y = win_loc.y.saturating_sub(TITLEBAR_H).saturating_add((TITLEBAR_H - CLOSE_BTN_SIZE) / 2);
    smithay::utils::Rectangle::new(
        smithay::utils::Point::from((x, y)),
        (CLOSE_BTN_SIZE, CLOSE_BTN_SIZE).into(),
    )
}

/// W17.1: rect do botao maximize (esquerda do close) em coordenadas Logicas.
pub fn ssd_max_btn_rect_logical(
    win_loc: smithay::utils::Point<i32, smithay::utils::Logical>,
    win_w: i32,
) -> smithay::utils::Rectangle<i32, smithay::utils::Logical> {
    let win_w = win_w.clamp(0, 4096);
    let x = win_loc.x.saturating_add(win_w).saturating_sub(CLOSE_BTN_SIZE * 2).saturating_sub(CLOSE_BTN_MARGIN).saturating_sub(BTN_GAP);
    let y = win_loc.y.saturating_sub(TITLEBAR_H).saturating_add((TITLEBAR_H - CLOSE_BTN_SIZE) / 2);
    smithay::utils::Rectangle::new(
        smithay::utils::Point::from((x, y)),
        (CLOSE_BTN_SIZE, CLOSE_BTN_SIZE).into(),
    )
}

/// W17.1: rect do botao minimize (esquerda do maximize) em coordenadas Logicas.
pub fn ssd_min_btn_rect_logical(
    win_loc: smithay::utils::Point<i32, smithay::utils::Logical>,
    win_w: i32,
) -> smithay::utils::Rectangle<i32, smithay::utils::Logical> {
    let win_w = win_w.clamp(0, 4096);
    let x = win_loc.x.saturating_add(win_w).saturating_sub(CLOSE_BTN_SIZE * 3).saturating_sub(CLOSE_BTN_MARGIN).saturating_sub(BTN_GAP * 2);
    let y = win_loc.y.saturating_sub(TITLEBAR_H).saturating_add((TITLEBAR_H - CLOSE_BTN_SIZE) / 2);
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
    let win_w = win_w.clamp(0, 4096);
    smithay::utils::Rectangle::new(
        smithay::utils::Point::from((win_loc.x, win_loc.y.saturating_sub(TITLEBAR_H))),
        (win_w, TITLEBAR_H).into(),
    )
}

/// Gera elementos de titlebar (fundo + close button) pra todas SSD windows.
/// Retorna lista front->back: close button na frente, fundo atras.
/// Caller insere esses elementos ANTES do Space na lista de render.
/// W29.4: titlebar btns pra UMA window. Per-window helper pra interleave Z.
pub fn titlebar_btns_for_window(
    window: &Window,
    space: &Space<Window>,
    ssd_windows: &std::collections::HashSet<
        smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    >,
) -> Vec<SolidColorRenderElement> {
    let single = [window];
    let mut out = Vec::new();
    let bg_color = Color32F::new(
        TITLEBAR_BG[0],
        TITLEBAR_BG[1],
        TITLEBAR_BG[2],
        TITLEBAR_BG[3],
    );
    let btn_color = Color32F::new(
        CLOSE_BTN_COLOR[0],
        CLOSE_BTN_COLOR[1],
        CLOSE_BTN_COLOR[2],
        CLOSE_BTN_COLOR[3],
    );
    for window in single.iter().copied() {
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
        // W32.1: skip se janela ainda nao configurada (size<200 = ack inicial pendente)
        if win_w < 200 {
            continue;
        }
        // W24.6: Ajusta loc com offset interno da geometria (geo.loc)
        let actual_loc = loc + geo.loc;

        // W37.7: BG da titlebar full-width. Antes faltava -> SSD aparecia
        // como botoes + titulo flutuante isolados, conteudo da janela
        // vazava por baixo (bug "barra errada" no Mousepad).
        // NOTA: Push order em smithay - PRIMEIRO elem renderiza POR CIMA
        // (front). bg vai z=1 + pushed APOS botoes pra renderizar atras.
        let tb_bg_rect: Rectangle<i32, Physical> = Rectangle::new(
            smithay::utils::Point::from((actual_loc.x, actual_loc.y - TITLEBAR_H))
                .to_physical_precise_round(1.0),
            (win_w, TITLEBAR_H).into(),
        );

        let btn_rect = close_btn_rect(
            smithay::utils::Point::from((actual_loc.x, actual_loc.y - TITLEBAR_H)),
            win_w,
        );
        out.push(SolidColorRenderElement::new(
            Id::new(),
            btn_rect,
            0,
            btn_color,
            Kind::Unspecified,
        ));
        let max_color = Color32F::new(
            BTN_MAX_COLOR[0],
            BTN_MAX_COLOR[1],
            BTN_MAX_COLOR[2],
            BTN_MAX_COLOR[3],
        );
        let max_x = actual_loc.x + win_w - CLOSE_BTN_SIZE * 2 - CLOSE_BTN_MARGIN - BTN_GAP;
        let max_y = actual_loc.y - TITLEBAR_H + (TITLEBAR_H - CLOSE_BTN_SIZE) / 2;
        let max_rect: Rectangle<i32, Physical> = Rectangle::new(
            smithay::utils::Point::from((max_x, max_y)).to_physical_precise_round(1.0),
            (CLOSE_BTN_SIZE, CLOSE_BTN_SIZE).into(),
        );
        out.push(SolidColorRenderElement::new(
            Id::new(),
            max_rect,
            0,
            max_color,
            Kind::Unspecified,
        ));
        let min_color = Color32F::new(
            BTN_MIN_COLOR[0],
            BTN_MIN_COLOR[1],
            BTN_MIN_COLOR[2],
            BTN_MIN_COLOR[3],
        );
        let min_x = actual_loc.x + win_w - CLOSE_BTN_SIZE * 3 - CLOSE_BTN_MARGIN - BTN_GAP * 2;
        let min_y = actual_loc.y - TITLEBAR_H + (TITLEBAR_H - CLOSE_BTN_SIZE) / 2;
        let min_rect: Rectangle<i32, Physical> = Rectangle::new(
            smithay::utils::Point::from((min_x, min_y)).to_physical_precise_round(1.0),
            (CLOSE_BTN_SIZE, CLOSE_BTN_SIZE).into(),
        );
        out.push(SolidColorRenderElement::new(
            Id::new(),
            min_rect,
            0,
            min_color,
            Kind::Unspecified,
        ));

        // W37.7: BG pushed POR ULTIMO (smithay renderiza Vec em ordem
        // reversa - ultimo elem renderiza atras). z=1 reforca atras.
        out.push(SolidColorRenderElement::new(
            Id::new(),
            tb_bg_rect,
            1,
            bg_color,
            Kind::Unspecified,
        ));
    }
    out
}

/// W29.4: titlebar bg shader pra UMA window. Per-window helper.
pub fn titlebar_bg_for_window(
    window: &Window,
    space: &Space<Window>,
    ssd_windows: &std::collections::HashSet<
        smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    >,
    bg_shader: Option<&crate::backend::corner_shader::TitlebarBgShader>,
) -> Option<PixelShaderElement> {
    let shader = bg_shader?;
    let is_ssd = window
        .wl_surface()
        .map(|s| ssd_windows.contains(&*s))
        .unwrap_or(false);
    if !is_ssd {
        return None;
    }
    let loc = space.element_location(window).unwrap_or_default();
    let win_w = window.geometry().size.w;
    // W32.1: skip se janela ainda nao configurada (evita flicker btns lado esquerdo)
    if win_w < 200 {
        return None;
    }
    let area: Rectangle<i32, smithay::utils::Logical> = Rectangle::new(
        smithay::utils::Point::from((loc.x, loc.y - TITLEBAR_H)),
        (win_w, TITLEBAR_H).into(),
    );
    let uniforms = vec![
        smithay::backend::renderer::gles::Uniform::new(
            "u_color",
            (TITLEBAR_BG[0], TITLEBAR_BG[1], TITLEBAR_BG[2]),
        )
        .into_owned(),
        smithay::backend::renderer::gles::Uniform::new("u_radius", 12.0f32).into_owned(),
    ];
    Some(PixelShaderElement::new(
        shader.program.clone(),
        area,
        None,
        1.0,
        uniforms,
        Kind::Unspecified,
    ))
}

pub fn titlebar_elements(
    space: &Space<Window>,
    ssd_windows: &std::collections::HashSet<
        smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    >,
) -> Vec<SolidColorRenderElement> {
    let mut out = Vec::new();
    let bg_color = Color32F::new(
        TITLEBAR_BG[0],
        TITLEBAR_BG[1],
        TITLEBAR_BG[2],
        TITLEBAR_BG[3],
    );
    let btn_color = Color32F::new(
        CLOSE_BTN_COLOR[0],
        CLOSE_BTN_COLOR[1],
        CLOSE_BTN_COLOR[2],
        CLOSE_BTN_COLOR[3],
    );

    // W29.4: iterar TOP-DOWN (focal first). Smithay vec front-first; focal
    // push primeiro = vec[idx_menor] = drawn last em damage tracker = ON TOP.
    // space.elements() default = bottom-to-top; .rev() = top-to-bottom.
    let windows: Vec<&Window> = space.elements().rev().collect();
    for window in windows.iter().copied() {
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

        // W19.3 FIX: smithay convention list eh FRONT-FIRST (idx 0 = topo
        // da pilha). Antes bg ia em primeiro = COBRIA os botoes. Agora
        // buttons primeiro (frente), bg por ultimo (fundo).

        // Close button (vermelho, direita).
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

        // W17.1: Maximize button (verde, esquerda do close).
        let max_color = Color32F::new(
            BTN_MAX_COLOR[0],
            BTN_MAX_COLOR[1],
            BTN_MAX_COLOR[2],
            BTN_MAX_COLOR[3],
        );
        let max_x = loc.x + win_w - CLOSE_BTN_SIZE * 2 - CLOSE_BTN_MARGIN - BTN_GAP;
        let max_y = loc.y - TITLEBAR_H + (TITLEBAR_H - CLOSE_BTN_SIZE) / 2;
        let max_rect: Rectangle<i32, Physical> = Rectangle::new(
            smithay::utils::Point::from((max_x, max_y)).to_physical_precise_round(1.0),
            (CLOSE_BTN_SIZE, CLOSE_BTN_SIZE).into(),
        );
        out.push(SolidColorRenderElement::new(
            Id::new(),
            max_rect,
            0,
            max_color,
            Kind::Unspecified,
        ));

        // W17.1: Minimize button (amarelo, esquerda do maximize).
        let min_color = Color32F::new(
            BTN_MIN_COLOR[0],
            BTN_MIN_COLOR[1],
            BTN_MIN_COLOR[2],
            BTN_MIN_COLOR[3],
        );
        let min_x = loc.x + win_w - CLOSE_BTN_SIZE * 3 - CLOSE_BTN_MARGIN - BTN_GAP * 2;
        let min_y = loc.y - TITLEBAR_H + (TITLEBAR_H - CLOSE_BTN_SIZE) / 2;
        let min_rect: Rectangle<i32, Physical> = Rectangle::new(
            smithay::utils::Point::from((min_x, min_y)).to_physical_precise_round(1.0),
            (CLOSE_BTN_SIZE, CLOSE_BTN_SIZE).into(),
        );
        out.push(SolidColorRenderElement::new(
            Id::new(),
            min_rect,
            0,
            min_color,
            Kind::Unspecified,
        ));

        // W29: bg titlebar renderizado por titlebar_bg_elements (PixelShaderElement
        // com SDF top-round). titlebar_elements retorna SO botoes agora.
    }
    let _ = bg_color;
    out
}

/// W29: titlebar bg via shader. UM PixelShaderElement por janela SSD cobrindo
/// area titlebar inteira (loc.x..loc.x+win_w, loc.y-TITLEBAR_H..loc.y). Shader
/// = TitlebarBgShader SDF top-round (top corners radius=12, bottom squared).
pub fn titlebar_bg_elements(
    space: &Space<Window>,
    ssd_windows: &std::collections::HashSet<
        smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    >,
    bg_shader: Option<&crate::backend::corner_shader::TitlebarBgShader>,
) -> Vec<PixelShaderElement> {
    let mut out = Vec::new();
    let Some(shader) = bg_shader else {
        return out;
    };

    // W29.4: TOP-DOWN ordem pra focal frente bg shader (vec[idx_menor]).
    let windows: Vec<&Window> = space.elements().rev().collect();
    for window in windows.iter().copied() {
        let is_ssd = window
            .wl_surface()
            .map(|s| ssd_windows.contains(&*s))
            .unwrap_or(false);
        if !is_ssd {
            continue;
        }
        let loc = space.element_location(window).unwrap_or_default();
        let win_w = window.geometry().size.w;

        let area: Rectangle<i32, smithay::utils::Logical> = Rectangle::new(
            smithay::utils::Point::from((loc.x, loc.y - TITLEBAR_H)),
            (win_w, TITLEBAR_H).into(),
        );
        let uniforms = vec![
            smithay::backend::renderer::gles::Uniform::new(
                "u_color",
                (TITLEBAR_BG[0], TITLEBAR_BG[1], TITLEBAR_BG[2]),
            )
            .into_owned(),
            smithay::backend::renderer::gles::Uniform::new("u_radius", 12.0f32).into_owned(),
        ];
        out.push(PixelShaderElement::new(
            shader.program.clone(),
            area,
            None,
            1.0,
            uniforms,
            Kind::Unspecified,
        ));
    }
    out
}

/// W28.8: corner mask elements (TL+TR+BL+BR) com SDF AA preto premultiplied.
/// Cobre wallpaper atras dos cantos round da janela. 4 PixelShaderElement
/// por janela SSD, 12x12 logico cada, anchor=(1,1)/(0,1)/(1,0)/(0,0).
pub fn ssd_corner_masks(
    space: &Space<Window>,
    ssd_windows: &std::collections::HashSet<
        smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    >,
    mask_shader: Option<&CornerMaskShader>,
) -> Vec<PixelShaderElement> {
    let mut out = Vec::new();
    let Some(shader) = mask_shader else {
        return out;
    };
    const CORNER_SZ: i32 = 12;
    const R: f32 = 12.0;

    for window in space.elements() {
        let is_ssd = window
            .wl_surface()
            .map(|s| ssd_windows.contains(&*s))
            .unwrap_or(false);
        if !is_ssd {
            continue;
        }
        let loc = space.element_location(window).unwrap_or_default();
        let win_w = window.geometry().size.w;
        let win_h = window.geometry().size.h;
        let titlebar_top_y = loc.y - TITLEBAR_H;
        let content_bottom_y = loc.y + win_h - CORNER_SZ;

        let corners: [((i32, i32), (f32, f32)); 4] = [
            ((loc.x, titlebar_top_y), (1.0, 1.0)),
            ((loc.x + win_w - CORNER_SZ, titlebar_top_y), (0.0, 1.0)),
            ((loc.x, content_bottom_y), (1.0, 0.0)),
            ((loc.x + win_w - CORNER_SZ, content_bottom_y), (0.0, 0.0)),
        ];

        for ((cx, cy), (ax, ay)) in corners {
            let area: Rectangle<i32, smithay::utils::Logical> = Rectangle::new(
                smithay::utils::Point::from((cx, cy)),
                (CORNER_SZ, CORNER_SZ).into(),
            );
            let uniforms = vec![
                smithay::backend::renderer::gles::Uniform::new("u_anchor", (ax, ay)).into_owned(),
                smithay::backend::renderer::gles::Uniform::new("u_radius", R).into_owned(),
            ];
            out.push(PixelShaderElement::new(
                shader.program.clone(),
                area,
                None,
                1.0,
                uniforms,
                Kind::Unspecified,
            ));
        }
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
        Id::new(),
        bg_rect,
        0,
        Color32F::new(0.10, 0.10, 0.11, 0.95),
        Kind::Unspecified,
    ));
    let sep_y = menu_pos.y + item_h * 3 + item_h / 2 - 1;
    let sep_rect: Rectangle<i32, Physical> = Rectangle::new(
        smithay::utils::Point::from((menu_pos.x + 8, sep_y)).to_physical_precise_round(1.0),
        (menu_w - 16, 1).into(),
    );
    out.push(SolidColorRenderElement::new(
        Id::new(),
        sep_rect,
        0,
        Color32F::new(0.3, 0.3, 0.3, 0.5),
        Kind::Unspecified,
    ));
    if hover_idx < 5 && hover_idx != 3 {
        let hl_rect: Rectangle<i32, Physical> = Rectangle::new(
            smithay::utils::Point::from((
                menu_pos.x + 2,
                menu_pos.y + item_h * hover_idx as i32 + 1,
            ))
            .to_physical_precise_round(1.0),
            (menu_w - 4, item_h - 2).into(),
        );
        out.push(SolidColorRenderElement::new(
            Id::new(),
            hl_rect,
            0,
            Color32F::new(0.24, 0.24, 0.28, 0.90),
            Kind::Unspecified,
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

/// Cor de clear do framebuffer. SEMPRE preto opaco linear, independente
/// de tema. R1.fix4: clear branco em tema light causava flash branco
/// visivel em mouse-move quando wallpaper buffer ainda nao tinha
/// renderizado o frame completo (race scanout vs blit). Como wallpaper
/// cobre toda a tela em qualquer tema, scanout-floor preto neutro = zero
/// flash. Tema light continua afetando LumoColors.bg para UI.
pub fn clear_color_linear() -> [f32; 4] {
    [0.0, 0.0, 0.0, 1.0]
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
        let geo: Rectangle<i32, Physical> = Rectangle::new(Point::from((x, y)), (r, r).into());
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
        // W24.6: sombra precisa acompanhar o offset da geometria (geo.loc)
        let actual_loc = *loc + geo.loc;
        // Faixa de sombra: so ABAIXO da janela (drop-shadow classico).
        let sx = actual_loc.x - SHADOW_BLEED;
        let sy = actual_loc.y + geo.size.h;
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
    sx: i32,
    sy: i32,
    sw: i32,
    sh: i32,
    ox: i32,
    oy: i32,
    ow: i32,
    oh: i32,
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
    /// W6.C: splash logo alpha (0.0 = invisivel, 1.0 = opaco). 0.0 = skip render.
    pub splash_alpha: f32,
    /// W6.C: splash buffer pre-carregado. None = sem splash.
    pub splash_buffer: Option<&'a MemoryRenderBuffer>,
    /// M1: surfaces SSD para pintar titlebar.
    pub ssd_windows: &'a std::collections::HashSet<
        smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    >,
    pub corner_mask_shader: Option<&'a CornerMaskShader>,
    pub titlebar_bg_shader: Option<&'a crate::backend::corner_shader::TitlebarBgShader>,
    /// T1.1: menu popup titlebar ativo. None = sem menu.
    pub titlebar_menu: Option<(smithay::utils::Point<i32, smithay::utils::Logical>, usize)>,
    /// W9.B: snap zone preview during window drag.
    pub snap_preview: Option<crate::input::move_grab::SnapZone>,
    /// W12.B: mission control overview elements. None = not active.
    pub overview_elements: Vec<smithay::backend::renderer::element::solid::SolidColorRenderElement>,
    /// W12.C: stack picker elements. None = not active.
    pub picker_elements: Vec<smithay::backend::renderer::element::solid::SolidColorRenderElement>,
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
        let geo: Rectangle<i32, Physical> = Rectangle::new(
            Point::from((0, 0)),
            (inputs.output_w, inputs.output_h).into(),
        );
        overlay.push(LumoCustomElement::Solid(SolidColorRenderElement::new(
            Id::new(),
            geo,
            0,
            Color32F::new(0.0, 0.0, 0.0, alpha),
            Kind::Unspecified,
        )));
        // W6.C: splash logo sobre boot curtain.
        if inputs.splash_alpha > 0.001 {
            if let Some(buf) = inputs.splash_buffer {
                if let Some(elem) = crate::backend::wallpaper::splash_element(
                    renderer,
                    buf,
                    inputs.output_w,
                    inputs.output_h,
                    inputs.splash_alpha.clamp(0.0, 1.0),
                ) {
                    overlay.push(LumoCustomElement::Memory(elem));
                }
            }
        }
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

    // 3. Corner radius: quads sobre cantos de cada toplevel.
    for elem in window_corner_elements(inputs.space) {
        overlay.push(LumoCustomElement::Solid(elem));
    }

    // M1: SSD titlebars (fundo + close button).
    for elem in titlebar_elements(inputs.space, inputs.ssd_windows) {
        overlay.push(LumoCustomElement::Solid(elem));
    }
    // W28.9: mask preto cantos desativado
    // for elem in ssd_corner_masks(inputs.space, inputs.ssd_windows, inputs.corner_mask_shader) {
    //     overlay.push(LumoCustomElement::Pixel(elem));
    // }
    // T1.1: menu popup SSD.
    if let Some((menu_pos, hover)) = inputs.titlebar_menu {
        for elem in titlebar_menu_elements(menu_pos, hover) {
            overlay.push(LumoCustomElement::Solid(elem));
        }
    }
    // W9.B: snap zone preview overlay.
    if let Some(zone) = inputs.snap_preview {
        overlay.push(LumoCustomElement::Solid(snap_preview_element(
            zone,
            inputs.output_w,
            inputs.output_h,
        )));
    }
    // W12.B/C: overview + picker overlays (front of stack).
    for elem in &inputs.overview_elements {
        overlay.push(LumoCustomElement::Solid(elem.clone()));
    }
    for elem in &inputs.picker_elements {
        overlay.push(LumoCustomElement::Solid(elem.clone()));
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
    /// W6.C: splash logo alpha.
    pub splash_alpha: f32,
    /// W6.C: splash buffer. None = sem splash.
    pub splash_buffer: Option<&'a MemoryRenderBuffer>,
    /// M1: surfaces SSD para pintar titlebar.
    pub ssd_windows: &'a std::collections::HashSet<
        smithay::reexports::wayland_server::protocol::wl_surface::WlSurface,
    >,
    pub corner_mask_shader: Option<&'a CornerMaskShader>,
    pub titlebar_bg_shader: Option<&'a crate::backend::corner_shader::TitlebarBgShader>,
    /// T1.1: menu popup titlebar ativo. None = sem menu.
    pub titlebar_menu: Option<(smithay::utils::Point<i32, smithay::utils::Logical>, usize)>,
    /// W9.B: snap zone preview during window drag.
    pub snap_preview: Option<crate::input::move_grab::SnapZone>,
    /// W12.B: mission control overview elements.
    pub overview_elements: Vec<smithay::backend::renderer::element::solid::SolidColorRenderElement>,
    /// W12.C: stack picker elements.
    pub picker_elements: Vec<smithay::backend::renderer::element::solid::SolidColorRenderElement>,
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
    if inputs.boot_curtain_alpha > 0.001 {
        let alpha = inputs.boot_curtain_alpha.clamp(0.0, 1.0);
        let geo: Rectangle<i32, Physical> = Rectangle::new(
            Point::from((0, 0)),
            (inputs.output_w, inputs.output_h).into(),
        );
        out.push(LumoCustomElement::Solid(SolidColorRenderElement::new(
            Id::new(),
            geo,
            0,
            Color32F::new(0.0, 0.0, 0.0, alpha),
            Kind::Unspecified,
        )));
        // W6.C: splash logo sobre boot curtain (lista front-first: splash fica na frente do quad preto).
        if inputs.splash_alpha > 0.001 {
            if let Some(buf) = inputs.splash_buffer {
                if let Some(elem) = crate::backend::wallpaper::splash_element(
                    renderer,
                    buf,
                    inputs.output_w,
                    inputs.output_h,
                    inputs.splash_alpha.clamp(0.0, 1.0),
                ) {
                    // Insere antes do quad preto (index 0 = mais na frente).
                    out.insert(0, LumoCustomElement::Memory(elem));
                }
            }
        }
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

    // 3a. Corner radius sobre cantos dos toplevels.
    for elem in window_corner_elements(inputs.space) {
        out.push(LumoCustomElement::Solid(elem));
    }

    // W29.5: per-window interleave. Em vez de espace_render_elements global
    // que mistura todos toplevels (= SSD btns vazam sobre janela frente),
    // split em upper (front)/lower (back) layers separados + iterar windows
    // manualmente front-first. Pra cada janela: btns+bg+content como UM block
    // no vec, garantindo Z-order per janela.
    let all_elements = space_render_elements::<_, Window, _>(
        renderer,
        std::iter::once(inputs.space),
        inputs.output,
        1.0,
    )
    .unwrap_or_default();
    let mut upper_layers: Vec<_> = Vec::new();
    let mut lower_layers: Vec<_> = Vec::new();
    let mut in_lower = false;
    let mut element_seen = false;
    for el in all_elements {
        match &el {
            smithay::desktop::space::SpaceRenderElements::Element(_) => {
                element_seen = true;
            }
            smithay::desktop::space::SpaceRenderElements::Surface(_) => {
                if element_seen {
                    in_lower = true;
                }
            }
            _ => {}
        }
        if matches!(el, smithay::desktop::space::SpaceRenderElements::Element(_)) {
            continue;
        }
        if in_lower {
            lower_layers.push(el);
        } else {
            upper_layers.push(el);
        }
    }

    // 4a. Upper layers (Layer::Top/Overlay) na frente de titlebars.
    for el in upper_layers {
        out.push(LumoCustomElement::Space(el));
    }

    // T1.1: menu popup SSD.
    if let Some((menu_pos, hover)) = inputs.titlebar_menu {
        for elem in titlebar_menu_elements(menu_pos, hover) {
            out.push(LumoCustomElement::Solid(elem));
        }
    }
    // W9.B: snap zone preview overlay.
    if let Some(zone) = inputs.snap_preview {
        out.push(LumoCustomElement::Solid(snap_preview_element(
            zone,
            inputs.output_w,
            inputs.output_h,
        )));
    }
    // W12.B/C: overview + picker overlays.
    for elem in &inputs.overview_elements {
        out.push(LumoCustomElement::Solid(elem.clone()));
    }
    for elem in &inputs.picker_elements {
        out.push(LumoCustomElement::Solid(elem.clone()));
    }

    // W29.5: per-window block. Iterar front-first. Pra cada window:
    //   1. SSD btns (vec[idx_menor]=frente do bloco = ON TOP)
    //   2. SSD bg shader
    //   3. Window content via window.render_elements
    // Resultado: focal window block frente do bg window block. Focal content
    // cobre bg btns+bg+content em overlap area cross-window.
    let windows: Vec<&Window> = inputs.space.elements().rev().collect();
    for window in windows.iter().copied() {
        // SSD btns
        for btn in titlebar_btns_for_window(window, inputs.space, inputs.ssd_windows) {
            out.push(LumoCustomElement::Solid(btn));
        }
        // SSD bg shader
        if let Some(bg) = titlebar_bg_for_window(
            window,
            inputs.space,
            inputs.ssd_windows,
            inputs.titlebar_bg_shader,
        ) {
            out.push(LumoCustomElement::Pixel(bg));
        }
        // Content via window.render_elements
        let loc = inputs.space.element_location(window).unwrap_or_default();
        let phys_loc: smithay::utils::Point<i32, smithay::utils::Physical> =
            smithay::utils::Point::from((loc.x, loc.y)).to_physical_precise_round(1.0);
        use smithay::desktop::space::SpaceElement;
        let content_elems: Vec<WaylandSurfaceRenderElement<GlesRenderer>> =
            smithay::backend::renderer::element::AsRenderElements::render_elements(
                window,
                renderer,
                phys_loc,
                smithay::utils::Scale::from(1.0),
                1.0,
            );
        if let Some(cs) = inputs.corner_shader {
            // Wrap content em RoundedSurfaceElement
            for el in content_elems {
                let space_wrap = smithay::desktop::space::SpaceRenderElements::Surface(el);
                out.push(LumoCustomElement::Rounded(RoundedSurfaceElement::new(
                    space_wrap,
                    cs.program.clone(),
                    CORNER_RADIUS_WINDOW as f32,
                )));
            }
        } else {
            for el in content_elems {
                let space_wrap = smithay::desktop::space::SpaceRenderElements::Surface(el);
                out.push(LumoCustomElement::Space(space_wrap));
            }
        }
        let _ = SpaceElement::geometry(window); // mantem import
    }

    // 4c. Lower layers (Layer::Bottom/Background) atras dos toplevels.
    for el in lower_layers {
        out.push(LumoCustomElement::Space(el));
    }

    // W29.5: SSD btns+bg ja foram pushed per-window acima junto com content.

    // shadow pos space: sombras renderem ABAIXO de popups/toplevels.
    // Lista smithay eh front-first; shadow apos space = atras.
    for elem in shadow_elements(inputs.space) {
        out.push(LumoCustomElement::Solid(elem));
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
        let geo: Rectangle<i32, Physical> = Rectangle::new(
            Point::from((0, 0)),
            (inputs.output_w, inputs.output_h).into(),
        );
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

    // 3a. Corner radius sobre cantos dos toplevels.
    for elem in window_corner_elements(inputs.space) {
        out.push(LumoCustomElement::Solid(elem));
    }

    // 4. Space (toplevels + layer-shell + popups).
    //    BUG1 FIX: mesmo split que collect_drm_elements.
    //    upper layers (Layer::Top/Overlay) -> titlebars -> toplevels + lower.
    let (upper_layers_w, space_rest_w): (Vec<_>, Vec<_>) = match space_render_elements::<_, Window, _>(
        renderer,
        std::iter::once(inputs.space),
        output,
        1.0,
    ) {
        Ok(elements) => {
            let first_elem = elements
                .iter()
                .position(|e| matches!(e, smithay::desktop::space::SpaceRenderElements::Element(_)))
                .unwrap_or(elements.len());
            let mut upper = Vec::with_capacity(first_elem);
            let mut rest = Vec::with_capacity(elements.len().saturating_sub(first_elem));
            for (i, el) in elements.into_iter().enumerate() {
                if i < first_elem {
                    upper.push(el);
                } else {
                    rest.push(el);
                }
            }
            (upper, rest)
        }
        Err(err) => {
            tracing::warn!(?err, "space_render_elements falhou no winit path");
            (Vec::new(), Vec::new())
        }
    };

    // 4a. Upper layers (Layer::Top/Overlay) na frente.
    for el in upper_layers_w {
        out.push(LumoCustomElement::Space(el));
    }

    // M1: SSD titlebars -- atras de Layer::Top, na frente de toplevels.
    // W29.1: botoes ANTES bg shader. Smithay vec[0]=topmost. Bg shader
    // cobre area inteira titlebar; precisa ficar atras dos botoes.
    for elem in titlebar_elements(inputs.space, inputs.ssd_windows) {
        out.push(LumoCustomElement::Solid(elem));
    }
    for elem in titlebar_bg_elements(inputs.space, inputs.ssd_windows, inputs.titlebar_bg_shader) {
        out.push(LumoCustomElement::Pixel(elem));
    }
    // W28.9: mask preto cantos desativado (visivel sobre titlebar dark)
    // for elem in ssd_corner_masks(inputs.space, inputs.ssd_windows, inputs.corner_mask_shader) {
    //     out.push(LumoCustomElement::Pixel(elem));
    // }
    // T1.1: menu popup SSD.
    if let Some((menu_pos, hover)) = inputs.titlebar_menu {
        for elem in titlebar_menu_elements(menu_pos, hover) {
            out.push(LumoCustomElement::Solid(elem));
        }
    }
    // W9.B: snap zone preview overlay.
    if let Some(zone) = inputs.snap_preview {
        out.push(LumoCustomElement::Solid(snap_preview_element(
            zone,
            inputs.output_w,
            inputs.output_h,
        )));
    }
    // W12.B/C: overview + picker overlays.
    for elem in &inputs.overview_elements {
        out.push(LumoCustomElement::Solid(elem.clone()));
    }
    for elem in &inputs.picker_elements {
        out.push(LumoCustomElement::Solid(elem.clone()));
    }

    // 4b. Toplevels + lower layer-shell.
    //     A38: toplevels (Element variant) recebem SDF corner radius.
    if let Some(cs) = inputs.corner_shader {
        for el in wrap_space_elements_rounded(space_rest_w, cs) {
            out.push(el);
        }
    } else {
        for el in space_rest_w {
            out.push(LumoCustomElement::Space(el));
        }
    }

    // shadow pos space (winit): sombras ABAIXO de popups/toplevels.
    for elem in shadow_elements(inputs.space) {
        out.push(LumoCustomElement::Solid(elem));
    }

    // 5. Wallpaper no fundo.
    if let Some(wp) = inputs.wallpaper {
        out.push(LumoCustomElement::Texture(
            wp.element(inputs.output_w, inputs.output_h),
        ));
    }

    out
}
/// W3.P2: coleta apenas o elemento cursor pra fast-path de cursor HW plane.
///
/// Quando so o cursor se moveu (sem window damage), passa apenas o elemento
/// cursor para render_frame. DrmCompositor usa o buffer do primary plane
/// existente (skip=true) e faz atomic commit apenas do cursor plane,
/// desacoplando movimento de cursor da taxa de render das aplicacoes.
///
/// Retorna Vec vazio se cursor nao pode ser renderizado (fallback pra full render).
pub fn collect_cursor_only_elements(
    renderer: &mut GlesRenderer,
    inputs: &DrmCollectInputs<'_>,
) -> Vec<LumoCustomElement> {
    let mut out = Vec::with_capacity(2);

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

    out
}

/// W9.B: render snap zone preview overlay quad (accent color alpha 0.3).
pub fn snap_preview_element(
    zone: crate::input::move_grab::SnapZone,
    out_w: i32,
    out_h: i32,
) -> SolidColorRenderElement {
    let accent = lumo_foundation::LFTokens::EMERALD_500;
    let color = Color32F::new(accent[0], accent[1], accent[2], 0.30);
    let (x, y, w, h) = zone.layout(out_w, out_h);
    let geo: Rectangle<i32, Physical> = Rectangle::new(
        Point::from((x, y)).to_physical_precise_round(1.0),
        (w, h).into(),
    );
    SolidColorRenderElement::new(Id::new(), geo, 0, color, Kind::Unspecified)
}

#[cfg(test)]
mod w37_7_tb_bg_tests {
    use super::*;
    use smithay::utils::Point;

    /// W37.7: BG da titlebar deve cobrir full-width na altura TITLEBAR_H.
    /// Antes faltava esse rect -> SSD aparecia como botoes flutuantes
    /// + titulo isolado, conteudo da janela vazava por baixo.
    #[test]
    fn w37_7_titlebar_bg_full_width() {
        let actual_x = 100;
        let actual_y = 200;
        let win_w = 800;
        // Constroi o rect manualmente igual ao paint code.
        let bg_rect: Rectangle<i32, Physical> = Rectangle::new(
            Point::<i32, smithay::utils::Logical>::from((actual_x, actual_y - TITLEBAR_H))
                .to_physical_precise_round(1.0),
            (win_w, TITLEBAR_H).into(),
        );
        assert_eq!(bg_rect.size.w, win_w);
        assert_eq!(bg_rect.size.h, TITLEBAR_H);
        // Botoes devem estar DENTRO do bg rect.
        let close = ssd_close_btn_rect_logical(
            Point::<i32, smithay::utils::Logical>::from((actual_x, actual_y - TITLEBAR_H)),
            win_w,
        );
        let max = ssd_max_btn_rect_logical(
            Point::<i32, smithay::utils::Logical>::from((actual_x, actual_y - TITLEBAR_H)),
            win_w,
        );
        let min = ssd_min_btn_rect_logical(
            Point::<i32, smithay::utils::Logical>::from((actual_x, actual_y - TITLEBAR_H)),
            win_w,
        );
        // Bg cobre toda a area da titlebar onde os botoes ficam.
        assert!(close.loc.x >= bg_rect.loc.x);
        assert!(close.loc.x + close.size.w <= bg_rect.loc.x + bg_rect.size.w);
        assert!(max.loc.x >= bg_rect.loc.x);
        assert!(min.loc.x >= bg_rect.loc.x);
    }
}

#[cfg(test)]
mod ssd_btn_tests {
    use super::*;
    use smithay::utils::Point;

    /// W17.1: SSD titlebar deve ter 3 rects nao-sobrepostos (min, max, close).
    #[test]
    fn titlebar_has_three_buttons_per_ssd_window() {
        let loc: Point<i32, smithay::utils::Logical> = Point::from((100, 200));
        let win_w = 400;
        let close = ssd_close_btn_rect_logical(loc, win_w);
        let max = ssd_max_btn_rect_logical(loc, win_w);
        let min = ssd_min_btn_rect_logical(loc, win_w);

        // Cada rect tem o tamanho correto.
        assert_eq!(close.size.w, CLOSE_BTN_SIZE);
        assert_eq!(max.size.w, CLOSE_BTN_SIZE);
        assert_eq!(min.size.w, CLOSE_BTN_SIZE);

        // Order esquerda->direita: min < max < close (no eixo x).
        assert!(
            min.loc.x < max.loc.x,
            "minimize deve ficar a esquerda de maximize"
        );
        assert!(
            max.loc.x < close.loc.x,
            "maximize deve ficar a esquerda de close"
        );

        // Nao sobrepoem (gap entre eles).
        assert!(
            min.loc.x + min.size.w <= max.loc.x,
            "min e max nao podem sobrepor"
        );
        assert!(
            max.loc.x + max.size.w <= close.loc.x,
            "max e close nao podem sobrepor"
        );

        // Todos dentro da titlebar (y entre [loc.y - TITLEBAR_H, loc.y]).
        for r in [close, max, min] {
            assert!(r.loc.y >= loc.y - TITLEBAR_H);
            assert!(r.loc.y + r.size.h <= loc.y);
        }
    }

    #[test]
    fn test_renderer_extreme_geometry_no_panic() {
        use smithay::utils::Point;
        let loc: Point<i32, smithay::utils::Logical> = Point::from((0, 0));
        let huge_w = i32::MAX;
        
        // Teste: nao deve dar panic nem overflow bizarro (usamos saturating e clamp)
        let rect = close_btn_rect(loc, huge_w);
        assert!(rect.loc.x >= 0);
        assert!(rect.size.w > 0);
        
        let logical_rect = ssd_close_btn_rect_logical(loc, huge_w);
        assert!(logical_rect.loc.x >= 0);
    }

    #[test]
    fn test_renderer_negative_geometry_no_panic() {
        use smithay::utils::Point;
        let loc: Point<i32, smithay::utils::Logical> = Point::from((0, 0));
        let neg_w = -100;
        
        // Deve clamp para 0 e nao dar panic
        let rect = close_btn_rect(loc, neg_w);
        assert!(rect.size.w > 0);
    }
}
