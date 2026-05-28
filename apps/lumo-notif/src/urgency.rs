//! urgency.rs — re-export de lumo_foundation::urgency.
//!
//! A3 review: Urgency vive em lumo-foundation pra compartilhar com OSD/center
//! sem cruzar bin boundary. Esse modulo persiste pra evitar churn de imports
//! ao longo do bin lumo-notif.

pub use lumo_foundation::urgency::{
    Urgency, CRITICAL_TIMEOUT_MS, LOW_TIMEOUT_MS, NORMAL_TIMEOUT_MS,
};
