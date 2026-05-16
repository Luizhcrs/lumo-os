//! Design tokens — Luiz Shell
//! Single source of truth pra cores, durations, sizing
//! Replica de Shell Proprio/03 - Design Tokens.md no Obsidian

use std::time::Duration;

// ============================================================
// Colors (dark default — sem alpha)
// ============================================================
pub const C_BG: u32         = 0x0a0a0c;
pub const C_PANEL: u32      = 0x131318;
pub const C_PANEL_HI: u32   = 0x1a1a21;
pub const C_TEXT: u32       = 0xf5f5f7;
pub const C_MUTED: u32      = 0x9596a0;
pub const C_ACCENT: u32     = 0x059669;  // emerald-600
pub const C_ACCENT_PRESS: u32 = 0x065f46;
pub const C_ON_ACCENT: u32  = 0x0a0a0c;
pub const C_DANGER: u32     = 0xf87171;

// ============================================================
// Colors with alpha (use rgba())
// ============================================================
pub const C_BORDER: u32      = 0xffffff14; // white .08
pub const C_BORDER_SOFT: u32 = 0xffffff0a; // white .04
pub const C_BACKDROP: u32    = 0x000000a6; // black .65
pub const C_BACKDROP_SOFT: u32 = 0x00000080; // black .50

// ============================================================
// Durations (Apple HIG)
// ============================================================
pub const DUR_QUICK: Duration  = Duration::from_millis(180);
pub const DUR_BASE: Duration   = Duration::from_millis(280);
pub const DUR_MODAL: Duration  = Duration::from_millis(350);
pub const DUR_BOUNCE: Duration = Duration::from_millis(420);
pub const DUR_PRESS: Duration  = Duration::from_millis(150);

// ============================================================
// Sizing
// ============================================================
pub const SEG_WIDTH: f32 = 80.0;
