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

/// Increment errors_total{code, severity}. No-op se init() nao foi chamado.
pub fn record_error(code: &str, severity: &str) {
    if let Some(s) = store() {
        if let Ok(mut guard) = s.lock() {
            guard.record_error(code, severity);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_event_noop_when_not_initialized() {
        // Sem init(), nenhum panic. Resultado: no-op silencioso.
        record_event(EventKind::Click, HashMap::new());
    }

    #[test]
    fn histogram_noop_when_not_initialized() {
        histogram("test_metric", 100);
    }

    #[test]
    fn time_returns_closure_result_even_without_init() {
        let r = time("test_op", || 42);
        assert_eq!(r, 42);
    }

    #[test]
    fn time_returns_string_result() {
        let r = time("test_op", || String::from("ok"));
        assert_eq!(r, "ok");
    }

    #[test]
    fn time_propagates_panic() {
        // Closure que panica deve panicar fora de time(); histograma nao registra.
        let result = std::panic::catch_unwind(|| {
            time::<_, ()>("test_panic", || {
                panic!("expected");
            })
        });
        assert!(result.is_err());
    }
}
