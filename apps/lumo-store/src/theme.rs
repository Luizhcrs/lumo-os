//! theme.rs -- estilos Iced 0.13 para lumo-store usando tokens lumo-foundation.

use iced::widget::container;
use iced::{Background, Border, Color};
use lumo_foundation::LFTokens;

fn c(srgb: [f32; 4]) -> Color {
    Color::from_rgba(srgb[0], srgb[1], srgb[2], srgb[3])
}

pub fn bg_style() -> container::Style {
    container::Style {
        background: Some(Background::Color(c(LFTokens::INK_DEEP_SRGB))),
        ..Default::default()
    }
}

pub fn card_style() -> container::Style {
    container::Style {
        background: Some(Background::Color(c(LFTokens::PANEL_HI_SRGB))),
        border: Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.06),
            width: 1.0,
            radius: 10.0.into(),
        },
        ..Default::default()
    }
}

pub fn text_color() -> Color {
    c(LFTokens::PEARL_SRGB)
}
pub fn muted_color() -> Color {
    c(LFTokens::MUTED_SRGB)
}
pub fn accent_color() -> Color {
    c(LFTokens::EMERALD_600_SRGB)
}
