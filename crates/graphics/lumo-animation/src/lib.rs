//! lumo-animation: framework animacoes Apple-grade pra Lumo OS.
//!
//! Modulos:
//!   spring      - Spring (LASpring): mola amortecida 1D, presets Apple
//!   easing      - LACurve: cubic-bezier deterministica + presets iOS
//!   interpolate - LAInterpolable: trait lerp + impls (f32, Color, Rect)
//!   animator    - LAAnimator<T>: driver duration-based ou spring generico
//!
//! Namespace LA* espelha Apple CoreAnimation (CA*) pra familiaridade.

pub mod animator;
pub mod easing;
pub mod interpolate;
pub mod spring;

// Re-exports de primeiro nivel pra uso sem qualificar sub-modulo.
pub use animator::{AnimCurve, LAAnimator};
pub use easing::LACurve;
pub use interpolate::{LAColor, LAInterpolable};
pub use spring::{LASpring, Spring};
