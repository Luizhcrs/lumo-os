//! lumo-launcher-core — logic pura compartilhada pelo Spotlight Lumo.
//!
//! Modulos:
//! - convert: unit conversion (50 miles to km, etc)
//! - settings_index: lookup paineis settings via keyword
//! - files_search: walk recursive home filtrado por query
//!
//! Bin apps/lumo-launcher importa + chama. Lib testavel sem Wayland deps
//! (sctk + xkbcommon nao precisam em Windows pra rodar tests).

pub mod convert;
pub mod files_search;
pub mod settings_index;
