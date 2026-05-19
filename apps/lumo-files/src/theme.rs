//! Bridge lumo-foundation tokens -> iced::Theme customizado.
//!
//! Converte os tokens sRGB de LFTokens para iced::Color e monta
//! iced::Theme::Custom com a paleta Lumo.

use iced::widget::button;
use iced::widget::container;
use iced::{Background, Border, Color};
use lumo_foundation::LFTokens;

/// Converte [f32;4] sRGB normalizado para iced::Color.
fn c(srgb: [f32; 4]) -> Color {
    Color::from_rgba(srgb[0], srgb[1], srgb[2], srgb[3])
}

/// Paleta Lumo como consts iced::Color para uso em widgets.
pub struct LumoTheme;

impl LumoTheme {
    pub fn bg() -> Color { c(LFTokens::INK_DEEP_SRGB) }
    pub fn panel() -> Color { c(LFTokens::PANEL_SRGB) }
    pub fn panel_hi() -> Color { c(LFTokens::PANEL_HI_SRGB) }
    pub fn fg() -> Color { c(LFTokens::PEARL_SRGB) }
    pub fn muted() -> Color { c(LFTokens::MUTED_SRGB) }
    pub fn accent() -> Color { c(LFTokens::EMERALD_600_SRGB) }
    pub fn accent_hover() -> Color { c(LFTokens::EMERALD_500_SRGB) }
    pub fn danger() -> Color { c(LFTokens::DANGER_SRGB) }

    pub fn sep() -> Color {
        Color::from_rgba(0.2, 0.2, 0.25, 1.0)
    }

    pub fn pill_bg() -> Color {
        Color::from_rgba(
            LFTokens::EMERALD_600_SRGB[0],
            LFTokens::EMERALD_600_SRGB[1],
            LFTokens::EMERALD_600_SRGB[2],
            0.2,
        )
    }

    /// Fundo accent com alpha 0x20 (hover sidebar).
    pub fn accent_alpha20() -> Color {
        Color::from_rgba(
            LFTokens::EMERALD_600_SRGB[0],
            LFTokens::EMERALD_600_SRGB[1],
            LFTokens::EMERALD_600_SRGB[2],
            0x20 as f32 / 255.0,
        )
    }

    /// Fundo accent com alpha 0x40 (selected sidebar).
    pub fn accent_alpha40() -> Color {
        Color::from_rgba(
            LFTokens::EMERALD_600_SRGB[0],
            LFTokens::EMERALD_600_SRGB[1],
            LFTokens::EMERALD_600_SRGB[2],
            0x40 as f32 / 255.0,
        )
    }

    /// Fundo accent com alpha 0x30 (selected grid cell).
    pub fn accent_alpha30() -> Color {
        Color::from_rgba(
            LFTokens::EMERALD_600_SRGB[0],
            LFTokens::EMERALD_600_SRGB[1],
            LFTokens::EMERALD_600_SRGB[2],
            0x30 as f32 / 255.0,
        )
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
                border: Border { radius: 4.0.into(), ..Default::default() },
                text_color: LumoTheme::bg(),
                ..Default::default()
            },
            ButtonStyle::Secondary => button::Style {
                background: Some(Background::Color(LumoTheme::panel_hi())),
                border: Border {
                    color: LumoTheme::sep(),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                text_color: LumoTheme::fg(),
                ..Default::default()
            },
            ButtonStyle::Ghost => button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                border: Border { radius: 4.0.into(), ..Default::default() },
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
