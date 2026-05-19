//! Handlers Wayland - cada modulo implementa um conjunto de traits
//! Smithay (Handler + Delegate) e usa as macros `delegate_*!` pra
//! reduzir boilerplate de dispatch.

pub mod compositor;
pub mod data_device;
pub mod input;
pub mod layer_shell;
pub mod misc;
pub mod output;
pub mod seat;
pub mod shm;
pub mod xdg_shell;
pub mod xdg_decoration;
pub mod dmabuf;
pub mod lid;
pub mod screencopy;
pub mod idle;
pub mod color_management;
pub mod wayland_modern;
