//! lumo-telemetry: full-stack telemetry for Lumo OS.
//!
//! API:
//!   init()                                           -- spawns exporter (idempotent)
//!   record_event(kind, meta)                         -- push event to ring buffer
//!   histogram(name, value_us)                        -- record latency sample
//!   time(name, closure) -> R                         -- closure timing helper

mod event;
mod exporter;
mod histogram;
mod init;
mod store;

pub use event::{Event, EventKind};
pub use histogram::SnapshotJson;
pub use init::init;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use store::TelemetryStore;

static GLOBAL_STORE: OnceLock<Arc<Mutex<TelemetryStore>>> = OnceLock::new();

fn store() -> Option<Arc<Mutex<TelemetryStore>>> {
    GLOBAL_STORE.get().cloned()
}

/// Push a new event into the ring buffer.
/// No-op if init() was not called.
pub fn record_event(kind: EventKind, meta: HashMap<String, String>) {
    if let Some(s) = store() {
        if let Ok(mut guard) = s.lock() {
            guard.push_event(Event::new(kind, meta));
        }
    }
}

/// Record a latency sample (microseconds) in a named histogram.
/// No-op if init() was not called.
pub fn histogram(name: &str, value_us: u64) {
    if let Some(s) = store() {
        if let Ok(mut guard) = s.lock() {
            guard.record_histogram(name, value_us);
        }
    }
}

/// Time a closure and record the duration in a named histogram.
/// Returns the closure result regardless of init() status.
pub fn time<F, R>(name: &str, f: F) -> R
where
    F: FnOnce() -> R,
{
    let t0 = std::time::Instant::now();
    let result = f();
    let elapsed_us = t0.elapsed().as_micros() as u64;
    histogram(name, elapsed_us);
    result
}
