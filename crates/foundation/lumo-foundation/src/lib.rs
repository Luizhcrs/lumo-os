//! # lumo-foundation
//!
//! Proposito: Design tokens, color helpers e geometria NDC. Zero deps wgpu/winit.
//!
//! ## Invariantes
//! - Cores em campos sem sufixo sao linear space (nao sRGB) — ver I-10.
//! - Zero dependencias de wgpu/winit/smithay: pode ser usada por qualquer crate acima.
//! - LFColor::srgb_to_linear obrigatoria pra qualquer cor vinda de input externo (theme override).
//!
//! ## Memory refs
//! - [[feedback-design-lapidado]]
//! - [[project-lumo-os]]

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
    /// A18: pill background RGB (0xRRGGBB). Light: ink escuro
    /// invertido pra destaque sobre fundo claro; Dark: pearl
    /// translucido sutil sobre fundo escuro.
    pub pill_bg: u32,
    /// A18: pill background alpha (0..0xFF). Light = 0xEE (semi opaco
    /// escuro contraste forte); Dark = 0x22 (sutileza sobre AMOLED).
    pub // A18.1 alpha 0xEE -> 0xFF (opaco, sem blend artifacts)
            pill_bg_alpha: u8,
    /// A18: pill foreground RGB (0xRRGGBB). Pearl em ambos os temas
    /// — light pill eh escuro entao texto branco; dark pill eh pearl
    /// translucido com texto pearl.
    pub pill_fg: u32,
    /// A18: pill shadow alpha (0..0xFF). 0x40 = 25% black drop.
    pub pill_shadow_alpha: u8,
    /// A18: separator dot color RGB (`#FFFFFF66` = pearl alpha 0x66).
    pub pill_sep: u32,
    /// A18: separator dot alpha.
    pub pill_sep_alpha: u8,
}

impl LumoColors {
    /// Tema light -- pearl muito claro, ink ainda legivel, emerald-600 saturado.
    pub const fn light() -> Self {
        Self {
            bg:            0x00FAFAFA, // pearl muito claro (#FAFAFA)
            bg_subtle:     0x00F0F0F2, // hover stretch
            fg:            0x0018181B, // ink claro (Tailwind zinc-900)
            fg_subtle:     0x006B7280, // zinc-500
            accent:        0x003B82F6, // Samsung adaptive blue (One UI 7 inspired)
            accent_subtle: 0x00DBEAFE, // blue-100 hover wash
            border:        0x00E5E7EB, // zinc-200
            shadow:        0x00000000, // alpha aplicado on use
            // A18 pill spec: pill escuro #1F1F22 alpha EE -> contraste
            // invertido sobre bg pearl, vira destaque tipo Dynamic Island.
            pill_bg:           0x001F1F22,
            pill_bg_alpha:     0xE0, // A19.15 transparencia leve (shader demultiply correto)
            pill_fg:           0x00F5F5F7, // pearl sobre pill escuro
            pill_shadow_alpha: 0x40,       // 25% preto neutro
            pill_sep:          0x00FFFFFF, // dot middle separator
            pill_sep_alpha:    0x66,       // 40% pearl
        }
    }

