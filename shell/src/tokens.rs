//! Design tokens — Lumo Shell
//!
//! A13: adicionado bridge pra `lumo-foundation` theme system (light/dark
//! switchable via `LUMO_THEME` env). Constantes legacy `C_*` continuam
//! existindo pro lumo-shell Gallery demo (dark-only intentional) — sao a
//! materializacao da paleta `LumoColors::dark()` em ARGB packed pra
//! consumo direto por GPUI/`rgb()`/`rgba()`.
//!
//! `current_*` helpers expoem a paleta theme-aware pra qualquer caller
//! que queira respeitar `LUMO_THEME` (lumo-bar usa).
//!
//! Memory feedback_zero_neon_glow: nenhum token aqui carrega box-shadow
//! colorido; sombras vem via overlays no compositor.

use std::time::Duration;

// Re-export theme-aware API.
pub use lumo_foundation::{current_colors, current_theme, LumoColors, LumoTheme};

// ============================================================
// Gallery legacy palette (dark, fixed) — Apple-fluid demo show-off.
// Mantido por design: Gallery sempre dark intencional. Caller que quiser
// theme-aware usa `current_colors()`.
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
