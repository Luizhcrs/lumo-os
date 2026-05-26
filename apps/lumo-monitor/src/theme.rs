//! theme.rs -- Bridge lumo-foundation tokens -> iced para lumo-monitor.

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
    pub fn warn() -> Color {
        Color::from_rgba(0.85, 0.47, 0.08, 1.0)
    }
    pub fn danger() -> Color {
        c(LFTokens::DANGER_SRGB)
    }
}

pub enum TabStyle {
    Active,
    Inactive,
}

impl TabStyle {
    pub fn style(&self) -> button::Style {
        match self {
            TabStyle::Active => button::Style {
                background: Some(Background::Color(Color::from_rgba(
                    LFTokens::EMERALD_600_SRGB[0],
                    LFTokens::EMERALD_600_SRGB[1],
                    LFTokens::EMERALD_600_SRGB[2],
                    0.25,
                ))),
                border: Border {
                    color: LumoTheme::accent(),
                    width: 1.0,
                    radius: 4.0.into(),
                },
                text_color: LumoTheme::accent(),
                ..Default::default()
            },
            TabStyle::Inactive => button::Style {
                background: Some(Background::Color(Color::TRANSPARENT)),
                border: Border {
                    radius: 4.0.into(),
                    ..Default::default()
                },
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

pub fn container_panel() -> container::Style {
    container::Style {
        background: Some(Background::Color(LumoTheme::panel())),
        border: Border {
            color: LumoTheme::sep(),
            width: 1.0,
            radius: 6.0.into(),
        },
        ..Default::default()
    }
}

/// Color for a percentage value (green < 60%, orange < 85%, red >= 85%).
pub fn pct_color(pct: f32) -> Color {
    if pct < 60.0 {
        LumoTheme::accent()
    } else if pct < 85.0 {
        LumoTheme::warn()
    } else {
        LumoTheme::danger()
    }
}

/// Simple bar as colored text progress.
pub fn bar_str(pct: f32, width: usize) -> String {
    let filled = ((pct / 100.0) * width as f32).round() as usize;
    let empty = width.saturating_sub(filled);
    format!("[{}{}]", "#".repeat(filled), ".".repeat(empty))
}