    /// Tema dark -- ink_deep, pearl no fg, emerald-500 mais vivo.
    pub const fn dark() -> Self {
        Self {
            bg:            0x000F1419, // dark Samsung AMOLED-style // ink_deep
            bg_subtle:     0x001F2024, // hover panel
            fg:            0x00F5F5F7, // pearl
            fg_subtle:     0x009CA3AF, // zinc-400
            accent:        0x0060A5FA, // blue-400 (mais vibrante no dark AMOLED)
            accent_subtle: 0x001E3A8A, // blue-900
            border:        0x002A2A2E, // hairline
            shadow:        0x00000000,
            // A18 pill spec dark: pearl alpha 0x22 sutil sobre AMOLED.
            // Pill bg quase invisivel — relevo dado pela sombra preta neutra.
            pill_bg:           0x00FFFFFF,
            pill_bg_alpha:     0xCC, // A19.15 dark mantem 80%
            pill_fg:           0x00F5F5F7,
            pill_shadow_alpha: 0x40,
            pill_sep:          0x00FFFFFF,
            pill_sep_alpha:    0x66,
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

    /// Background do tema atual ja em linear `[f32; 4]`. A14 helper:
    /// corner mask pinta cantos na MESMA cor do clear, somando
    /// "invisivel". Antes era preto fixo -> em light theme aparecia
    /// ponto preto nos cantos (Luiz reportou).
    pub fn bg_as_linear_rgba(&self) -> [f32; 4] {
        Self::hex_to_linear(self.bg)
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

/// Cor da mascara de cantos do output. A14: ANTES era preto hardcoded
/// -> em light theme aparecia ponto preto nos cantos (Luiz reportou).
/// AGORA: pinta cantos na MESMA cor do clear background -> some sobre
/// o fundo do compositor em qualquer tema.
///
/// Funcao runtime (le tema corrente) em vez de constante, porque tema
/// vem do env. Custo: 1 env lookup + 4 multiplicacoes por frame —
/// desprezivel.
pub fn corner_mask_color_linear() -> [f32; 4] {
    current_colors().bg_as_linear_rgba()
}

/// Legacy constante mantida pra compat. Aponta pra preto neutro mas
/// novos call sites devem usar `corner_mask_color_linear()` runtime.
#[deprecated(note = "use corner_mask_color_linear() pra theme-aware")]
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

// ---------------------------------------------------------------------------
// LumoTokens (runtime) -- carregavel de disco / TOML
// ---------------------------------------------------------------------------
//
// Diferente de LFTokens (compile-time consts), LumoTokens armazena
// o tema ativo como dados runtime. Lido de ~/.config/lumo/theme.toml,
// com fallback para as constantes do design system quando o arquivo
// nao existe ou esta invalido.
//
// Formato theme.toml:
//   [theme]
//   mode = "light"   # ou "dark"
//
//   [colors]
//   accent      = "#3B82F6"
//   ink_deep    = "#0a0a0c"
//   pill_bg     = "#1F1F22"
//   ...outros tokens opcionais...

#[derive(Debug, Clone)]
pub struct LumoTokens {
    pub mode: LumoTheme,
    /// Cor accent (hex 0xRRGGBB). Sobrescreve accent da paleta base.
    pub accent: Option<u32>,
    /// ink_deep override (hex 0xRRGGBB). None = usa paleta padrao.
    pub ink_deep: Option<u32>,
    /// pill_bg override (hex 0xRRGGBB).
    pub pill_bg: Option<u32>,
    /// R4: familia de fonte sans-serif. None = usa pick_font_family padrao.
    /// Configuravel via [fonts] font_sans = "Inter" em theme.toml.
    pub font_sans: Option<String>,
    /// R4: familia de fonte mono. None = usa pick_font_family padrao.
    /// Configuravel via [fonts] font_mono = "JetBrainsMono Nerd Font" em theme.toml.
    pub font_mono: Option<String>,
}

/// Erros de I/O ao ler/escrever theme.toml.
#[derive(Debug)]
pub enum TokenError {
    Io(std::io::Error),
    Toml(String),
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenError::Io(e) => write!(f, "IO: {e}"),
            TokenError::Toml(s) => write!(f, "TOML: {s}"),
        }
    }
}

impl LumoTokens {
    /// Retorna o path padrao: ~/.config/lumo/theme.toml
    pub fn config_path() -> Option<std::path::PathBuf> {
        let home = std::env::var_os("HOME")?;
        let mut p = std::path::PathBuf::from(home);
        p.push(".config/lumo/theme.toml");
        Some(p)
    }

    /// Le ~/.config/lumo/theme.toml. Fallback para defaults se nao existe
    /// ou se o arquivo esta malformado.
    pub fn load_from_disk() -> Self {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return Self::default_tokens(),
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => return Self::default_tokens(),
        };
        Self::parse_toml(&text).unwrap_or_else(|_| Self::default_tokens())
    }

    /// Salva o estado atual pra ~/.config/lumo/theme.toml.
    pub fn save_to_disk(&self) -> Result<(), TokenError> {
        let path = Self::config_path().ok_or_else(|| {
            TokenError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "HOME nao definido",
            ))
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(TokenError::Io)?;
        }
        let content = self.to_toml();
        std::fs::write(&path, content).map_err(TokenError::Io)?;
        Ok(())
    }

