//! Backends de output/input.
//! - winit: nested em outro compositor (default, sempre disponivel)
//! - drm: TTY direto via DRM/KMS (gated drm-backend feature)
//! - libinput: input direto (so gated drm-backend, usa session libseat)
//! - render_common: helpers de cursor/corner/shadow compartilhados
//! - wallpaper: textura de fundo (A19), carregada uma vez por backend
//! - damage: heuristica de merge de rects (W3.P4)

pub mod corner_shader;
pub mod damage;
pub mod render_common;
pub mod wallpaper;
pub mod winit;

#[cfg(feature = "drm-backend")]
pub mod drm;

#[cfg(feature = "drm-backend")]
pub mod libinput;

#[cfg(feature = "drm-backend")]
pub mod vrr;

#[cfg(feature = "drm-backend")]
pub mod screencopy_cache;
