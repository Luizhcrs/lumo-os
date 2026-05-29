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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_histogram_is_empty() {
        let h = LumoHistogram::new();
        let s = h.snapshot();
        assert_eq!(s.count, 0);
        assert_eq!(s.max, 0);
    }

    #[test]
    fn record_increments_count() {
        let mut h = LumoHistogram::new();
        h.record(100);
        h.record(200);
        h.record(300);
        let s = h.snapshot();
        assert_eq!(s.count, 3);
    }

    #[test]
    fn record_below_bound_clamps_to_one() {
        let mut h = LumoHistogram::new();
        h.record(0); // abaixo do bound minimo (1)
        let s = h.snapshot();
        assert_eq!(s.count, 1);
        // Valor armazenado deve ser >=1 (clamped).
        assert!(s.max >= 1);
    }

    #[test]
    fn record_above_bound_saturates_no_panic() {
        let mut h = LumoHistogram::new();
        // 60s = 60_000_000us = bound max. Acima satura (clamp no record).
        h.record(999_999_999_999);
        let s = h.snapshot();
        assert_eq!(s.count, 1);
        // hdr guarda em buckets de 3 sig-figs; max() retorna o TETO do bucket
        // que contem 60_000_000 (passa de 60M por ~0.1%). Saturou = max dentro
        // do bucket-equivalente do bound, nao o valor cru gigante.
        assert!(s.max <= h.inner.highest_equivalent(60_000_000));
        assert!(s.max < 999_999_999_999);
    }

    #[test]
    fn percentiles_monotonic() {
        let mut h = LumoHistogram::new();
        for v in 1..=1000 {
            h.record(v);
        }
        let s = h.snapshot();
        assert!(s.p50 <= s.p95);
        assert!(s.p95 <= s.p99);
        assert!(s.p99 <= s.max);
    }

    #[test]
    fn reset_zeros_count() {
        let mut h = LumoHistogram::new();
        h.record(500);
        h.record(1000);
        h.reset();
        let s = h.snapshot();
        assert_eq!(s.count, 0);
    }

    #[test]
    fn snapshot_serializable_json() {
        let mut h = LumoHistogram::new();
        h.record(123);
        let s = h.snapshot();
        let json = serde_json::to_string(&s).expect("serialize");
        assert!(json.contains("p50"));
        assert!(json.contains("p95"));
        assert!(json.contains("count"));
    }

    #[test]
    fn default_equivalent_to_new() {
        let a = LumoHistogram::default();
        let b = LumoHistogram::new();
        assert_eq!(a.snapshot().count, b.snapshot().count);
    }
}