    /// Resolve tokens como LumoColors, aplicando overrides sobre a paleta base.
    pub fn resolve(&self) -> LumoColors {
        let mut colors = match self.mode {
            LumoTheme::Light => LumoColors::light(),
            LumoTheme::Dark => LumoColors::dark(),
        };
        if let Some(accent) = self.accent {
            colors.accent = accent;
        }
        if let Some(ink) = self.ink_deep {
            colors.bg = ink;
        }
        if let Some(pill) = self.pill_bg {
            colors.pill_bg = pill;
        }
        colors
    }

    fn default_tokens() -> Self {
        let mode = match std::env::var("LUMO_THEME").as_deref() {
            Ok("dark") | Ok("Dark") | Ok("DARK") => LumoTheme::Dark,
            _ => LumoTheme::Light,
        };
        Self { mode, accent: None, ink_deep: None, pill_bg: None, font_sans: None, font_mono: None }
    }

    fn parse_toml(text: &str) -> Result<Self, TokenError> {
        let mut mode = LumoTheme::Light;
        let mut accent: Option<u32> = None;
        let mut ink_deep: Option<u32> = None;
        let mut pill_bg: Option<u32> = None;
        let mut font_sans: Option<String> = None;
        let mut font_mono: Option<String> = None;

        // Parser minimalista: nao depende de serde para manter zero deps extras.
        // Percorre linhas, extrai pares key = "value".
        let mut section = "";
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                section = &line[1..line.len() - 1];
                continue;
            }
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some(eq) = line.find('=') else { continue };
            let key = line[..eq].trim();
            let val = line[eq + 1..].trim().trim_matches('"').trim_matches('\'');
            match (section, key) {
                ("theme", "mode") => {
                    mode = match val {
                        "dark" | "Dark" | "DARK" => LumoTheme::Dark,
                        _ => LumoTheme::Light,
                    };
                }
                ("colors", "accent") => accent = parse_hex_color(val),
                ("colors", "ink_deep") => ink_deep = parse_hex_color(val),
                ("colors", "pill_bg") => pill_bg = parse_hex_color(val),
                // R4: tokens de fonte configuravel.
                ("fonts", "font_sans") if !val.is_empty() => font_sans = Some(val.to_string()),
                ("fonts", "font_mono") if !val.is_empty() => font_mono = Some(val.to_string()),
                _ => {}
            }
        }
        Ok(Self { mode, accent, ink_deep, pill_bg, font_sans, font_mono })
    }

    fn to_toml(&self) -> String {
        let mut out = String::new();
        out.push_str("[theme]\n");
        let mode_str = match self.mode {
            LumoTheme::Light => "light",
            LumoTheme::Dark => "dark",
        };
        out.push_str(&format!("mode = \"{mode_str}\"\n\n"));
        out.push_str("[colors]\n");
        if let Some(a) = self.accent {
            out.push_str(&format!("accent = \"#{:06X}\"\n", a));
        }
        if let Some(i) = self.ink_deep {
            out.push_str(&format!("ink_deep = \"#{:06X}\"\n", i));
        }
        if let Some(p) = self.pill_bg {
            out.push_str(&format!("pill_bg = \"#{:06X}\"\n", p));
        }
        // R4: fontes (somente escreve se nao default).
        if self.font_sans.is_some() || self.font_mono.is_some() {
            out.push_str("\n[fonts]\n");
            if let Some(ref fs) = self.font_sans {
                out.push_str(&format!("font_sans = \"{fs}\"\n"));
            }
            if let Some(ref fm) = self.font_mono {
                out.push_str(&format!("font_mono = \"{fm}\"\n"));
            }
        }
        out
    }

    /// R4: retorna familia sans-serif configurada ou default "Inter".
    pub fn effective_font_sans(&self) -> &str {
        self.font_sans.as_deref().unwrap_or("Inter")
    }

    /// R4: retorna familia mono configurada ou default "JetBrainsMono Nerd Font".
    pub fn effective_font_mono(&self) -> &str {
        self.font_mono.as_deref().unwrap_or("JetBrainsMono Nerd Font")
    }
}

/// Parseia "#RRGGBB" ou "RRGGBB" em 0xRRGGBB. None se invalido.
fn parse_hex_color(s: &str) -> Option<u32> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 {
        return None;
    }
    u32::from_str_radix(s, 16).ok()
}

