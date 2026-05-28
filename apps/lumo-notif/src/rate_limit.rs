//! rate_limit.rs — H2 review: token-bucket-ish rate-limit por sender DBus.
//!
//! Pure logic, sem deps DBus, pra rodar tests Windows.

use std::sync::Mutex;
use std::time::Instant;

/// Decide se permitir um event agora; mantem historico de events em `history`.
/// Retorna true se permitir (e bumpa history); false se exceder burst no window.
pub fn rate_limit_check(
    history: &mut Vec<Instant>,
    now: Instant,
    burst: usize,
    window_ms: u64,
) -> bool {
    let cutoff = now
        .checked_sub(std::time::Duration::from_millis(window_ms))
        .unwrap_or(now);
    history.retain(|t| *t >= cutoff);
    if history.len() >= burst {
        return false;
    }
    history.push(now);
    true
}

/// H5: lock que sobrevive a poisoning. Mesma semantica que `Mutex::lock` mas
/// nunca panica — retorna inner mesmo se outra thread morreu segurando o lock.
pub fn safe_lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn allows_below_burst() {
        let mut h = vec![];
        let now = Instant::now();
        for _ in 0..5 {
            assert!(rate_limit_check(&mut h, now, 10, 1000));
        }
    }

    #[test]
    fn blocks_after_burst() {
        let mut h = vec![];
        let now = Instant::now();
        for _ in 0..10 {
            assert!(rate_limit_check(&mut h, now, 10, 1000));
        }
        assert!(!rate_limit_check(&mut h, now, 10, 1000));
    }

    #[test]
    fn evicts_entries_outside_window() {
        let mut h = vec![];
        let t0 = Instant::now();
        for _ in 0..10 {
            rate_limit_check(&mut h, t0, 10, 100);
        }
        let later = t0 + Duration::from_millis(200);
        assert!(rate_limit_check(&mut h, later, 10, 100));
        // historico nao deve mais ter os antigos.
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn zero_burst_blocks_everything() {
        let mut h = vec![];
        assert!(!rate_limit_check(&mut h, Instant::now(), 0, 1000));
    }

    #[test]
    fn allows_partial_window_replenish() {
        let mut h = vec![];
        let t0 = Instant::now();
        for _ in 0..3 {
            rate_limit_check(&mut h, t0, 3, 100);
        }
        let later = t0 + Duration::from_millis(50);
        // Mesmo window — ainda bloqueia.
        assert!(!rate_limit_check(&mut h, later, 3, 100));
        let way_later = t0 + Duration::from_millis(150);
        // Janela passou — libera.
        assert!(rate_limit_check(&mut h, way_later, 3, 100));
    }

    #[test]
    fn safe_lock_basic_access() {
        let m = Mutex::new(7);
        assert_eq!(*safe_lock(&m), 7);
    }

    #[test]
    fn safe_lock_survives_poison() {
        let m = Arc::new(Mutex::new(99));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("envenena");
        })
        .join();
        // Poisoned, mas safe_lock retorna inner.
        let g = safe_lock(&m);
        assert_eq!(*g, 99);
    }
}
