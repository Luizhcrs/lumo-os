//! # lumo-gfx-core
//!
//! Proposito: Umbrella crate: re-exporta todos os crates grafico Lumo para uso pelos bins demo.
//!
//! ## Invariantes
//! - Novos call sites devem importar dos crates especificos pra Cargo entender melhor o grafo.
//! - Re-exports com glob (*): conflitos de nome sao erro de compilacao, nao silenciosos.
//!
//! ## Memory refs
//! - [[feedback-design-lapidado]]
//! - [[project-lumo-os]]

// Flat top-level re-exports (preserva caminhos curtos `lumo_gfx_core::QuadInstance`).
pub use lumo_animation::*;
pub use lumo_beam::*;
pub use lumo_foundation::*;
pub use lumo_graphics::*;
pub use lumo_input::*;
pub use lumo_kit::*;
pub use lumo_text::*;

// Sub-modulos retro-compat (preserva caminhos `lumo_gfx_core::text::TextRenderer`
// usados nos bins demo legados).
pub mod text {
    pub use lumo_text::*;
}

pub mod widget {
    pub use lumo_kit::*;
}

pub mod input {
    pub use lumo_input::*;
}

pub mod anim {
    pub use lumo_animation::*;
}