// ---------------------------------------------------------------------------
// watch_theme -- file watcher via notify
// ---------------------------------------------------------------------------

/// Inicia um thread que observa ~/.config/lumo/theme.toml e chama
/// `callback` com o LumoTokens atualizado sempre que o arquivo muda.
///
/// Usa `notify` crate (backend inotify no Linux). O callback roda
/// em thread separada; use mpsc para sincronizar com o main loop.
///
/// Retorna silenciosamente se o path de config nao puder ser determinado
/// ou se o watcher falhar (nao bloqueia o boot do client).
pub fn watch_theme<F: Fn(LumoTokens) + Send + 'static>(callback: F) {
    let Some(path) = LumoTokens::config_path() else { return };
    std::thread::Builder::new()
        .name("lumo-theme-watcher".into())
        .spawn(move || {
            use notify::{EventKind, RecursiveMode, Watcher};
            use notify::event::{ModifyKind, CreateKind};

            // Garante que o diretorio existe antes de registrar o watcher.
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }

            let (tx, rx) = std::sync::mpsc::channel();
            let mut watcher = match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(ev) = res {
                    let _ = tx.send(ev);
                }
            }) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("[lumo-foundation] watch_theme: watcher init falhou: {e}");
                    return;
                }
            };

            // Observa o diretorio pai para capturar criacao/atomic-rename do arquivo.
            let watch_dir = path.parent().unwrap_or(path.as_path());
            if let Err(e) = watcher.watch(watch_dir, RecursiveMode::NonRecursive) {
                eprintln!("[lumo-foundation] watch_theme: watch({}) falhou: {e}", watch_dir.display());
                return;
            }

            for event in rx {
                let is_theme_file = event.paths.iter().any(|p| p == &path);
                if !is_theme_file {
                    continue;
                }
                let relevant = matches!(
                    event.kind,
                    EventKind::Modify(ModifyKind::Data(_))
                    | EventKind::Modify(ModifyKind::Any)
                    | EventKind::Create(CreateKind::File)
                    | EventKind::Create(CreateKind::Any)
                );
                if relevant {
                    let tokens = LumoTokens::load_from_disk();
                    callback(tokens);
                }
            }
        })
        .ok();
}

// ---------------------------------------------------------------------------
// Tests LumoTokens
// ---------------------------------------------------------------------------

#[cfg(test)]
mod theme_tests {
    use super::*;

    #[test]
    fn load_fallback_when_no_file() {
        // Com HOME apontando pra dir inexistente, deve retornar defaults.
        std::env::set_var("HOME", "/tmp/lumo_test_nonexistent_xyz");
        let tokens = LumoTokens::load_from_disk();
        assert!(matches!(tokens.mode, LumoTheme::Light));
        assert!(tokens.accent.is_none());
    }

    #[test]
    fn parse_toml_basic() {
        let toml = "[theme]\nmode = \"dark\"\n\n[colors]\naccent = \"#3B82F6\"\nink_deep = \"#0a0a0c\"\n";
        let tokens = LumoTokens::parse_toml(toml).unwrap();
        assert!(matches!(tokens.mode, LumoTheme::Dark));
        assert_eq!(tokens.accent, Some(0x3B82F6));
        assert_eq!(tokens.ink_deep, Some(0x0a0a0c));
    }

    #[test]
    fn parse_hex_color_valid() {
        assert_eq!(parse_hex_color("#FF6B35"), Some(0xFF6B35));
        assert_eq!(parse_hex_color("3B82F6"), Some(0x3B82F6));
        assert_eq!(parse_hex_color("#ZZZZZZ"), None);
    }

    #[test]
    fn roundtrip_toml() {
        let t = LumoTokens {
            mode: LumoTheme::Dark,
            accent: Some(0xFF6B35),
            ink_deep: None,
            pill_bg: Some(0x1F1F22),
            font_sans: None,
            font_mono: None,
        };
        let toml = t.to_toml();
        let t2 = LumoTokens::parse_toml(&toml).unwrap();
        assert!(matches!(t2.mode, LumoTheme::Dark));
        assert_eq!(t2.accent, Some(0xFF6B35));
        assert_eq!(t2.pill_bg, Some(0x1F1F22));
        assert!(t2.ink_deep.is_none());
    }

