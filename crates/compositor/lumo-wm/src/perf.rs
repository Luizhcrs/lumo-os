//! perf.rs - Instrumentacao de performance Lumo WM (W6.D).
//!
//! Histograma p50/p95/p99 de frame time e input-to-presented latency.
//! Logado via tracing::info! a cada 60s (alinhadao com L2 em drm.rs).
//!
//! Uso:
//!   - PerfTracker::new() no startup do backend
//!   - record_frame(duration) a cada frame renderizado
//!   - record_input_latency(duration) quando presentation-time callback chega
//!     (W3 presentation-time protocol -- futuro; por enquanto usa frame_dur como proxy)
//!   - tick(elapsed_since_last_log) checa se e hora de logar

use std::time::Duration;

/// Histograma simplificado: vetor de samples, sort-on-demand no log.
pub struct PerfHistogram {
    samples: Vec<Duration>,
    label: &'static str,
}

impl PerfHistogram {
    pub fn new(label: &'static str) -> Self {
        Self { samples: Vec::with_capacity(4096), label }
    }

    pub fn record(&mut self, d: Duration) {
        self.samples.push(d);
    }

    /// Calcula e retorna (p50, p95, p99) em microssegundos.
    /// Retorna None se amostras insuficientes (< 10).
    pub fn percentiles(&mut self) -> Option<(u64, u64, u64)> {
        if self.samples.len() < 10 {
            return None;
        }
        self.samples.sort_unstable();
        let n = self.samples.len();
        let p50 = self.samples[n / 2].as_micros() as u64;
        let p95 = self.samples[(n * 95 / 100).min(n - 1)].as_micros() as u64;
        let p99 = self.samples[(n * 99 / 100).min(n - 1)].as_micros() as u64;
        Some((p50, p95, p99))
    }

    pub fn clear(&mut self) {
        self.samples.clear();
    }

    pub fn len(&self) -> usize {
        self.samples.len()
    }
}

/// Tracker agregado: frame time + input latency + contador de frames.
pub struct PerfTracker {
    pub frame_time: PerfHistogram,
    pub input_latency: PerfHistogram,
    pub total_frames: u64,
}

impl PerfTracker {
    pub fn new() -> Self {
        Self {
            frame_time: PerfHistogram::new("frame_time"),
            input_latency: PerfHistogram::new("input_latency"),
            total_frames: 0,
        }
    }

    /// Registra duracao de um frame.
    pub fn record_frame(&mut self, d: Duration) {
        self.frame_time.record(d);
        self.total_frames += 1;
    }

    /// Registra latencia input-to-presented (quando disponivel via presentation-time).
    pub fn record_input_latency(&mut self, d: Duration) {
        self.input_latency.record(d);
    }

    /// Log p50/p95/p99 via tracing. Limpa samples apos log.
    /// Retorna true se logou (suficiente samples).
    pub fn log_and_reset(&mut self) -> bool {
        let logged_ft = if let Some((p50, p95, p99)) = self.frame_time.percentiles() {
            // Converte us -> ms para leitura humana.
            tracing::info!(
                samples = self.frame_time.len(),
                total_frames = self.total_frames,
                frame_time_p50_us = p50,
                frame_time_p95_us = p95,
                frame_time_p99_us = p99,
                frame_time_p50_ms = p50 / 1000,
                frame_time_p95_ms = p95 / 1000,
                frame_time_p99_ms = p99 / 1000,
                "W6.D: frame_time_p50={:.2}ms p95={:.2}ms p99={:.2}ms",
                p50 as f64 / 1000.0,
                p95 as f64 / 1000.0,
                p99 as f64 / 1000.0,
            );
            self.frame_time.clear();
            true
        } else {
            false
        };

        if let Some((p50, p95, p99)) = self.input_latency.percentiles() {
            tracing::info!(
                samples = self.input_latency.len(),
                input_latency_p50_us = p50,
                input_latency_p95_us = p95,
                input_latency_p99_us = p99,
                "W6.D: input_latency_p50={:.2}ms p95={:.2}ms p99={:.2}ms",
                p50 as f64 / 1000.0,
                p95 as f64 / 1000.0,
                p99 as f64 / 1000.0,
            );
            self.input_latency.clear();
        }
        logged_ft
    }
}

impl Default for PerfTracker {
    fn default() -> Self {
        Self::new()
    }
}
