//! Bridge lumo-foundation tokens -> iced helpers para Lumo Files.
//!
//! Camadas:
//!   - LumoTheme::light() / dark() retornam um Snapshot fixo (sem env read).
//!   - LumoTheme:: static getters (legacy) ainda funcionam usando o
//!     snapshot Default (dark), pra nao quebrar codigo existente.
//!   - Helpers polish v2: bg_subtle, border, shadow, accent_subtle,
//!     accent_10, accent_30.

use iced::widget::button;
use iced::widget::container;
use iced::{Background, Border, Color};
use lumo_foundation::LFTokens;

/// Converte [f32;4] sRGB normalizado para iced::Color.
fn c(srgb: [f32; 4]) -> Color {
    Color::from_rgba(srgb[0], srgb[1], srgb[2], srgb[3])
}

/// Variante de tema (passada explicitamente; NUNCA lido de env em render).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Variant {
    #[default]
    Dark,
    Light,
}

/// Snapshot imutavel de tema. Construido uma vez em App::new e passado
/// por referencia para todos os modulos de view.
#[derive(Debug, Clone, Copy)]
pub struct ThemeSnapshot {
    pub variant: Variant,
    pub bg: Color,
    pub bg_subtle: Color,
    pub fg: Color,
    pub fg_subtle: Color,
    pub accent: Color,
    pub accent_hover: Color,
    pub accent_subtle: Color,
    pub accent_10: Color,
    pub accent_30: Color,
    pub border: Color,
    pub shadow: Color,
    pub danger: Color,
}

impl ThemeSnapshot {
    pub const fn variant(&self) -> Variant {
        self.variant
    }

    pub fn dark() -> Self {
        let accent = c(LFTokens::EMERALD_600_SRGB);
        Self {
            variant: Variant::Dark,
            bg: c(LFTokens::INK_DEEP_SRGB),
            bg_subtle: c(LFTokens::PANEL_HI_SRGB),
            fg: c(LFTokens::PEARL_SRGB),
            fg_subtle: c(LFTokens::MUTED_SRGB),
            accent,
            accent_hover: c(LFTokens::EMERALD_500_SRGB),
            accent_subtle: with_alpha(accent, 0.18),
            accent_10: with_alpha(accent, 0.10),
            accent_30: with_alpha(accent, 0.30),
            border: Color::from_rgba(0.165, 0.165, 0.18, 1.0),
            shadow: Color::from_rgba(0.0, 0.0, 0.0, 0.40),
            danger: c(LFTokens::DANGER_SRGB),
        }
    }

    pub fn light() -> Self {
        let accent = c(LFTokens::EMERALD_600_SRGB);
        Self {
            variant: Variant::Light,
            bg: Color::from_rgba(0.980, 0.980, 0.980, 1.0),
            bg_subtle: Color::from_rgba(0.941, 0.941, 0.949, 1.0),
            fg: Color::from_rgba(0.094, 0.094, 0.106, 1.0),
            fg_subtle: Color::from_rgba(0.420, 0.447, 0.502, 1.0),
            accent,
            accent_hover: c(LFTokens::EMERALD_500_SRGB),
            accent_subtle: with_alpha(accent, 0.14),
            accent_10: with_alpha(accent, 0.08),
            accent_30: with_alpha(accent, 0.24),
            border: Color::from_rgba(0.898, 0.906, 0.922, 1.0),
            shadow: Color::from_rgba(0.0, 0.0, 0.0, 0.16),
            danger: c(LFTokens::DANGER_SRGB),
        }
    }

    /// Le LUMO_THEME do env apenas para escolha inicial em App::new.
    /// NUNCA chamar isso em render.
    pub fn from_env() -> Self {
        match std::env::var("LUMO_THEME").as_deref() {
            Ok("light") | Ok("Light") | Ok("LIGHT") => Self::light(),
            _ => Self::dark(),
        }
    }
}

fn with_alpha(color: Color, a: f32) -> Color {
    Color { a, ..color }
}

// ---------------------------------------------------------------------------
// LumoTheme legacy static API (used by older view paths in app.rs)
// ---------------------------------------------------------------------------

/// Paleta Lumo como consts iced::Color para uso em widgets.
///
/// IMPORTANTE: esta API retorna sempre cores do tema dark — eh apenas
/// pra compatibilidade com o codigo legado. Codigo novo deve usar
/// `ThemeSnapshot` passado por referencia.
pub struct LumoTheme;

impl LumoTheme {
    pub fn bg() -> Color {
        c(LFTokens::INK_DEEP_SRGB)
    }
    pub fn panel() -> Color {
        c(LFTokens::PANEL_SRGB)
    }
    pub fn panel_hi() -> Color {
        c(LFTokens::PANEL_HI_SRGB)
    }
    pub fn fg() -> Color {
        c(LFTokens::PEARL_SRGB)
    }
    pub fn muted() -> Color {
        c(LFTokens::MUTED_SRGB)
    }
    pub fn accent() -> Color {
        c(LFTokens::EMERALD_600_SRGB)
    }
    pub fn accent_hover() -> Color {
        c(LFTokens::EMERALD_500_SRGB)
    }
    pub fn danger() -> Color {
        c(LFTokens::DANGER_SRGB)
    }

    pub fn sep() -> Color {
        Color::from_rgba(0.165, 0.165, 0.18, 1.0)
    }

    pub fn border() -> Color {
        Self::sep()
    }

