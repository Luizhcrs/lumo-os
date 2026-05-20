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
