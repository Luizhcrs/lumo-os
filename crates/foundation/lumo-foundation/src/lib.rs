//! lumo-foundation
//!
//! Apple-style "Foundation": camada base com design tokens, helpers de cor
//! e helpers de geometria (pixel <-> NDC). Zero dependencias de wgpu/winit
//! para poder ser usada por qualquer camada acima.
//!
//! Layout:
//! - [`LFTokens`]   : design tokens (cores ink/panel/emerald/pearl/danger).
//! - [`LFColor`]    : conversao sRGB <-> linear.
//! - [`LFGeometry`] : pixel-to-NDC helpers para widgets / renderers.
//!
//! # Color space gotcha
//!
//! Surfaces wgpu usam `Bgra8UnormSrgb` (sRGB). O hardware aplica
//! `linear -> sRGB` automaticamente ao escrever no framebuffer. Portanto
//! **as cores enviadas ao shader devem estar em linear space**, nao em
//! sRGB. Os campos `*` (sem sufixo) em `LFTokens` ja sao linear.
//! Os `*_SRGB` preservam o valor nominal do design system pra debug.
//! Use [`LFColor::srgb_to_linear`] em runtime quando carregar cor de
//! usuario (hex picker, theme override, etc.).

// ---------------------------------------------------------------------------
// LFTokens — design tokens
// ---------------------------------------------------------------------------

/// Tokens de cor do Lumo OS.
///
/// **Importante**: os valores em `[f32; 4]` aqui ja estao em **linear space**,
/// prontos para serem escritos por um shader cuja saida alimenta um surface
/// sRGB. As versoes `_SRGB` preservam os valores nominais do design system
/// para debug / referencia / parsing humano.
pub struct LFTokens;

impl LFTokens {
    // -- sRGB references (design system originals; debug / reference) -----------
    /// `#0a0a0c` ink deep -- sRGB normalizado.
    pub const INK_DEEP_SRGB:    [f32; 4] = [0.039_215_688, 0.039_215_688, 0.047_058_82, 1.0];
    /// `#1a1a21` panel-hi -- sRGB normalizado.
    pub const PANEL_HI_SRGB:    [f32; 4] = [0.101_960_786, 0.101_960_786, 0.129_411_77, 1.0];
    /// `#131318` panel -- sRGB normalizado.
    pub const PANEL_SRGB:       [f32; 4] = [0.074_509_805, 0.074_509_805, 0.094_117_65, 1.0];
    /// `#059669` emerald-600 -- sRGB normalizado.
    pub const EMERALD_600_SRGB: [f32; 4] = [0.019_607_844, 0.588_235_3, 0.411_764_7, 1.0];
    /// `#10b981` emerald-500 -- sRGB normalizado.
    pub const EMERALD_500_SRGB: [f32; 4] = [0.062_745_1, 0.725_490_2, 0.505_882_36, 1.0];
    /// `#f5f5f7` pearl -- sRGB normalizado.
    pub const PEARL_SRGB:       [f32; 4] = [0.960_784_3, 0.960_784_3, 0.968_627_5, 1.0];
    /// `#9596a0` muted -- sRGB normalizado.
    pub const MUTED_SRGB:       [f32; 4] = [0.585_0, 0.586_0, 0.627_0, 1.0];
    /// `#f87171` danger (red-400) -- sRGB normalizado.
    pub const DANGER_SRGB:      [f32; 4] = [0.972_549, 0.443_137, 0.443_137, 1.0];

    // -- Linear (GPU-ready) -----------------------------------------------------
    // Pre-computado offline via `LFColor::srgb_to_linear(c)` para evitar
    // runtime cost por frame e manter `const`. Se algum hex mudar,
    // recalcular e atualizar.

    /// `#0a0a0c` ink deep (background do shell) -- linear.
    pub const INK_DEEP:    [f32; 4] = [0.003_035_3, 0.003_035_3, 0.003_676_5, 1.0];
    /// `#1a1a21` panel-hi (cards, surfaces elevadas) -- linear.
    pub const PANEL_HI:    [f32; 4] = [0.010_329_8, 0.010_329_8, 0.015_208_5, 1.0];
    /// `#131318` panel base (surface neutra) -- linear.
    pub const PANEL:       [f32; 4] = [0.006_512_1, 0.006_512_1, 0.009_134_1, 1.0];
    /// `#059669` emerald-600 (accent primario) -- linear.
    pub const EMERALD_600: [f32; 4] = [0.001_517_6, 0.304_987_3, 0.141_263_3, 1.0];
    /// `#10b981` emerald-500 (accent secundario / hover) -- linear.
    pub const EMERALD_500: [f32; 4] = [0.005_181_5, 0.485_149_9, 0.219_526_2, 1.0];
    /// `#f5f5f7` quasi-white (texto, borders fortes) -- linear.
    pub const PEARL:       [f32; 4] = [0.913_098_6, 0.913_098_6, 0.930_111_0, 1.0];
    /// `#9596a0` muted (text de baixa enfase) -- linear.
    pub const MUTED:       [f32; 4] = [0.301_318_7, 0.302_449_8, 0.350_975_3, 1.0];
    /// `#f87171` danger / red-400 -- linear.
    pub const DANGER:      [f32; 4] = [0.938_685_7, 0.165_132_2, 0.165_132_2, 1.0];

