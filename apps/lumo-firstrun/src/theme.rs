//! theme.rs -- estilos Iced 0.13 para lumo-firstrun usando tokens lumo-foundation.

use iced::widget::container;
use iced::{Background, Border, Color};
use lumo_foundation::LFTokens;

fn c(srgb: [f32; 4]) -> Color {
    Color::from_rgba(srgb[0], srgb[1], srgb[2], srgb[3])
}

pub struct LumoFirstrunTheme;

impl LumoFirstrunTheme {
    pub fn bg()     -> Color { c(LFTokens::INK_DEEP_SRGB) }
    pub fn panel()  -> Color { c(LFTokens::PANEL_HI_SRGB) }
    pub fn accent() -> Color { c(LFTokens::EMERALD_600_SRGB) }
}

pub fn bg_style() -> container::Style {
    container::Style {
        background: Some(Background::Color(LumoFirstrunTheme::bg())),
        ..Default::default()
    }
}

pub fn card_style() -> container::Style {
    container::Style {
        background: Some(Background::Color(LumoFirstrunTheme::panel())),
        border: Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.05),
            width: 1.0,
            radius: 12.0.into(),
        },
        ..Default::default()
    }
}

pub fn progress_fill_style() -> container::Style {
    container::Style {
        background: Some(Background::Color(LumoFirstrunTheme::accent())),
        border: Border {
            radius: 4.0.into(),
            ..Default::default()
        },
        ..Default::default()
    }
}
