//! desktop/ - layer-shell Background pra capturar pointer events na area
//! de trabalho (estilo macOS Finder / Windows desktop).
//!
//! A21: novo binario.
//! A27: menu Apple-style + items MVP wallpaper/sobre/atualizar/store.
//!
//! Memory feedback_zero_neon_glow: hover pill accent SOLIDO sem glow.
//! Memory feedback_lumo_arquitetura_clean: render compartilhado com
//! lumo-bar via `crate::menu`.

pub mod handlers;
pub mod input;
pub mod main_loop;
pub mod menu_overlay;
pub mod state;

pub use main_loop::run;
