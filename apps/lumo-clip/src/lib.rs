//! lumo-clip lib target — logic puro testavel sem Wayland deps.
//!
//! Apenas re-exporta history pra rodar testes em Windows (sctk + xkbcommon
//! nao buildam Windows; main.rs continua bin com Wayland).

pub mod history;
