//! theme.rs -- Bridge lumo-foundation tokens -> iced para lumo-calc.

use iced::widget::button;
use iced::widget::container;
use iced::{Background, Border, Color};
use lumo_foundation::LFTokens;

fn c(srgb: [f32; 4]) -> Color {
    Color::from_rgba(srgb[0], srgb[1], srgb[2], srgb[3])
}

pub struct LumoTheme;

impl LumoTheme {
    pub fn bg() -> Color      { c(LFTokens::INK_DEEP_SRGB) }
    pub fn panel() -> Color   { c(LFTokens::PANEL_SRGB) }
    pub fn panel_hi() -> Color { c(LFTokens::PANEL_HI_SRGB) }
    pub fn fg() -> Color      { c(LFTokens::PEARL_SRGB) }
    pub fn muted() -> Color   { c(LFTokens::MUTED_SRGB) }
    pub fn accent() -> Color  { c(LFTokens::EMERALD_600_SRGB) }
    pub fn danger() -> Color  { c(LFTokens::DANGER_SRGB) }
    pub fn sep() -> Color     { Color::from_rgba(0.2, 0.2, 0.25, 1.0) }
}

pub enum ButtonKind {
    Digit,
    Op,
    Equals,
    Clear,
    Special,
}

impl ButtonKind {
    pub fn style(&self) -> button::Style {
        match self {
            ButtonKind::Digit => button::Style {
                background: Some(Background::Color(LumoTheme::panel_hi())),
                border: Border { color: LumoTheme::sep(), width: 1.0, radius: 8.0.into() },
                text_color: LumoTheme::fg(),
                ..Default::default()
            },
            ButtonKind::Op => button::Style {
                background: Some(Background::Color(Color::from_rgba(
                    LFTokens::EMERALD_600_SRGB[0],
                    LFTokens::EMERALD_600_SRGB[1],
                    LFTokens::EMERALD_600_SRGB[2],
                    0.2,
                ))),
                border: Border { color: LumoTheme::accent(), width: 1.0, radius: 8.0.into() },
                text_color: LumoTheme::accent(),
                ..Default::default()
            },
            ButtonKind::Equals => button::Style {
                background: Some(Background::Color(LumoTheme::accent())),
                border: Border { radius: 8.0.into(), ..Default::default() },
                text_color: LumoTheme::bg(),
                ..Default::default()
            },
            ButtonKind::Clear => button::Style {
                background: Some(Background::Color(Color::from_rgba(
                    LFTokens::DANGER_SRGB[0],
                    LFTokens::DANGER_SRGB[1],
                    LFTokens::DANGER_SRGB[2],
                    0.2,
                ))),
                border: Border { color: LumoTheme::danger(), width: 1.0, radius: 8.0.into() },
                text_color: LumoTheme::danger(),
                ..Default::default()
            },
            ButtonKind::Special => button::Style {
                background: Some(Background::Color(LumoTheme::panel())),
                border: Border { color: LumoTheme::sep(), width: 1.0, radius: 8.0.into() },
                text_color: LumoTheme::muted(),
                ..Default::default()
            },
        }
    }
}

pub fn container_bg() -> container::Style {
    container::Style {
        background: Some(Background::Color(LumoTheme::bg())),
        ..Default::default()
    }
}

pub fn container_display() -> container::Style {
    container::Style {
        background: Some(Background::Color(LumoTheme::panel_hi())),
        border: Border { color: LumoTheme::sep(), width: 1.0, radius: 8.0.into() },
        ..Default::default()
    }
}

pub fn container_history() -> container::Style {
    container::Style {
        background: Some(Background::Color(LumoTheme::panel())),
        border: Border { color: LumoTheme::sep(), width: 1.0, radius: 4.0.into() },
        ..Default::default()
    }
}
