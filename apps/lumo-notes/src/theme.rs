//! theme.rs -- Bridge lumo-foundation tokens -> iced para lumo-notes.

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
    pub fn sep() -> Color     { Color::from_rgba(0.2, 0.2, 0.25, 1.0) }

    pub fn accent_alpha20() -> Color {
        Color::from_rgba(LFTokens::EMERALD_600_SRGB[0], LFTokens::EMERALD_600_SRGB[1], LFTokens::EMERALD_600_SRGB[2], 0.2)
    }
}

pub enum ButtonStyle {
    Primary,
    Secondary,
    Ghost,
    NoteItem { active: bool },
}

impl ButtonStyle {
    pub fn style(&self) -> button::Style {
        match self {
            ButtonStyle::Primary => button::Style {
                background: Some(Background::Color(LumoTheme::accent())),
                border: Border { radius: 6.0.into(), ..Default::default() },
                text_color: LumoTheme::bg(),
                ..Default::default()
            },
            ButtonStyle::Secondary => button::Style {
                background: Some(Background::Color(LumoTheme::panel_hi())),
                border: Border { color: LumoTheme::sep(), width: 1.0, radius: 6.0.into() },
                text_color: LumoTheme::fg(),
                ..Default::default()
            },
            ButtonStyle::Ghost => button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                border: Border { radius: 4.0.into(), ..Default::default() },
                text_color: LumoTheme::fg(),
                ..Default::default()
            },
            ButtonStyle::NoteItem { active } => button::Style {
                background: Some(Background::Color(if *active { LumoTheme::accent_alpha20() } else { Color::TRANSPARENT })),
                border: Border { color: if *active { LumoTheme::accent() } else { Color::TRANSPARENT }, width: 1.0, radius: 6.0.into() },
                text_color: LumoTheme::fg(),
                ..Default::default()
            },
        }
    }
}

pub enum ContainerStyle { Sidebar, Main, Card }

impl ContainerStyle {
    pub fn style(&self) -> container::Style {
        match self {
            ContainerStyle::Sidebar => container::Style {
                background: Some(Background::Color(LumoTheme::panel())),
                border: Border { color: LumoTheme::sep(), width: 0.0, ..Default::default() },
                ..Default::default()
            },
            ContainerStyle::Main => container::Style {
                background: Some(Background::Color(LumoTheme::bg())),
                ..Default::default()
            },
            ContainerStyle::Card => container::Style {
                background: Some(Background::Color(LumoTheme::panel_hi())),
                border: Border { color: LumoTheme::sep(), width: 1.0, radius: 8.0.into() },
                ..Default::default()
            },
        }
    }
}
