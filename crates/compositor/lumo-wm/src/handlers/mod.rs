//! Handlers Wayland - cada modulo implementa um conjunto de traits
//! Smithay (Handler + Delegate) e usa as macros `delegate_*!` pra
//! reduzir boilerplate de dispatch.

pub mod color_management;
pub mod compositor;
pub mod data_device;
pub mod dmabuf;
pub mod idle;
pub mod input;
pub mod layer_shell;
pub mod lid;
pub mod misc;
pub mod output;
pub mod screencopy;
pub mod seat;
pub mod shm;
pub mod wayland_modern;
pub mod xdg_decoration;
pub mod xdg_shell;
