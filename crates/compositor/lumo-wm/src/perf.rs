//! perf.rs - Instrumentacao de performance Lumo WM (W6.D + W11.D).
//!
//! W6.D: Histograma p50/p95/p99 de frame time e input-to-presented latency.
//! W11.D: Sample continuo de RSS (VmRSS /proc/self/status) e CPU
//!        (/proc/self/stat utime+stime delta) a cada 60s.
//!        Campos LumoState.perf.cpu_usage_pct e .rss_mb expostos.
//!        Log estruturado via tracing::info!.
//!
//! Targets:
//!   - compositor idle CPU < 1%
//!   - shell completo RSS < 200MB
//!   - frame time p95 < 16ms

use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// PerfHistogram
// ---------------------------------------------------------------------------

/// Histograma simplificado: vetor de samples, sort-on-demand no log.
pub struct PerfHistogram {
    samples: Vec<Duration>,
    _label: &'static str,
}

impl PerfHistogram {
    pub fn new(label: &'static str) -> Self {
        Self { samples: Vec::with_capacity(4096), _label: label }
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

// ---------------------------------------------------------------------------
// ProcStat - leitura de /proc/self/stat para CPU usage
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
struct ProcStat {
    utime: u64,
    stime: u64,
}

impl ProcStat {
    fn read() -> Option<Self> {
        let s = std::fs::read_to_string("/proc/self/stat").ok()?;
        // Formato: pid (comm) state ppid ... utime(14) stime(15) ...
        // Campos separados por espaco; comm pode ter espacos entre parens.
        let after_comm = s.rfind(')')?;
        let rest = &s[after_comm + 2..];
        let fields: Vec<&str> = rest.split_whitespace().collect();
        // utime = campo 14 (index 11 apos remover pid+comm+state+ppid+pgrp+session+tty+tpgid+flags+minflt+cminflt+majflt+cmajflt)
        // relativo ao rest: state(0) ppid(1) pgrp(2) session(3) tty(4) tpgid(5) flags(6) minflt(7) cminflt(8) majflt(9) cmajflt(10) utime(11) stime(12)
        if fields.len() < 13 {
            return None;
        }
        let utime = fields[11].parse().ok()?;
        let stime = fields[12].parse().ok()?;
        Some(Self { utime, stime })
    }

    fn total(&self) -> u64 {
        self.utime + self.stime
    }
}

// ---------------------------------------------------------------------------
// ProcStatus - leitura de VmRSS de /proc/self/status
// ---------------------------------------------------------------------------

fn read_rss_kb() -> Option<u64> {
    let s = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in s.lines() {
        if line.starts_with("VmRSS:") {
            // "VmRSS:   12345 kB"
            let val: u64 = line
                .split_whitespace()
                .nth(1)?
                .parse()
                .ok()?;
            return Some(val);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// PerfSampler - amostragem periodica RSS+CPU (W11.D)
// ---------------------------------------------------------------------------

/// Estado para calculo de CPU usage delta entre amostras.
pub struct PerfSampler {
    last_stat: Option<ProcStat>,
    last_sample_time: Instant,
    pub cpu_usage_pct: f32,
    pub rss_mb: f32,
    /// Tick clock (USER_HZ) -- Linux: 100 Hz padrao.
    ticks_per_sec: u64,
}

impl PerfSampler {
    pub fn new() -> Self {
        let ticks_per_sec = Self::read_ticks_per_sec();
        Self {
            last_stat: ProcStat::read(),
            last_sample_time: Instant::now(),
            cpu_usage_pct: 0.0,
            rss_mb: 0.0,
            ticks_per_sec,
        }
    }

    fn read_ticks_per_sec() -> u64 {
        #[cfg(target_os = "linux")]
        unsafe {
            let v = libc::sysconf(libc::_SC_CLK_TCK);
            if v > 0 { return v as u64; }
        }
        100
    }

    /// Deve ser chamado a cada ~60s. Atualiza cpu_usage_pct e rss_mb.
    pub fn sample(&mut self) {
        let now = Instant::now();
        let elapsed_secs = now.duration_since(self.last_sample_time).as_secs_f64();
        self.last_sample_time = now;

        // RSS
        if let Some(rss_kb) = read_rss_kb() {
            self.rss_mb = rss_kb as f32 / 1024.0;
        }

        // CPU
        if let Some(stat) = ProcStat::read() {
            if let Some(ref last) = self.last_stat {
                let delta_ticks = stat.total().saturating_sub(last.total());
                let tps = self.ticks_per_sec as f64;
                // CPU% = (delta_ticks / ticks_per_sec) / elapsed_secs * 100
                if elapsed_secs > 0.0 {
                    self.cpu_usage_pct =
                        ((delta_ticks as f64 / tps) / elapsed_secs * 100.0) as f32;
                }
            }
            self.last_stat = Some(stat);
        }

        tracing::info!(
            cpu_pct = self.cpu_usage_pct,
            rss_mb = self.rss_mb,
            "W11.D perf: cpu={:.2}% rss={:.1}MB",
            self.cpu_usage_pct,
            self.rss_mb,
        );

        // Alertas de threshold
        if self.rss_mb > 200.0 {
            tracing::warn!(
                rss_mb = self.rss_mb,
                "W11.D perf: RSS acima de 200MB (target: <200MB)"
            );
        }
        if self.cpu_usage_pct > 5.0 {
            tracing::warn!(
                cpu_pct = self.cpu_usage_pct,
                "W11.D perf: CPU idle acima de 5% (target: <1%)"
            );
        }
    }

    /// Verifica se e hora de amostrar (a cada 60s).
    pub fn should_sample(&self) -> bool {
        self.last_sample_time.elapsed() >= Duration::from_secs(60)
    }
}

impl Default for PerfSampler {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PerfTracker - agregado frame time + input latency + sampler (W11.D)
// ---------------------------------------------------------------------------

/// Tracker agregado: frame time + input latency + counter + RSS/CPU sampler.
pub struct PerfTracker {
    pub frame_time: PerfHistogram,
    pub input_latency: PerfHistogram,
    pub total_frames: u64,
    /// W11.D: sampler RSS+CPU.
    pub sampler: PerfSampler,
    /// Exposto para bar pill opcional (dev mode).
    pub cpu_usage_pct: f32,
    /// Exposto para bar pill opcional (dev mode).
    pub rss_mb: f32,
}

impl PerfTracker {
    pub fn new() -> Self {
        let sampler = PerfSampler::new();
        let cpu = sampler.cpu_usage_pct;
        let rss = sampler.rss_mb;
        Self {
            frame_time: PerfHistogram::new("frame_time"),
            input_latency: PerfHistogram::new("input_latency"),
            total_frames: 0,
            sampler,
            cpu_usage_pct: cpu,
            rss_mb: rss,
        }
    }

    /// Registra duracao de um frame.
    pub fn record_frame(&mut self, d: Duration) {
        self.frame_time.record(d);
        self.total_frames += 1;

        // W11.D: samplear RSS+CPU a cada 60s (chamado no hot path, check barato)
        if self.sampler.should_sample() {
            self.sampler.sample();
            self.cpu_usage_pct = self.sampler.cpu_usage_pct;
            self.rss_mb = self.sampler.rss_mb;
        }
    }

    /// Registra latencia input-to-presented.
    pub fn record_input_latency(&mut self, d: Duration) {
        self.input_latency.record(d);
    }

    /// Log p50/p95/p99 via tracing. Limpa samples apos log.
    pub fn log_and_reset(&mut self) -> bool {
        let logged_ft = if let Some((p50, p95, p99)) = self.frame_time.percentiles() {
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
            // p95 > 16ms = frame drop warning
            if p95 > 16_000 {
                tracing::warn!(
                    frame_time_p95_us = p95,
                    "W11.D: frame_time p95 acima de 16ms (target: <16ms)"
                );
            }
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_percentiles_basic() {
        let mut h = PerfHistogram::new("test");
        for i in 1..=100 {
            h.record(Duration::from_micros(i * 100));
        }
        let (p50, p95, p99) = h.percentiles().unwrap();
        assert!(p50 < p95);
        assert!(p95 < p99);
    }

    #[test]
    fn histogram_percentiles_none_below_10() {
        let mut h = PerfHistogram::new("test");
        for i in 0..5 {
            h.record(Duration::from_micros(i));
        }
        assert!(h.percentiles().is_none());
    }

    #[test]
    fn histogram_clear() {
        let mut h = PerfHistogram::new("test");
        for i in 0..50 {
            h.record(Duration::from_micros(i));
        }
        h.clear();
        assert_eq!(h.len(), 0);
    }

    #[test]
    fn read_rss_kb_linux() {
        // Linux-only: /proc/self/status deve existir.
        // Em CI sem /proc pula silenciosamente.
        if std::path::Path::new("/proc/self/status").exists() {
            let rss = read_rss_kb();
            assert!(rss.is_some());
            assert!(rss.unwrap() > 0);
        }
    }

    #[test]
    fn proc_stat_read_linux() {
        if std::path::Path::new("/proc/self/stat").exists() {
            let stat = ProcStat::read();
            assert!(stat.is_some());
            let s = stat.unwrap(); assert!(s.utime + s.stime == s.total());
        }
    }

    #[test]
    fn perf_tracker_record_frame_updates_total() {
        let mut pt = PerfTracker::new();
        pt.record_frame(Duration::from_millis(16));
        pt.record_frame(Duration::from_millis(16));
        assert_eq!(pt.total_frames, 2);
    }

    #[test]
    fn perf_sampler_cpu_non_negative() {
        let mut sampler = PerfSampler::new();
        // Forca sample independente do timer
        sampler.last_sample_time = Instant::now() - Duration::from_secs(61);
        sampler.sample();
        assert!(sampler.cpu_usage_pct >= 0.0);
    }

    #[test]
    fn perf_sampler_rss_non_negative() {
        let mut sampler = PerfSampler::new();
        sampler.last_sample_time = Instant::now() - Duration::from_secs(61);
        sampler.sample();
        assert!(sampler.rss_mb >= 0.0);
    }

    #[test]
    fn log_and_reset_returns_false_few_samples() {
        let mut pt = PerfTracker::new();
        pt.record_frame(Duration::from_millis(16));
        let logged = pt.log_and_reset();
        assert!(!logged); // menos de 10 amostras
    }
}
