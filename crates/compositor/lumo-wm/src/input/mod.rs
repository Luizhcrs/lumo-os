//! Modulo de input do lumo-wm.
//!
//! Sub-modulos:
//! - keyboard: keybindings globais + config TOML
//! - touchpad: gestos + config Apple-like

pub mod keyboard;
pub mod touchpad;

pub use touchpad::{TouchpadConfig, TouchpadGestureState};