    /// Transparente puro.
    pub const TRANSPARENT: [f32; 4] = [0.0, 0.0, 0.0, 0.0];

    // -- shadow tokens --------------------------------------------------------
    /// Sombra preta neutra (drop shadow padrao de cards). RGB linear (0,0,0)
    /// dispensa conversao; alpha controla intensidade.
    pub const SHADOW_BLACK: [f32; 4] = [0.0, 0.0, 0.0, 0.4];
    /// Sombra accent translucida (cards emerald, glow controlado) -- RGB linear.
    pub const SHADOW_ACCENT: [f32; 4] = [0.001_517_0, 0.304_947_2, 0.141_288_9, 0.3];
    /// Sombra danger leve (botoes destrutivos) -- RGB linear.
    pub const SHADOW_DANGER: [f32; 4] = [0.938_685_7, 0.165_132_2, 0.165_132_2, 0.25];

    /// Compat shim Layer 4.1 (callers antigos importam de raiz). Linear.
    pub const EMERALD_600_RGB3: [f32; 3] = [0.001_517_6, 0.304_987_3, 0.141_263_3];
}

// ---------------------------------------------------------------------------
// Re-export tokens como `color::*` para retro-compat com call sites antigos
// que faziam `color::EMERALD_600`. Esses sites continuam compilando sem
// alterar a string -- so trocam `crate::color::X` por `LFTokens::X` quando
// a migracao completar.
// ---------------------------------------------------------------------------

/// Modulo de compatibilidade: re-exporta tokens com nomes flat antigos
/// (`EMERALD_600`, `PEARL`, `SHADOW_BLACK`, ...). Util para evitar churn
/// em call sites quando refatoramos pra LFTokens.
pub mod color {
    use super::LFTokens as T;

    pub const INK_DEEP_SRGB:    [f32; 4] = T::INK_DEEP_SRGB;
    pub const PANEL_HI_SRGB:    [f32; 4] = T::PANEL_HI_SRGB;
    pub const PANEL_SRGB:       [f32; 4] = T::PANEL_SRGB;
    pub const EMERALD_600_SRGB: [f32; 4] = T::EMERALD_600_SRGB;
    pub const EMERALD_500_SRGB: [f32; 4] = T::EMERALD_500_SRGB;
    pub const PEARL_SRGB:       [f32; 4] = T::PEARL_SRGB;
    pub const MUTED_SRGB:       [f32; 4] = T::MUTED_SRGB;
    pub const DANGER_SRGB:      [f32; 4] = T::DANGER_SRGB;

    pub const INK_DEEP:    [f32; 4] = T::INK_DEEP;
    pub const PANEL_HI:    [f32; 4] = T::PANEL_HI;
    pub const PANEL:       [f32; 4] = T::PANEL;
    pub const EMERALD_600: [f32; 4] = T::EMERALD_600;
    pub const EMERALD_500: [f32; 4] = T::EMERALD_500;
    pub const PEARL:       [f32; 4] = T::PEARL;
    pub const MUTED:       [f32; 4] = T::MUTED;
    pub const DANGER:      [f32; 4] = T::DANGER;
    pub const TRANSPARENT: [f32; 4] = T::TRANSPARENT;

    pub const SHADOW_BLACK:  [f32; 4] = T::SHADOW_BLACK;
    pub const SHADOW_ACCENT: [f32; 4] = T::SHADOW_ACCENT;
    pub const SHADOW_DANGER: [f32; 4] = T::SHADOW_DANGER;

    pub use super::LFColor as Col;
    pub fn srgb_to_linear(s: [f32; 4]) -> [f32; 4] {
        super::LFColor::srgb_to_linear(s)
    }
    pub fn srgb_to_linear_channel(c: f32) -> f32 {
        super::LFColor::srgb_to_linear_channel(c)
    }
}

