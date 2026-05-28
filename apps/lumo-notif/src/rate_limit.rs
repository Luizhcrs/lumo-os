//! rate_limit.rs — re-export de lumo_foundation::util.
//!
//! Movido pra foundation pra ser reusavel por outros crates (lumo-bridge,
//! lumo-center, etc).

pub use lumo_foundation::util::{rate_limit_check, safe_lock};