    pub fn shadow() -> Color {
        Color::from_rgba(0.0, 0.0, 0.0, 0.40)
    }

    pub fn pill_bg() -> Color {
        with_alpha(c(LFTokens::EMERALD_600_SRGB), 0.20)
    }

    /// Hover wash 8 % accent.
    pub fn accent_8() -> Color {
        with_alpha(c(LFTokens::EMERALD_600_SRGB), 0.08)
    }

    /// Hover wash 10 % accent.
    pub fn accent_10() -> Color {
        with_alpha(c(LFTokens::EMERALD_600_SRGB), 0.10)
    }

    /// Selected wash 18 % accent.
    pub fn accent_subtle() -> Color {
        with_alpha(c(LFTokens::EMERALD_600_SRGB), 0.18)
    }

    /// Fundo accent com alpha 0x20 (hover sidebar).
    pub fn accent_alpha20() -> Color {
        with_alpha(c(LFTokens::EMERALD_600_SRGB), 0x20 as f32 / 255.0)
    }

    /// Fundo accent com alpha 0x40 (selected sidebar).
    pub fn accent_alpha40() -> Color {
        with_alpha(c(LFTokens::EMERALD_600_SRGB), 0x40 as f32 / 255.0)
    }

    /// Fundo accent com alpha 0x30 (selected grid cell).
    pub fn accent_alpha30() -> Color {
        with_alpha(c(LFTokens::EMERALD_600_SRGB), 0x30 as f32 / 255.0)
    }

    /// Hover wash neutra (bg_subtle escuro/claro). Para hover de itens
    /// nao-selecionados (sidebar/toolbar/ctxmenu).
    pub fn hover_bg() -> Color {
        c(LFTokens::PANEL_HI_SRGB)
    }
}

// ---------------------------------------------------------------------------
// ButtonStyle enum + builder
// ---------------------------------------------------------------------------

pub enum ButtonStyle {
    Primary,
    Secondary,
    Ghost,
}

impl ButtonStyle {
    pub fn style(&self) -> button::Style {
        match self {
            ButtonStyle::Primary => button::Style {
                background: Some(Background::Color(LumoTheme::accent())),
                border: Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                text_color: LumoTheme::bg(),
                ..Default::default()
            },
            ButtonStyle::Secondary => button::Style {
                background: Some(Background::Color(LumoTheme::panel_hi())),
                border: Border {
                    color: LumoTheme::sep(),
                    width: 1.0,
                    radius: 8.0.into(),
                },
                text_color: LumoTheme::fg(),
                ..Default::default()
            },
            ButtonStyle::Ghost => button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                border: Border {
                    radius: 8.0.into(),
                    ..Default::default()
                },
                text_color: LumoTheme::fg(),
                ..Default::default()
            },
        }
    }
}

// ---------------------------------------------------------------------------
// ContainerStyle enum + builder
// ---------------------------------------------------------------------------

pub enum ContainerStyle {
    Sidebar,
    Main,
    Toolbar,
}

impl ContainerStyle {
    pub fn style(&self) -> container::Style {
        match self {
            ContainerStyle::Sidebar => container::Style {
                background: Some(Background::Color(LumoTheme::panel())),
                ..Default::default()
            },
            ContainerStyle::Main => container::Style {
                background: Some(Background::Color(LumoTheme::bg())),
                ..Default::default()
            },
            ContainerStyle::Toolbar => container::Style {
                background: Some(Background::Color(LumoTheme::panel())),
                border: Border {
                    color: LumoTheme::sep(),
                    width: 0.0,
                    ..Default::default()
                },
                ..Default::default()
            },
        }
    }
}

// ---------------------------------------------------------------------------
// TextStyle enum (semantic sizes + colors)
// ---------------------------------------------------------------------------

pub enum TextStyle {
    Heading,
    Body,
    Caption,
    Muted,
}

impl TextStyle {
    pub fn color(&self) -> Color {
        match self {
            TextStyle::Heading => LumoTheme::fg(),
            TextStyle::Body => LumoTheme::fg(),
            TextStyle::Caption => LumoTheme::muted(),
            TextStyle::Muted => LumoTheme::muted(),
        }
    }

    pub fn size(&self) -> f32 {
        match self {
            TextStyle::Heading => 16.0,
            TextStyle::Body => 13.0,
            TextStyle::Caption => 11.0,
            TextStyle::Muted => 12.0,
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_dark_distinct_from_light() {
        let d = ThemeSnapshot::dark();
        let l = ThemeSnapshot::light();
        assert_ne!(d.bg, l.bg, "bg deve diferir entre dark e light");
        assert_ne!(d.fg, l.fg, "fg deve diferir entre dark e light");
    }

    #[test]
    fn accent_subtle_has_alpha_below_one() {
        let d = ThemeSnapshot::dark();
        assert!(d.accent_subtle.a < 1.0);
        assert!(d.accent_10.a < d.accent_subtle.a);
    }

    #[test]
    fn shadow_neutro_sem_tint_accent() {
        let d = ThemeSnapshot::dark();
        assert_eq!(d.shadow.r, 0.0, "shadow.r deve ser 0 (neutro)");
        assert_eq!(d.shadow.g, 0.0, "shadow.g deve ser 0 (neutro)");
        assert_eq!(d.shadow.b, 0.0, "shadow.b deve ser 0 (neutro)");
    }
}