    #[test]
    fn resolve_applies_overrides() {
        let t = LumoTokens {
            mode: LumoTheme::Light,
            accent: Some(0xABCDEF),
            ink_deep: None,
            pill_bg: None,
            font_sans: None,
            font_mono: None,
        };
        let colors = t.resolve();
        assert_eq!(colors.accent, 0xABCDEF);
    }

    #[test]
    fn default_font_fields_are_none() {
        let t = LumoTokens {
            mode: LumoTheme::Dark,
            accent: None,
            ink_deep: None,
            pill_bg: None,
            font_sans: None,
            font_mono: None,
        };
        assert!(t.font_sans.is_none());
        assert!(t.font_mono.is_none());
    }
}

// ---------------------------------------------------------------------------
// Tests LFColor + LFGeometry + LumoColors + LFTokens (L4)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod lf_color_tests {
    use super::*;

    #[test]
    fn srgb_to_linear_channel_zero_is_zero() {
        assert!((LFColor::srgb_to_linear_channel(0.0)).abs() < 1e-6);
    }

    #[test]
    fn srgb_to_linear_channel_one_is_one() {
        let v = LFColor::srgb_to_linear_channel(1.0);
        assert!((v - 1.0).abs() < 1e-4, "v={v}");
    }

    #[test]
    fn srgb_to_linear_channel_is_monotonic() {
        let a = LFColor::srgb_to_linear_channel(0.4);
        let b = LFColor::srgb_to_linear_channel(0.5);
        assert!(a < b, "should be monotonic: a={a} b={b}");
    }

    #[test]
    fn srgb_to_linear_preserves_alpha() {
        let s = [0.5, 0.5, 0.5, 0.75];
        let l = LFColor::srgb_to_linear(s);
        assert!((l[3] - 0.75).abs() < 1e-6);
    }

    #[test]
    fn srgb_to_linear_darkens_midgray() {
        // sRGB 0.5 -> linear < 0.5 (gamma correction darkens mid tones)
        let v = LFColor::srgb_to_linear_channel(0.5);
        assert!(v < 0.5, "linear should be darker: v={v}");
    }

    #[test]
    fn srgb_to_linear_low_value_linear_segment() {
        // Below 0.04045 uses linear segment c/12.92
        let v = LFColor::srgb_to_linear_channel(0.01);
        let expected = 0.01 / 12.92;
        assert!((v - expected).abs() < 1e-6);
    }
}

#[cfg(test)]
mod lf_geometry_tests {
    use super::*;

    #[test]
    fn px_size_to_ndc_fullscreen() {
        // 1920x1080 fills entire viewport -> NDC size [2.0, 2.0]
        let result = LFGeometry::px_size_to_ndc(1920.0, 1080.0, [1920.0, 1080.0]);
        assert!((result[0] - 2.0).abs() < 1e-5);
        assert!((result[1] - 2.0).abs() < 1e-5);
    }

    #[test]
    fn px_center_to_ndc_center_is_origin() {
        let result = LFGeometry::px_center_to_ndc(960.0, 540.0, [1920.0, 1080.0]);
        assert!((result[0]).abs() < 1e-4, "x={}", result[0]);
        assert!((result[1]).abs() < 1e-4, "y={}", result[1]);
    }

