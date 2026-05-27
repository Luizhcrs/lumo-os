//! degraded.rs — tracker de features degradadas + emit IPC.
//!
//! Compositor mantem set de codigos atualmente degradados. Quando feature
//! entra/sai do modo degradado, broadcast LumoEvent::DegradedFeature[Cleared].
//! Bar mostra pill warning enquanto codigo ativo.
//!
//! Exemplos:
//! - WM-RENDER-002 = vsync off / page-flip falhando
//! - WM-COLOR-OFF = wp-color-manager-v1 nao registrado
//! - WM-ICON-OFF = xdg-toplevel-icon nao registrado
//!
//! API: DegradedTracker::set(code, label) e clear(code). Idempotente.

use std::collections::HashMap;

use lumo_ipc::LumoEvent;

#[derive(Default)]
pub struct DegradedTracker {
    /// code -> label atual. label pode ser atualizado mantendo mesmo codigo.
    active: HashMap<String, String>,
}

impl DegradedTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Marca feature como degradada. Retorna evento pra broadcast se transicao.
    /// Reentrada mesma label = no-op (None).
    pub fn set(&mut self, code: &str, label: &str) -> Option<LumoEvent> {
        let prev = self.active.insert(code.to_string(), label.to_string());
        if prev.as_deref() == Some(label) {
            return None;
        }
        Some(LumoEvent::DegradedFeature {
            code: code.to_string(),
            label: label.to_string(),
        })
    }

    /// Marca feature como recuperada. Retorna evento se estava ativa.
    pub fn clear(&mut self, code: &str) -> Option<LumoEvent> {
        if self.active.remove(code).is_some() {
            Some(LumoEvent::DegradedFeatureCleared {
                code: code.to_string(),
            })
        } else {
            None
        }
    }

    pub fn is_active(&self, code: &str) -> bool {
        self.active.contains_key(code)
    }

    pub fn snapshot(&self) -> Vec<(String, String)> {
        self.active
            .iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_returns_event_on_first_insert() {
        let mut t = DegradedTracker::new();
        let ev = t.set("WM-RENDER-002", "Vsync off").expect("first insert returns event");
        assert!(matches!(ev, LumoEvent::DegradedFeature { .. }));
        assert!(t.is_active("WM-RENDER-002"));
    }

    #[test]
    fn set_same_label_returns_none() {
        let mut t = DegradedTracker::new();
        t.set("X-1", "label").unwrap();
        assert!(t.set("X-1", "label").is_none(), "reinsert same label = no-op");
    }

    #[test]
    fn set_new_label_returns_event() {
        let mut t = DegradedTracker::new();
        t.set("X-1", "label1").unwrap();
        let ev = t.set("X-1", "label2").expect("label change re-emits");
        if let LumoEvent::DegradedFeature { label, .. } = ev {
            assert_eq!(label, "label2");
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn clear_returns_event_when_active() {
        let mut t = DegradedTracker::new();
        t.set("X-1", "label").unwrap();
        let ev = t.clear("X-1").expect("clear of active returns event");
        assert!(matches!(ev, LumoEvent::DegradedFeatureCleared { .. }));
        assert!(!t.is_active("X-1"));
    }

    #[test]
    fn clear_inactive_returns_none() {
        let mut t = DegradedTracker::new();
        assert!(t.clear("X-1").is_none());
    }

    #[test]
    fn snapshot_lists_all_active() {
        let mut t = DegradedTracker::new();
        t.set("A", "labelA").unwrap();
        t.set("B", "labelB").unwrap();
        let snap = t.snapshot();
        assert_eq!(snap.len(), 2);
    }

    #[test]
    fn snapshot_empty_when_nothing_active() {
        let t = DegradedTracker::new();
        assert!(t.snapshot().is_empty());
    }

    #[test]
    fn multiple_codes_independent() {
        let mut t = DegradedTracker::new();
        t.set("A", "x").unwrap();
        t.set("B", "y").unwrap();
        t.clear("A").unwrap();
        assert!(!t.is_active("A"));
        assert!(t.is_active("B"));
    }
}
