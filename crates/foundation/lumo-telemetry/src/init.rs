//! Idempotent init: spawns exporter thread once.

use crate::exporter::spawn_exporter;
use crate::store::TelemetryStore;
use crate::GLOBAL_STORE;

pub fn init() {
    // OnceLock guarantees single init even with concurrent callers.
    GLOBAL_STORE.get_or_init(|| {
        let store = std::sync::Arc::new(std::sync::Mutex::new(TelemetryStore::new()));
        spawn_exporter(std::sync::Arc::clone(&store));
        store
    });
}