// ---------------------------------------------------------------------------
// LFColor -- conversao sRGB <-> linear
// ---------------------------------------------------------------------------

/// Helpers de conversao de cor. Curva oficial IEC 61966-2-1.
pub struct LFColor;

impl LFColor {
    /// Converte 1 canal sRGB normalizado (0..1) para linear (0..1).
    /// Usar em runtime quando carregar cor de usuario (hex picker, theme).
    pub fn srgb_to_linear_channel(c: f32) -> f32 {
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    /// Converte uma cor sRGB `[f32; 4]` para linear (alpha passa intacto).
    /// As constantes em `LFTokens` ja sao o resultado dessa conversao.
    pub fn srgb_to_linear(s: [f32; 4]) -> [f32; 4] {
        [
            Self::srgb_to_linear_channel(s[0]),
            Self::srgb_to_linear_channel(s[1]),
            Self::srgb_to_linear_channel(s[2]),
            s[3],
        ]
    }
}

// ---------------------------------------------------------------------------
// LFGeometry -- pixel <-> NDC helpers
// ---------------------------------------------------------------------------
//
// Renderers trabalham em NDC `[-1..+1]`. Widgets / demos pensam em pixels
// top-left. Estas funcoes encapsulam a conversao para nao ser
// re-implementada em cada bin. `viewport` e o tamanho do canvas em pixels.

/// Helpers de geometria pixel <-> NDC. Sao puros (sem state), expostos
/// como funcoes associadas para namespacing.
pub struct LFGeometry;

impl LFGeometry {
    /// Converte um tamanho (largura, altura) em pixels para **half-size** em NDC.
    pub fn px_size_to_ndc(w_px: f32, h_px: f32, viewport: [f32; 2]) -> [f32; 2] {
        [w_px / (viewport[0] * 0.5), h_px / (viewport[1] * 0.5)]
    }

    /// Converte um centro em pixels (origem top-left, y cresce pra baixo) em NDC.
    pub fn px_center_to_ndc(cx_px: f32, cy_px: f32, viewport: [f32; 2]) -> [f32; 2] {
        let x = (cx_px / (viewport[0] * 0.5)) - 1.0;
        let y = 1.0 - (cy_px / (viewport[1] * 0.5));
        [x, y]
    }

    /// Converte um offset CSS-style (positivo = direita/baixo) para NDC.
    pub fn px_offset_to_ndc(dx_px: f32, dy_px: f32, viewport: [f32; 2]) -> [f32; 2] {
        [dx_px / (viewport[0] * 0.5), dy_px / (viewport[1] * 0.5)]
    }

    /// Converte um raio (border / corner / shadow) em pixels para NDC. Usa o
    /// eixo Y como base -- mantem a "feel" consistente com unidades CSS px.
    pub fn px_to_ndc_radius(px: f32, viewport_height: f32) -> f32 {
        px / (viewport_height * 0.5)
    }
}

// ---------------------------------------------------------------------------
// LumoTheme -- runtime light/dark switch (A13)
// ---------------------------------------------------------------------------
//
// Decisao A13: Luiz reportou bar visualmente ruim + tema escuro indesejado.
// Default agora eh LIGHT (memory feedback_zero_neon_glow: validar AMBOS
// temas mentalmente; sombras pretas neutras sem glow colorido).
//
// Cores armazenadas como hex 0xRRGGBB (sem alpha). Alpha aplicado on use.
// Render layer converte hex -> sRGB -> linear via `LFColor` quando precisa
// alimentar GPU.

/// Enum binario: Light (default) ou Dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LumoTheme {
    Light,
    Dark,
}

/// Paleta completa pra um tema. Hex 0xRRGGBB.
#[derive(Debug, Clone, Copy)]
pub struct LumoColors {
    /// Background principal (bar, painel).
    pub bg: u32,
    /// Hover background (sutil).
    pub bg_subtle: u32,
    /// Texto principal.
    pub fg: u32,
    /// Texto secundario / muted.
    pub fg_subtle: u32,
    /// Accent emerald (workspace ativo, dot brand).
    pub accent: u32,
    /// Accent hover / fundo emerald translucido.
    pub accent_subtle: u32,
    /// 1px linha sutil (border-bottom da bar).
    pub border: u32,
    /// Sombra preta neutra. Alpha aplicado quando usada.
    pub shadow: u32,
}

