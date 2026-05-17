//! lumo-gfx-core (umbrella)
//!
//! Re-exporta todos os crates Lumo framework-style. Existe pra:
//! 1) manter 1 ponto unico para os bins demo (`triangle`, `quad-gallery`,
//!    `quad-shadow`, `text-demo`, `button-demo`, `button-interactive`).
//! 2) dar uma API de "monorepo gfx" pra callers que nao querem listar 7
//!    deps no Cargo.toml deles.
//!
//! Novos call sites devem importar dos crates especificos
//! (`lumo_beam`, `lumo_kit`, etc.) para Cargo entender melhor o grafo
//! de dependencias.

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
