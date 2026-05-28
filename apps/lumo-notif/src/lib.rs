//! lumo-notif lib target — logic puro testavel sem Wayland/DBus deps.
//!
//! Bin (main.rs) usa estes modulos + tudo do state/dbus/paint que precisam Wayland.

pub mod urgency;
pub mod toast_logic;
