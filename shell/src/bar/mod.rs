//! bar/ - top bar Lumo OS via wlr-layer-shell + SHM + tiny-skia.
//!
//! A18 redesign: pill-style flutuante estilo Samsung Galaxy / iOS Dynamic
//! Island. Bar fundo TRANSPARENTE; 2 pills arredondadas escuras
//! semi-translucent com sombra preta neutra (sem accent glow,
//! memory feedback_zero_neon_glow).
//!
//! Layout (40px altura total, pills 28px com 6px margem topo):
//!
//!   +------------------------------------------------------+
//!   |  [== . Lumo . 1 ==]                [== ~ 82% 16:42 ==]|
//!   +------------------------------------------------------+
//!
//! Memory feedback_lumo_arquitetura_clean: modulos por feature. Ordem de
//! leitura sugerida: tokens (constants) -> fonts/icons/pills (primitives)
//! -> dropdowns/* (paineis) -> state (struct + paint_frame) -> handlers
//! (Wayland delegates + redraw) -> input (PointerHandler) -> input_region
//! -> ipc -> main_loop (entry).

pub mod appmenu;
pub mod registrar;
pub mod dropdowns;
pub mod fonts;
pub mod handlers;
pub mod icons;
pub mod input;
pub mod input_region;
pub mod ipc;
pub mod main_loop;
pub mod pills;
pub mod state;
pub mod system_info;
pub mod tokens;

pub use main_loop::run;
