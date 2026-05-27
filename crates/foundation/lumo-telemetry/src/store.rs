//! In-memory ring buffer + histogram map. Central mutable state.

use std::collections::HashMap;
use std::collections::VecDeque;

use serde::Serialize;

use crate::event::Event;
use crate::histogram::{LumoHistogram, SnapshotJson};

const RING_CAPACITY: usize = 1000;

#[derive(Debug, Serialize)]
pub struct Snapshot {
    pub ts: String,
    pub events_per_kind: HashMap<String, usize>,
    pub histograms: HashMap<String, SnapshotJson>,
}

pub struct TelemetryStore {
    ring: VecDeque<Event>,
    histograms: HashMap<String, LumoHistogram>,
}

impl TelemetryStore {
    pub fn new() -> Self {
        Self {
            ring: VecDeque::with_capacity(RING_CAPACITY),
            histograms: HashMap::new(),
        }
    }

    pub fn push_event(&mut self, event: Event) {
        if self.ring.len() >= RING_CAPACITY {
            self.ring.pop_front();
        }
        self.ring.push_back(event);
    }

    pub fn record_histogram(&mut self, name: &str, value_us: u64) {
        self.histograms
            .entry(name.to_string())
            .or_default()
            .record(value_us);
    }

    pub fn build_snapshot(&mut self) -> Snapshot {
        let ts = chrono::Local::now().to_rfc3339();

        let mut events_per_kind: HashMap<String, usize> = HashMap::new();
        for ev in &self.ring {
            *events_per_kind.entry(ev.kind.to_string()).or_insert(0) += 1;
        }

        let histograms: HashMap<String, SnapshotJson> = self
            .histograms
            .iter()
            .map(|(k, v)| (k.clone(), v.snapshot()))
            .collect();

        Snapshot {
            ts,
            events_per_kind,
            histograms,
        }
    }
}

impl Default for TelemetryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventKind;

    fn make_event(kind: EventKind) -> Event {
        Event::new(kind, HashMap::new())
    }

    #[test]
    fn push_event_grows_ring() {
        let mut s = TelemetryStore::new();
        s.push_event(make_event(EventKind::Click));
        s.push_event(make_event(EventKind::KeyPress));
        assert_eq!(s.ring.len(), 2);
    }

    #[test]
    fn ring_buffer_evicts_oldest_at_capacity() {
        let mut s = TelemetryStore::new();
        for _ in 0..(RING_CAPACITY + 50) {
            s.push_event(make_event(EventKind::Click));
        }
        assert_eq!(s.ring.len(), RING_CAPACITY);
    }

    #[test]
    fn record_histogram_creates_entry_lazily() {
        let mut s = TelemetryStore::new();
        assert!(!s.histograms.contains_key("startup_us"));
        s.record_histogram("startup_us", 1500);
        assert!(s.histograms.contains_key("startup_us"));
    }

    #[test]
    fn record_histogram_accumulates_per_key() {
        let mut s = TelemetryStore::new();
        s.record_histogram("frame_us", 100);
        s.record_histogram("frame_us", 200);
        s.record_histogram("ipc_us", 50);
        let snap = s.build_snapshot();
        assert_eq!(snap.histograms.get("frame_us").map(|h| h.count), Some(2));
        assert_eq!(snap.histograms.get("ipc_us").map(|h| h.count), Some(1));
    }

    #[test]
    fn snapshot_counts_events_per_kind() {
        let mut s = TelemetryStore::new();
        s.push_event(make_event(EventKind::Click));
        s.push_event(make_event(EventKind::Click));
        s.push_event(make_event(EventKind::KeyPress));
        let snap = s.build_snapshot();
        assert_eq!(snap.events_per_kind.get("click"), Some(&2));
        assert_eq!(snap.events_per_kind.get("key_press"), Some(&1));
    }

    #[test]
    fn snapshot_has_iso8601_timestamp() {
        let mut s = TelemetryStore::new();
        let snap = s.build_snapshot();
        // rfc3339 e superset de iso8601 e contem T entre data e hora.
        assert!(snap.ts.contains('T'));
        assert!(snap.ts.len() > 10);
    }

    #[test]
    fn snapshot_empty_store_returns_empty_maps() {
        let mut s = TelemetryStore::new();
        let snap = s.build_snapshot();
        assert!(snap.events_per_kind.is_empty());
        assert!(snap.histograms.is_empty());
    }

    #[test]
    fn ring_capacity_constant_matches_spec() {
        // Galaxy Book 4 fast SSD: 1000 events e sweet spot RAM/duration.
        assert_eq!(RING_CAPACITY, 1000);
    }
}
