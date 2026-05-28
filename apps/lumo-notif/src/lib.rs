//! lumo-notif lib target — logic puro testavel sem Wayland/DBus deps.
//!
//! Bin (main.rs) usa estes modulos + tudo do state/dbus/paint que precisam Wayland.

pub mod rate_limit;
pub mod sanitize;
pub mod toast_logic;
pub mod urgency;
