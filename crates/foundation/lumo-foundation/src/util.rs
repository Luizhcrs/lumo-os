//! util.rs — helpers compartilhados entre crates Lumo.
//!
//! Consolida funcs antes duplicadas em lumo-notif/sanitize, lumo-notif/rate_limit,
//! lumo-clip/history (clamp + safe_lock + rate_limit_check + markup_escape).

use std::sync::Mutex;
use std::time::Instant;

/// Trunca string em chars (nao bytes). Retorna nova string com ate `max_chars`.
pub fn clamp(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    s.chars().take(max_chars).collect()
}

/// Escape Pango markup chars + strip control chars.
/// Mesma logica do glib::markup_escape_text + extras de seguranca.
pub fn markup_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\'' => out.push_str("&apos;"),
            '"' => out.push_str("&quot;"),
            c if (c as u32) < 0x20 && c != '\n' && c != '\t' => {}
            c if (c as u32) == 0x7f => {}
            c => out.push(c),
        }
    }
    out
}

/// Mutex lock que sobrevive poisoning — retorna inner mesmo se outra thread
/// panicou segurando o lock.
pub fn safe_lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

/// Decide se um evento esta dentro do rate-limit. Mutating: drena history
/// antigo + push current. Retorna true se permitiu.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    // clamp
    #[test]
    fn clamp_short_unchanged() {
        assert_eq!(clamp("hi", 10), "hi");
    }

    #[test]
    fn clamp_exact_length() {
        assert_eq!(clamp("12345", 5), "12345");
    }

    #[test]
    fn clamp_truncates() {
        assert_eq!(clamp("1234567890", 5), "12345");
    }

    #[test]
    fn clamp_chars_not_bytes() {
        let s = "ñáéíó";
        assert_eq!(s.chars().count(), 5);
        assert!(s.len() > 5);
        assert_eq!(clamp(s, 5), s);
    }

    #[test]
    fn clamp_multibyte_truncate() {
        assert_eq!(clamp("ñáéíó", 2).chars().count(), 2);
    }

    // markup_escape
    #[test]
    fn markup_escape_ampersand() {
        assert_eq!(markup_escape("A & B"), "A &amp; B");
    }

    #[test]
    fn markup_escape_lt_gt() {
        assert_eq!(markup_escape("<b>"), "&lt;b&gt;");
    }

    #[test]
    fn markup_escape_quotes() {
        assert_eq!(markup_escape("'x'"), "&apos;x&apos;");
        assert_eq!(markup_escape("\"y\""), "&quot;y&quot;");
    }

    #[test]
    fn markup_escape_preserves_newline_tab() {
        assert_eq!(markup_escape("a\nb\tc"), "a\nb\tc");
    }

    #[test]
    fn markup_escape_strips_ansi() {
        let s = "a\x1b[31mRED\x07b";
        let out = markup_escape(s);
        assert!(!out.contains('\x1b'));
        assert!(!out.contains('\x07'));
    }

    #[test]
    fn markup_escape_strips_del() {
        assert_eq!(markup_escape("a\x7fb"), "ab");
    }

    #[test]
    fn markup_escape_neutralizes_pango() {
        let attack = "<span>FAKE</span>";
        let out = markup_escape(attack);
        assert!(!out.contains("<span"));
    }

    #[test]
    fn markup_escape_empty() {
        assert_eq!(markup_escape(""), "");
    }

    #[test]
    fn markup_escape_safe_string_unchanged() {
        assert_eq!(markup_escape("hello 123"), "hello 123");
    }

    // safe_lock
    #[test]
    fn safe_lock_basic() {
        let m = Mutex::new(7);
        assert_eq!(*safe_lock(&m), 7);
    }

    #[test]
    fn safe_lock_survives_poison() {
        let m = Arc::new(Mutex::new(42));
        let m2 = m.clone();
        let _ = std::thread::spawn(move || {
            let _g = m2.lock().unwrap();
            panic!("envenena");
        })
        .join();
        let g = safe_lock(&m);
        assert_eq!(*g, 42);
    }

    // rate_limit
    #[test]
    fn rate_limit_allows_below_burst() {
        let mut h = vec![];
        let now = Instant::now();
        for _ in 0..5 {
            assert!(rate_limit_check(&mut h, now, 10, 1000));
        }
    }

    #[test]
    fn rate_limit_blocks_after_burst() {
        let mut h = vec![];
        let now = Instant::now();
        for _ in 0..10 {
            rate_limit_check(&mut h, now, 10, 1000);
        }
        assert!(!rate_limit_check(&mut h, now, 10, 1000));
    }

    #[test]
    fn rate_limit_evicts_outside_window() {
        let mut h = vec![];
        let t0 = Instant::now();
        for _ in 0..10 {
            rate_limit_check(&mut h, t0, 10, 100);
        }
        let later = t0 + Duration::from_millis(200);
        assert!(rate_limit_check(&mut h, later, 10, 100));
    }

    #[test]
    fn rate_limit_zero_burst_blocks_all() {
        let mut h = vec![];
        assert!(!rate_limit_check(&mut h, Instant::now(), 0, 1000));
    }

    #[test]
    fn rate_limit_partial_window_replenish() {
        let mut h = vec![];
        let t0 = Instant::now();
        for _ in 0..3 {
            rate_limit_check(&mut h, t0, 3, 100);
        }
        let mid = t0 + Duration::from_millis(50);
        assert!(!rate_limit_check(&mut h, mid, 3, 100));
        let after = t0 + Duration::from_millis(150);
        assert!(rate_limit_check(&mut h, after, 3, 100));
    }
}
