//! Backends de output/input.
//! - winit: nested em outro compositor (default, sempre disponivel)
//! - drm: TTY direto via DRM/KMS (gated `drm-backend` feature)

pub mod winit;

#[cfg(feature = "drm-backend")]
pub mod drm;
