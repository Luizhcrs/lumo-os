//! lumo-wm - Wayland compositor proprio do Lumo OS (Layer 5).
//!
//! Fase 5.4 (A7): cursor xcursor real + spawn keybind + bar SHM.
//!
//! Fase 5.5 (A8):
//! - backend DRM/KMS (`backend::drm`) gated por feature `drm-backend`,
//!   selecionado via env `LUMO_WM_BACKEND=drm` (default winit).
//! - IPC unix socket (`crate::ipc`) pra workspaces -> lumo-bar.
//! - Moldura desktop: corner radius + sombra preta neutra (sem neon).
//!
//! Roda nested em Hyprland (winit) ou full-session em TTY (drm).

pub mod backend;
pub mod focus;
pub mod hardware;
pub mod cursor;
pub mod handlers;
pub mod input;
pub mod ipc;
pub mod state;

pub use state::{init_socket, ClientState, LumoState};
