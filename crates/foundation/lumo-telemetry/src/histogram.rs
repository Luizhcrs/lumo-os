//! HDR histogram wrapper with JSON snapshot.

use hdrhistogram::Histogram;
use serde::Serialize;

#[derive(Debug, Serialize, Clone)]
pub struct SnapshotJson {
    pub p50: u64,
    pub p95: u64,
    pub p99: u64,
    pub max: u64,
    pub count: u64,
}

pub struct LumoHistogram {
    inner: Histogram<u64>,
}

impl LumoHistogram {
    pub fn new() -> Self {
        // Range: 1us..60s, 3 significant digits.
        Self {
            inner: Histogram::new_with_bounds(1, 60_000_000, 3).expect("hdrhistogram bounds"),
        }
    }

    pub fn record(&mut self, value_us: u64) {
        // Saturate at max instead of panicking on out-of-range.
        let v = value_us.clamp(1, 60_000_000);
        let _ = self.inner.record(v);
    }

    pub fn snapshot(&self) -> SnapshotJson {
        SnapshotJson {
            p50: self.inner.value_at_quantile(0.50),
            p95: self.inner.value_at_quantile(0.95),
            p99: self.inner.value_at_quantile(0.99),
            max: self.inner.max(),
            count: self.inner.len(),
        }
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.inner.reset();
    }
}

impl Default for LumoHistogram {
    fn default() -> Self {
        Self::new()
    }
}
