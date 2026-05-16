//! lumo-wm - Wayland compositor proprio do Lumo OS (Layer 5).
//!
//! Fase 5.1: bootstrap Smithay nested. O binario `lumo-wm` roda como
//! cliente Wayland em cima do Hyprland atual (via backend winit), expoe
//! seu proprio socket `wayland-N` e aceita clientes de teste.
//!
//! Fases proximas:
//! - 5.2 layer-shell + tiling
//! - 5.3 animation engine integrada lumo-gfx
//! - 5.4 gestures completos
//! - 5.5 feature parity Hyprland + TTY (udev/DRM)
//!
//! Veja [[06 - Layer 3 Compositor Research]] no vault pra justificativa
//! arquitetural da escolha do Smithay.

pub mod backend;
pub mod handlers;
pub mod state;

pub use state::{init_socket, ClientState, LumoState};
