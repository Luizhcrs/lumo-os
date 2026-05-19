//! # lumo-wm
//!
//! Proposito: Wayland compositor DRM/winit do Lumo OS. Backend selecionado via LUMO_WM_BACKEND.
//!
//! ## Invariantes
//! - Apenas 1 processo pode ser DRM master simultaneamente — ver I-01.
//! - LumoState nao implementa Send; event loop calloop e single-threaded — ver I-09.
//! - WAYLAND_DISPLAY e socket_name sao imutaveis apos EventLoop::run() — ver I-05.
//! - LumoWallpaper.buffer nao pode sobreviver ao GlesRenderer que o criou — ver I-04.
//!
//! ## Memory refs
//! - [[feedback-design-lapidado]]
//! - [[project-lumo-os]]

pub mod backend;
pub mod focus;
pub mod perf;
pub mod hardware;
pub mod cursor;
pub mod handlers;
pub mod input;
pub mod ipc;
pub mod workspace;
pub mod state;

pub use state::{init_socket, ClientState, LumoState};
pub mod window_anim;
pub mod multi_monitor;
pub mod tiling;
pub mod overview;
pub mod stack_picker;
