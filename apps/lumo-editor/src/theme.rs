//! Bridge lumo-foundation tokens -> iced Theme para lumo-text.

use iced::widget::button;
use iced::widget::container;
use iced::{Background, Border, Color};
use lumo_foundation::LFTokens;

fn c(srgb: [f32; 4]) -> Color {
    Color::from_rgba(srgb[0], srgb[1], srgb[2], srgb[3])
}

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
    pub fn sep() -> Color {
        Color::from_rgba(0.2, 0.2, 0.25, 1.0)
    }
}

pub enum ButtonStyle {
    Primary,
    Toolbar,
}

impl ButtonStyle {
    pub fn style(&self) -> button::Style {
        match self {
            ButtonStyle::Primary => button::Style {
                background: Some(Background::Color(LumoTheme::accent())),
                border: Border {
                    radius: 5.0.into(),
                    ..Default::default()
                },
                text_color: LumoTheme::bg(),
                ..Default::default()
            },
            ButtonStyle::Toolbar => button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
                text_color: LumoTheme::fg(),
                ..Default::default()
            },
        }
    }
}

pub enum ContainerStyle {
    Bg,
    Toolbar,
    Editor,
    FindBar,
}

impl ContainerStyle {
    pub fn style(&self) -> container::Style {
        match self {
            ContainerStyle::Bg => container::Style {
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
            ContainerStyle::Editor => container::Style {
                background: Some(Background::Color(LumoTheme::panel_hi())),
                border: Border {
                    color: LumoTheme::sep(),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            },
            ContainerStyle::FindBar => container::Style {
                background: Some(Background::Color(LumoTheme::panel())),
                border: Border {
                    color: LumoTheme::sep(),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                ..Default::default()
            },
        }
    }
}
