//! sanitize.rs — re-export de lumo_foundation::util.
//!
//! Funcs movidas pra foundation pra dedup (clamp/markup_escape sao usados
//! tambem por lumo-osd, lumo-clip, lumo-center futuros).

pub use lumo_foundation::util::{clamp, markup_escape};
