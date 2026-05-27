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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_is_idempotent() {
        // Chamadas multiplas nao panicam nem reinicializam global.
        init();
        let first = GLOBAL_STORE.get().map(|s| std::sync::Arc::as_ptr(s));
        init();
        let second = GLOBAL_STORE.get().map(|s| std::sync::Arc::as_ptr(s));
        assert_eq!(first, second, "GLOBAL_STORE deve ser mesmo Arc apos 2 inits");
    }

    #[test]
    fn init_makes_global_store_available() {
        init();
        assert!(GLOBAL_STORE.get().is_some());
    }
}
