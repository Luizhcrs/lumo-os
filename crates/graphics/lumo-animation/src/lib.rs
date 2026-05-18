//! # lumo-animation
//!
//! Proposito: Framework de animacoes: spring amortecido, cubic-bezier, LAAnimator generico.
//!
//! ## Invariantes
//! - LASpring e deterministica dado mesmo delta-t; nao depende de clock externo.
//! - LAAnimator::tick() deve ser chamado a cada frame com delta real (nao fixo) pra convergencia correta.
//!
//! ## Memory refs
//! - [[feedback-design-lapidado]]
//! - [[project-lumo-os]]

pub mod animator;
pub mod easing;
pub mod interpolate;
pub mod spring;

// Re-exports de primeiro nivel pra uso sem qualificar sub-modulo.
pub use animator::{AnimCurve, LAAnimator};
pub use easing::LACurve;
pub use interpolate::{LAColor, LAInterpolable};
pub use spring::{LASpring, Spring};

pub mod closed_form;
pub use closed_form::{ClosedFormSpring, DampingRegime, SpringPreset};
