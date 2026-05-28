//! lumo-osd-framework — pacote compartilhado pra todos OSDs Lumo OS.
//!
//! OSD = On-Screen Display. Overlay layer-shell que aparece centro-top
//! da tela quando user dispara acao sistema:
//! - Caps/Num/Scroll Lock toggle
//! - Brightness adjust
//! - Volume adjust + Mute toggle
//! - Mic mute
//! - Keyboard layout switch
//! - Display profile change
//!
//! Padroes Lumo OS (F1.5):
//! - Tamanho: 300x80 px (centro top)
//! - Posicao Y: 80px top (abaixo da bar)
//! - Radius: 16px
//! - Background: 0x2a2a2a alpha 0xF0 (semi-translucent dark)
//! - Foreground: 0xE0E0E0
//! - Fade in: 150ms
//! - Hold: 1800ms (ajustavel por OSD)
//! - Fade out: 200ms
//! - Total: ~2150ms padrao
//!
//! Componentes:
//! - `OsdLayout`: geometry calc (rect, padding, slot pra icon+texto+slider)
//! - `OsdAnimator`: fade in/out state machine
//! - `slider`: render slider 0-100% com fill bar
//! - `toggle`: render dot ON/OFF
//! - `paint_bg`: rrect background uniforme
//!
//! Cada bin OSD (`lumo-osd-locks`, `lumo-osd-brightness` etc) usa o
//! framework + adiciona logica especifica (le sysfs, dispatch event).

pub mod animator;
pub mod layout;
pub mod paint;
pub mod snapshot;

pub use animator::{OsdAnimator, OsdPhase};
pub use layout::{OsdLayout, SliderGeom, ToggleGeom};

/// Constants Lumo OS OSD design tokens.
pub mod tokens {
    /// Default OSD width em px (logical).
    pub const OSD_WIDTH: u32 = 300;
    /// Default OSD height em px (logical).
    pub const OSD_HEIGHT: u32 = 80;
    /// Margem do topo da tela (abaixo da bar Lumo 28px + gap).
    pub const OSD_MARGIN_TOP: i32 = 80;
    /// Radius pill background.
    pub const OSD_RADIUS: f32 = 16.0;
    /// Padding interno horizontal.
    pub const OSD_PAD_X: f32 = 20.0;
    /// Padding interno vertical.
    pub const OSD_PAD_Y: f32 = 16.0;
    /// Gap entre icon e content.
    pub const OSD_GAP_ICON: f32 = 12.0;
    /// Slider height em px.
    pub const SLIDER_H: f32 = 8.0;
    /// Slider fill radius (pill style).
    pub const SLIDER_RADIUS: f32 = 4.0;
    /// Fade in duration (ms).
    pub const FADE_IN_MS: u32 = 150;
    /// Hold duration default (ms).
    pub const HOLD_MS_DEFAULT: u32 = 1800;
    /// Fade out duration (ms).
    pub const FADE_OUT_MS: u32 = 200;
}
