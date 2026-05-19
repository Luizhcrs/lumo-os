//! Bridge lumo-foundation tokens -> iced::Theme customizado.
//!
//! Converte os tokens sRGB de LFTokens para iced::Color e monta
//! iced::Theme::Custom com a paleta Lumo.

use iced::Color;
use lumo_foundation::LFTokens;

/// Converte [f32;4] sRGB normalizado para iced::Color.
/// Os tokens _SRGB ja sao valores sRGB 0..1, iced::Color aceita sRGB diretamente.
fn c(srgb: [f32; 4]) -> Color {
    Color::from_rgba(srgb[0], srgb[1], srgb[2], srgb[3])
}

/// Paleta Lumo como consts iced::Color para uso em widgets.
pub struct LumoTheme;

impl LumoTheme {
    /// Fundo principal da janela.
    pub fn bg() -> Color {
        c(LFTokens::INK_DEEP_SRGB)
    }

    /// Superficie de painel / sidebar.
    pub fn panel() -> Color {
        c(LFTokens::PANEL_SRGB)
    }

    /// Superficie elevada (cards, hover).
    pub fn panel_hi() -> Color {
        c(LFTokens::PANEL_HI_SRGB)
    }

    /// Texto principal.
    pub fn fg() -> Color {
        c(LFTokens::PEARL_SRGB)
    }

    /// Texto de baixa enfase.
    pub fn muted() -> Color {
        c(LFTokens::MUTED_SRGB)
    }

    /// Accent primario (emerald-600).
    pub fn accent() -> Color {
        c(LFTokens::EMERALD_600_SRGB)
    }

    /// Accent hover (emerald-500).
    pub fn accent_hover() -> Color {
        c(LFTokens::EMERALD_500_SRGB)
    }

    /// Cor de perigo / destructive.
    pub fn danger() -> Color {
        c(LFTokens::DANGER_SRGB)
    }

    /// Separador / borda sutil.
    pub fn sep() -> Color {
        Color::from_rgba(0.2, 0.2, 0.25, 1.0)
    }

    /// Pill background (items selecionados).
    pub fn pill_bg() -> Color {
        Color::from_rgba(
            LFTokens::EMERALD_600_SRGB[0],
            LFTokens::EMERALD_600_SRGB[1],
            LFTokens::EMERALD_600_SRGB[2],
            0.2,
        )
    }
}
