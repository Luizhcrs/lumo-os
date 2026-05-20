// Shim crate: re-exports uzers public API as `users`.
// Replaces vulnerable users 0.10.0 (RUSTSEC-2023-0059 / RUSTSEC-2025-0040).
// API-compatible: uzers is the maintained fork of users.
pub use uzers::*;
