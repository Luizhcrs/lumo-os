//! Backends de output/input.
//! - winit: nested em outro compositor (default, sempre disponivel)
//! - drm: TTY direto via DRM/KMS (gated drm-backend feature)
//! - libinput: input direto (so gated drm-backend, usa session libseat)
//! - render_common: helpers de cursor/corner/shadow compartilhados

pub mod render_common;
pub mod winit;

#[cfg(feature = "drm-backend")]
pub mod drm;

#[cfg(feature = "drm-backend")]
pub mod libinput;
