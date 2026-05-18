//! Modulo de input do lumo-wm.
//!
//! Sub-modulos:
//! - keyboard: keybindings globais + config TOML
//! - touchpad: gestos + config Lumo-like
//! - move_grab: pointer grab para arrastar toplevel via CSD header

pub mod keyboard;
pub mod touchpad;
pub mod move_grab;

pub use touchpad::{TouchpadConfig, TouchpadGestureState};
