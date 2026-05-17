//! lumo-wm - Wayland compositor proprio do Lumo OS (Layer 5).
//!
//! Fase 5.2: client rendering + layer-shell + input dispatch + protocols
//! opcionais (primary_selection, xdg_activation, fractional_scale,
//! cursor_shape, xdg_toplevel_icon).
//!
//! Roda como cliente Wayland em cima do Hyprland (backend winit), expoe
//! seu proprio socket `wayland-N`, renderiza clientes via GlesRenderer
//! dentro da janela winit. Foot/weston-terminal aparecem nested.
//!
//! Fases proximas:
//! - 5.3 lumo-gfx-core integration (wgpu pipeline + custom cursor)
//! - 5.4 gestures completos + animation engine
//! - 5.5 feature parity Hyprland + TTY (udev/DRM)
//!
//! Veja [[06 - Layer 3 Compositor Research]] no vault pra justificativa
//! arquitetural da escolha do Smithay.

pub mod backend;
pub mod cursor;
pub mod handlers;
pub mod state;

pub use state::{init_socket, ClientState, LumoState};