impl LumoColors {
    /// Tema light -- pearl muito claro, ink ainda legivel, emerald-600 saturado.
    pub const fn light() -> Self {
        Self {
            bg:            0x00FAFAFA, // pearl muito claro (#FAFAFA)
            bg_subtle:     0x00F0F0F2, // hover stretch
            fg:            0x0018181B, // ink claro (Tailwind zinc-900)
            fg_subtle:     0x006B7280, // zinc-500
            accent:        0x00059669, // emerald-600
            accent_subtle: 0x00D1FAE5, // emerald-100 (hover wash)
            border:        0x00E5E7EB, // zinc-200
            shadow:        0x00000000, // alpha aplicado on use
        }
    }

    /// Tema dark -- ink_deep, pearl no fg, emerald-500 mais vivo.
    pub const fn dark() -> Self {
        Self {
            bg:            0x000A0A0C, // ink_deep
            bg_subtle:     0x001F2024, // hover panel
            fg:            0x00F5F5F7, // pearl
            fg_subtle:     0x009CA3AF, // zinc-400
            accent:        0x0010B981, // emerald-500 (mais vibrante no escuro)
            accent_subtle: 0x00064E3B, // emerald-900
            border:        0x002A2A2E, // hairline
            shadow:        0x00000000,
        }
    }

    /// Converte hex 0xRRGGBB pra `[f32; 4]` sRGB normalizado com alpha 1.0.
    /// Util pra interop com tiny-skia/wgpu sem precisar inline math no caller.
    pub fn hex_to_srgb(hex: u32) -> [f32; 4] {
        let r = ((hex >> 16) & 0xff) as f32 / 255.0;
        let g = ((hex >> 8) & 0xff) as f32 / 255.0;
        let b = (hex & 0xff) as f32 / 255.0;
        [r, g, b, 1.0]
    }

    /// Hex 0xRRGGBB -> linear `[f32; 4]` pronto pra surface sRGB.
    pub fn hex_to_linear(hex: u32) -> [f32; 4] {
        LFColor::srgb_to_linear(Self::hex_to_srgb(hex))
    }
}

/// Le `LUMO_THEME` do env. Default = Light (decisao A13).
pub fn current_theme() -> LumoTheme {
    match std::env::var("LUMO_THEME").as_deref() {
        Ok("dark") | Ok("Dark") | Ok("DARK") => LumoTheme::Dark,
        _ => LumoTheme::Light,
    }
}

/// Paleta resolvida do tema atual. Lida do env a cada chamada -- caller
/// pode cachear se hot path (geralmente init/redraw eh ok).
pub fn current_colors() -> LumoColors {
    match current_theme() {
        LumoTheme::Light => LumoColors::light(),
        LumoTheme::Dark => LumoColors::dark(),
    }
}

/// Clear color do compositor em linear `[f32; 4]`. DRM/winit usam.
/// Centralizado aqui pra trocar com env -- nao duplicar em backend.
pub fn clear_color_linear() -> [f32; 4] {
    LumoColors::hex_to_linear(current_colors().bg)
}

/// Cor da mascara de cantos do output. **Sempre preto neutro** -- nao
/// muda com tema (eh moldura fisica do display, nao chrome).
/// Alpha 1.0 pra opacificar cantos arredondados.
pub const CORNER_MASK_COLOR_LINEAR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

// Flat aliases para retro-compat com call sites antigos.
pub use LFGeometry as _Geom;

/// Re-export flat (compat). Prefira `LFGeometry::px_size_to_ndc`.
pub fn px_size_to_ndc(w_px: f32, h_px: f32, viewport: [f32; 2]) -> [f32; 2] {
    LFGeometry::px_size_to_ndc(w_px, h_px, viewport)
}
/// Re-export flat (compat). Prefira `LFGeometry::px_center_to_ndc`.
pub fn px_center_to_ndc(cx_px: f32, cy_px: f32, viewport: [f32; 2]) -> [f32; 2] {
    LFGeometry::px_center_to_ndc(cx_px, cy_px, viewport)
}
/// Re-export flat (compat). Prefira `LFGeometry::px_offset_to_ndc`.
pub fn px_offset_to_ndc(dx_px: f32, dy_px: f32, viewport: [f32; 2]) -> [f32; 2] {
    LFGeometry::px_offset_to_ndc(dx_px, dy_px, viewport)
}
/// Re-export flat (compat). Prefira `LFGeometry::px_to_ndc_radius`.
pub fn px_to_ndc_radius(px: f32, viewport_height: f32) -> f32 {
    LFGeometry::px_to_ndc_radius(px, viewport_height)
}
