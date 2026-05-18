//! lumo-shell library crate.
//!
//! Expoe modulos `bar` e `desktop` consumidos pelos binarios em `src/bin/`.
//! Refactor A-REFACTOR: dividir monolitos em modulos por feature pra
//! permitir paralelismo de agentes sem conflitos de merge.
//! Memory feedback_lumo_arquitetura_clean: modulos por feature, NAO por camada.

pub mod bar;
pub mod desktop;
pub mod menu;
