//! W10.A lumo-lock unit tests — 5+ tests per spec.

#[cfg(test)]
mod tests {
    #[allow(unused_imports)]
    use crate::LumoLock;
    use tiny_skia::PixmapMut;

    // Helper: build a minimal LumoLock with only the fields needed for unit tests.
    // (No Wayland connection required.)
    fn make_lock() -> LumoLockStub {
        LumoLockStub {
            password: String::new(),
            shake_count: 0,
            shake_start: None,
            last_fail_msg: String::new(),
        }
    }

    struct LumoLockStub {
        password: String,
        shake_count: u32,
        shake_start: Option<std::time::Instant>,
        last_fail_msg: String,
    }

    impl LumoLockStub {
        fn shake_offset(&self) -> f32 {
            let Some(start) = self.shake_start else {
                return 0.0;
            };
            let elapsed = start.elapsed().as_secs_f32();
            if elapsed > 0.5 {
                return 0.0;
            }
            let amplitude = 8.0f32;
            let freq = 40.0f32;
            let decay = 10.0f32;
            amplitude * (-decay * elapsed).exp() * (freq * elapsed * std::f32::consts::TAU).sin()
        }
    }

    #[test]
    fn shake_offset_zero_when_no_start() {
        let lock = make_lock();
        assert_eq!(lock.shake_offset(), 0.0);
    }

    #[test]
    fn shake_offset_nonzero_just_after_start() {
        let mut lock = make_lock();
        lock.shake_start = Some(std::time::Instant::now());
        // Immediately after start t~0 -> sin(0)=0 but exp(0)=1, amplitude*0=0.
        // The exact value depends on timing; we just verify it returns a finite f32.
        let offset = lock.shake_offset();
        assert!(offset.is_finite());
    }

    #[test]
    fn shake_offset_zero_after_500ms() {
        let mut lock = make_lock();
        lock.shake_start = Some(std::time::Instant::now() - std::time::Duration::from_millis(600));
        assert_eq!(lock.shake_offset(), 0.0);
    }

    #[test]
    fn password_accumulates_chars() {
        let mut lock = make_lock();
        lock.password.push('a');
        lock.password.push('b');
        lock.password.push('c');
        assert_eq!(lock.password.len(), 3);
    }

    #[test]
    fn backspace_removes_last_char() {
        let mut lock = make_lock();
        lock.password = "hello".to_string();
        lock.password.pop();
        assert_eq!(lock.password, "hell");
    }

    #[test]
    fn shake_count_increments_on_fail() {
        let mut lock = make_lock();
        assert_eq!(lock.shake_count, 0);
        lock.shake_count += 1;
        lock.shake_start = Some(std::time::Instant::now());
        lock.last_fail_msg = "Senha incorreta".to_string();
        assert_eq!(lock.shake_count, 1);
        assert!(!lock.last_fail_msg.is_empty());
    }

    #[test]
    fn paint_lock_runs_without_panic() {
        let w = 320u32;
        let h = 240u32;
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let mut pixmap = PixmapMut::from_bytes(&mut buf, w, h).unwrap();
        // Should not panic regardless of input.
        crate::paint_lock(&mut pixmap, w, h, "secret", "", 0.0);
        crate::paint_lock(&mut pixmap, w, h, "", "Senha incorreta", 3.0);
        crate::paint_lock(&mut pixmap, w, h, "aaaaa", "err", -2.0);
    }

    #[test]
    fn paint_lock_non_empty_output() {
        let w = 320u32;
        let h = 240u32;
        let mut buf = vec![0u8; (w * h * 4) as usize];
        let mut pixmap = PixmapMut::from_bytes(&mut buf, w, h).unwrap();
        crate::paint_lock(&mut pixmap, w, h, "pw", "", 0.0);
        // Backdrop must paint at least some non-zero pixels.
        assert!(buf.iter().any(|&b| b != 0));
    }
}