    #[test]
    fn px_center_to_ndc_top_left_corner() {
        let result = LFGeometry::px_center_to_ndc(0.0, 0.0, [1920.0, 1080.0]);
        assert!((result[0] - (-1.0)).abs() < 1e-4);
        assert!((result[1] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn px_offset_to_ndc_zero_is_zero() {
        let result = LFGeometry::px_offset_to_ndc(0.0, 0.0, [1920.0, 1080.0]);
        assert!((result[0]).abs() < 1e-6);
        assert!((result[1]).abs() < 1e-6);
    }

    #[test]
    fn px_to_ndc_radius_half_height() {
        // radius = 540px on 1080 viewport = 1.0 NDC
        let r = LFGeometry::px_to_ndc_radius(540.0, 1080.0);
        assert!((r - 1.0).abs() < 1e-5);
    }
}

#[cfg(test)]
mod lumo_colors_tests {
    use super::*;

    #[test]
    fn light_has_bright_background() {
        let c = LumoColors::light();
        let r = (c.bg >> 16) & 0xFF;
        // #FAFAFA -> r=0xFA=250
        assert!(r > 200, "light bg should be bright, r={r}");
    }

    #[test]
    fn dark_has_dark_background() {
        let c = LumoColors::dark();
        let r = (c.bg >> 16) & 0xFF;
        // Dark bg should have low r component
        assert!(r < 50, "dark bg should be dark, r={r}");
    }

    #[test]
    fn hex_to_srgb_emerald_red_component() {
        // 0x059669 -> r=5/255
        let s = LumoColors::hex_to_srgb(0x059669);
        assert!((s[0] - 5.0 / 255.0).abs() < 1e-4);
        assert!((s[3] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn hex_to_srgb_black_is_zeros() {
        let s = LumoColors::hex_to_srgb(0x000000);
        assert!(s[0].abs() < 1e-6);
        assert!(s[1].abs() < 1e-6);
        assert!(s[2].abs() < 1e-6);
    }

    #[test]
    fn hex_to_linear_white_is_one() {
        let l = LumoColors::hex_to_linear(0xFFFFFF);
        assert!((l[0] - 1.0).abs() < 1e-4);
        assert!((l[1] - 1.0).abs() < 1e-4);
        assert!((l[2] - 1.0).abs() < 1e-4);
    }

    #[test]
    fn bg_as_linear_rgba_matches_hex_to_linear() {
        let c = LumoColors::light();
        let a = c.bg_as_linear_rgba();
        let b = LumoColors::hex_to_linear(c.bg);
        assert!((a[0] - b[0]).abs() < 1e-6);
    }
}

#[cfg(test)]
mod lf_tokens_tests {
    use super::*;

    #[test]
    fn transparent_alpha_is_zero() {
        assert!((LFTokens::TRANSPARENT[3]).abs() < 1e-6);
    }

    #[test]
    fn shadow_black_rgb_are_zero() {
        assert!((LFTokens::SHADOW_BLACK[0]).abs() < 1e-6);
        assert!((LFTokens::SHADOW_BLACK[1]).abs() < 1e-6);
        assert!((LFTokens::SHADOW_BLACK[2]).abs() < 1e-6);
    }

    #[test]
    fn pearl_srgb_r_near_nominal() {
        // #f5f5f7 -> 0.9607...
        let r = LFTokens::PEARL_SRGB[0];
        assert!((r - 0.960_784_3).abs() < 1e-5, "r={r}");
    }

    #[test]
    fn linear_values_not_exceed_srgb() {
        // Linear representation <= sRGB for non-zero values
        for i in 0..3 {
            assert!(
                LFTokens::PEARL[i] <= LFTokens::PEARL_SRGB[i],
                "linear[{i}] should be <= srgb[{i}]"
            );
        }
    }

    #[test]
    fn color_module_emerald_matches_lf_tokens() {
        assert_eq!(color::EMERALD_600, LFTokens::EMERALD_600);
    }
}

// ---------------------------------------------------------------------------
// BarLayout -- data-driven layout carregado de layout.toml (F1)
// ---------------------------------------------------------------------------

/// Spec de uma pill individual na bar.
#[derive(Debug, Clone, PartialEq)]
pub struct PillSpec {
    pub id: String,
    pub width: Option<f32>,
}

impl PillSpec {
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string(), width: None }
    }
    pub fn with_width(id: &str, w: f32) -> Self {
        Self { id: id.to_string(), width: Some(w) }
    }
}

/// Layout completo da bar. Lido de layout.toml; fallback para default se ausente.
#[derive(Debug, Clone)]
pub struct BarLayout {
    pub height: u32,
    pub padding_x: f32,
    pub pill_gap: f32,
    pub pill_radius: f32,
    pub margin_top: f32,
    pub margin_x: f32,
    pub left_pills: Vec<PillSpec>,
    pub right_pills: Vec<PillSpec>,
}

impl BarLayout {
    /// Valores identicos aos hardcoded em tokens.rs -- zero regressao visual.
    pub fn default_layout() -> Self {
        Self {
            height: 40,
            padding_x: 14.0,
            pill_gap: 8.0,
            pill_radius: 14.0,
            margin_top: 6.0,
            margin_x: 14.0,
            left_pills: vec![
                PillSpec::with_width("brand", 88.0),
                PillSpec::new("appmenu"),
            ],
            right_pills: vec![
                PillSpec::with_width("battery", 32.0),
                PillSpec::with_width("brightness", 24.0),
                PillSpec::with_width("wifi", 24.0),
                PillSpec::with_width("datetime", 220.0),
            ],
        }
    }

    pub fn find_pill(&self, id: &str) -> Option<&PillSpec> {
        self.left_pills.iter()
            .chain(self.right_pills.iter())
            .find(|p| p.id == id)
    }

    pub fn config_path() -> Option<std::path::PathBuf> {
        let home = std::env::var_os("HOME")?;
        let mut p = std::path::PathBuf::from(home);
        p.push(".config/lumo/layout.toml");
        Some(p)
    }

    pub fn load_from_disk() -> Self {
        let path = match Self::config_path() {
            Some(p) => p,
            None => return Self::default_layout(),
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(_) => return Self::default_layout(),
        };
        Self::parse_toml(&text).unwrap_or_else(|_| Self::default_layout())
    }

    pub fn parse_toml(text: &str) -> Result<Self, String> {
        let mut layout = Self::default_layout();
        let mut section = String::new();
        for raw_line in text.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            if line.starts_with('[') && line.ends_with(']') {
                section = line[1..line.len() - 1].trim().to_string();
                continue;
            }
            if line.starts_with("pills") {
                if let Some(arr_start) = line.find('[') {
                    let pills = parse_pill_array(&line[arr_start..]);
                    match section.as_str() {
                        "bar.left"  => layout.left_pills  = pills,
                        "bar.right" => layout.right_pills = pills,
                        _ => {}
                    }
                }
                continue;
            }
            let Some(eq) = line.find('=') else { continue };
            let key = line[..eq].trim();
            let val = line[eq + 1..].trim().trim_matches('"').trim_matches('\'');
            match (section.as_str(), key) {
                ("bar", "height")      => { if let Ok(v) = val.parse::<u32>() { layout.height      = v; } }
                ("bar", "padding_x")   => { if let Ok(v) = val.parse::<f32>() { layout.padding_x   = v; } }
                ("bar", "pill_gap")    => { if let Ok(v) = val.parse::<f32>() { layout.pill_gap    = v; } }
                ("bar", "pill_radius") => { if let Ok(v) = val.parse::<f32>() { layout.pill_radius = v; } }
                ("bar", "margin_top")  => { if let Ok(v) = val.parse::<f32>() { layout.margin_top  = v; } }
                ("bar", "margin_x")    => { if let Ok(v) = val.parse::<f32>() { layout.margin_x    = v; } }
                _ => {}
            }
        }
        Ok(layout)
    }
}

fn parse_pill_array(arr_str: &str) -> Vec<PillSpec> {
    let mut pills = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    let chars: Vec<char> = arr_str.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '{' {
            if depth == 0 { start = i; }
            depth += 1;
        } else if c == '}' {
            depth -= 1;
            if depth == 0 {
                let item: String = chars[start..=i].iter().collect();
                if let Some(spec) = parse_pill_item(&item) { pills.push(spec); }
            }
        }
    }
    pills
}

fn parse_pill_item(item: &str) -> Option<PillSpec> {
    let inner = item.trim_start_matches('{').trim_end_matches('}').trim();
    let mut id: Option<String> = None;
    let mut width: Option<f32> = None;
    for part in inner.split(',') {
        let kv = part.trim();
        if let Some(eq) = kv.find('=') {
            let k = kv[..eq].trim();
            let v = kv[eq + 1..].trim().trim_matches('"').trim_matches('\'');
            match k {
                "id"    => id = Some(v.to_string()),
                "width" => { if let Ok(w) = v.parse::<f32>() { width = Some(w); } }
                _ => {}
            }
        }
    }
    let id = id?;
    Some(match width {
        Some(w) => PillSpec::with_width(&id, w),
        None    => PillSpec::new(&id),
    })
}

static BAR_LAYOUT_GLOBAL: std::sync::OnceLock<std::sync::Arc<std::sync::RwLock<BarLayout>>> =
    std::sync::OnceLock::new();

pub fn bar_layout_global() -> &'static std::sync::Arc<std::sync::RwLock<BarLayout>> {
    BAR_LAYOUT_GLOBAL.get_or_init(|| {
        std::sync::Arc::new(std::sync::RwLock::new(BarLayout::load_from_disk()))
    })
}

/// Snapshot imutavel do layout atual. Chame por frame (clone e barato).
pub fn current_bar_layout() -> BarLayout {
    bar_layout_global().read().unwrap().clone()
}

/// Inicia thread de filewatcher para layout.toml. Atualiza global e chama callback.
pub fn watch_layout<F: Fn(BarLayout) + Send + 'static>(callback: F) {
    let Some(path) = BarLayout::config_path() else { return };
    std::thread::Builder::new()
        .name("lumo-layout-watcher".into())
        .spawn(move || {
            use notify::{EventKind, RecursiveMode, Watcher};
            use notify::event::{ModifyKind, CreateKind};
            if let Some(parent) = path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let (tx, rx) = std::sync::mpsc::channel();
            let mut watcher = match notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                if let Ok(ev) = res { let _ = tx.send(ev); }
            }) {
                Ok(w) => w,
                Err(e) => {
                    eprintln!("[lumo-foundation] watch_layout: watcher init falhou: {e}");
                    return;
                }
            };
            let watch_dir = path.parent().unwrap_or(path.as_path());
            if let Err(e) = watcher.watch(watch_dir, RecursiveMode::NonRecursive) {
                eprintln!("[lumo-foundation] watch_layout: watch({}) falhou: {e}", watch_dir.display());
                return;
            }
            for event in rx {
                let is_layout_file = event.paths.iter().any(|p| p == &path);
                if !is_layout_file { continue; }
                let relevant = matches!(
                    event.kind,
                    EventKind::Modify(ModifyKind::Data(_))
                    | EventKind::Modify(ModifyKind::Any)
                    | EventKind::Create(CreateKind::File)
                    | EventKind::Create(CreateKind::Any)
                );
                if relevant {
                    let new_layout = BarLayout::load_from_disk();
                    if let Ok(mut guard) = bar_layout_global().write() {
                        *guard = new_layout.clone();
                    }
                    callback(new_layout);
                }
            }
        })
        .ok();
}

#[cfg(test)]
mod bar_layout_tests {
    use super::*;

    #[test]
    fn default_layout_has_expected_pills() {
        let l = BarLayout::default_layout();
        assert!(l.find_pill("brand").is_some());
        assert!(l.find_pill("battery").is_some());
        assert!(l.find_pill("datetime").is_some());
        assert!(l.find_pill("nonexistent").is_none());
    }

    #[test]
    fn default_layout_dimensions() {
        let l = BarLayout::default_layout();
        assert_eq!(l.height, 40);
        assert!((l.padding_x - 14.0).abs() < 0.01);
        assert!((l.pill_gap - 8.0).abs() < 0.01);
        assert!((l.pill_radius - 14.0).abs() < 0.01);
        assert!((l.margin_top - 6.0).abs() < 0.01);
    }

    #[test]
    fn load_fallback_when_no_file() {
        let layout = BarLayout::load_from_disk();
        assert!(layout.find_pill("brand").is_some());
    }

    #[test]
    fn parse_toml_dimensions() {
        let toml = "[bar]\nheight = 32\npadding_x = 16\n";
        let layout = BarLayout::parse_toml(toml).unwrap();
        assert_eq!(layout.height, 32);
        assert!((layout.padding_x - 16.0).abs() < 0.01);
    }

    #[test]
    fn parse_pill_array_inline() {
        let arr = concat!(
            r#"[{ id = "wifi", width = 24 }, { id = "brand" }]"#
        );
        let pills = super::parse_pill_array(arr);
        assert_eq!(pills.len(), 2);
        assert_eq!(pills[0].id, "wifi");
        assert_eq!(pills[0].width, Some(24.0));
        assert_eq!(pills[1].id, "brand");
        assert!(pills[1].width.is_none());
    }

    #[test]
    fn find_pill_returns_none_for_unknown() {
        let l = BarLayout::default_layout();
        assert!(l.find_pill("unknown_pill_xyz").is_none());
    }
}

// W8.C
pub mod accessibility;
pub use accessibility::{A11yTokens, watch_accessibility};

// W11.A
pub mod i18n;
pub use i18n::I18n;
